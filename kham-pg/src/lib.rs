//! kham-pg — PostgreSQL text-search parser extension for Thai.
//!
//! PostgreSQL calls four C callbacks (`kham_start`, `kham_gettoken`,
//! `kham_end`, `kham_lextypes`) that are implemented in `shim.c`.  That shim
//! handles all fmgr macro boilerplate and then delegates to the three
//! `*_impl` functions defined here, which use [`FtsTokenizer`] to segment the
//! document text.
//!
//! The parser uses [`FtsTokenizer`] (not the base [`Tokenizer`]) because it
//! must emit [`TokenKind::Named`] for NE tokens so PostgreSQL routes them to
//! the correct dictionary mapping.  Stopword filtering, synonym expansion,
//! soundex, and RTGS romanization are handled downstream by the dictionary
//! callbacks (`kham_dict_lexize_*`).
//!
//! Token-type mapping
//! ------------------
//! | PG type | Name    | [`TokenKind`]             |
//! |---------|---------|---------------------------|
//! | 1       | thai    | [`TokenKind::Thai`]        |
//! | 2       | latin   | [`TokenKind::Latin`]       |
//! | 3       | number  | [`TokenKind::Number`]      |
//! | 4       | punct   | [`TokenKind::Punctuation`] |
//! | 5       | emoji   | [`TokenKind::Emoji`]       |
//! | 6       | unknown | [`TokenKind::Unknown`]     |

// PostgreSQL fmgr trampolines are C-ABI entry points called by the PG backend.
// They have no Rust callers, so Safety docs would be noise.
#![allow(clippy::missing_safety_doc)]

use std::num::NonZeroUsize;
use std::os::raw::{c_char, c_int, c_void};
use std::panic::catch_unwind;
use std::sync::{Mutex, OnceLock};

use lru::LruCache;

use kham_core::fts::FtsTokenizer;
use kham_core::romanizer::RomanizationMap;
use kham_core::soundex::SoundexAlgorithm;
use kham_core::TokenKind;

// ---------------------------------------------------------------------------
// Parser state
// ---------------------------------------------------------------------------

/// Per-document parser state allocated in [`kham_start_impl`] and freed in
/// [`kham_end_impl`].
///
/// All token texts are packed into `text_buf` (one allocation for the whole
/// document); `tokens` records `(byte_offset, byte_len, pg_type)` triples so
/// `kham_gettoken_impl` can point PostgreSQL directly into the buffer without
/// any per-token allocation.
struct KhamState {
    /// Concatenated UTF-8 bytes of every non-whitespace token.
    text_buf: Vec<u8>,
    /// `(byte_offset_in_text_buf, byte_len, pg_token_type)` per token.
    tokens: Vec<(usize, usize, c_int)>,
    cursor: usize,
}

fn kind_to_pg_type(kind: TokenKind) -> c_int {
    match kind {
        TokenKind::Thai => 1,
        TokenKind::Latin => 2,
        TokenKind::Number => 3,
        TokenKind::Punctuation => 4,
        TokenKind::Emoji => 5,
        TokenKind::Whitespace => 0, // filtered below; should not reach here
        TokenKind::Unknown => 6,
        TokenKind::Named(_) => 7,
    }
}

// ---------------------------------------------------------------------------
// Cached parser tokenizer — initialised once per process, reused on every call.
//
// Uses FtsTokenizer (not the base Tokenizer) so that named entities are tagged
// TokenKind::Named(_) and PostgreSQL routes them to the correct dictionary.
// Soundex and RTGS romanization are not configured here — that work is done
// exclusively in the dictionary callbacks.
// ---------------------------------------------------------------------------

static PARSER_FTS: OnceLock<FtsTokenizer> = OnceLock::new();

fn parser_fts() -> &'static FtsTokenizer {
    PARSER_FTS.get_or_init(FtsTokenizer::new)
}

// ---------------------------------------------------------------------------
// Impl functions called from shim.c
// ---------------------------------------------------------------------------

/// Tokenise `text` (non-null-terminated, `len` bytes of UTF-8) and return an
/// opaque heap pointer to a [`KhamState`].
///
/// All non-whitespace token texts are packed into a single `text_buf`
/// allocation; `tokens` records `(offset, len, pg_type)` triples into it.
/// This reduces per-document heap allocations from O(n_tokens) to 2.
///
/// Returns `NULL` on panic — the C shim converts `NULL` to a PG error.
///
/// # Safety
///
/// `text` must point to `len` bytes of valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_start_impl(text: *const c_char, len: c_int) -> *mut c_void {
    let result = catch_unwind(|| {
        let bytes = unsafe { std::slice::from_raw_parts(text as *const u8, len as usize) };
        let s = std::str::from_utf8(bytes).unwrap_or("");

        let fts_tokens: Vec<_> = parser_fts()
            .segment_for_fts(s)
            .into_iter()
            .filter(|t| t.kind != TokenKind::Whitespace)
            .collect();

        // Pack all token texts into one contiguous buffer — 2 allocs total.
        let total: usize = fts_tokens.iter().map(|t| t.text.len()).sum();
        let mut text_buf = Vec::with_capacity(total);
        let mut tokens = Vec::with_capacity(fts_tokens.len());

        for ft in fts_tokens {
            let offset = text_buf.len();
            let len = ft.text.len();
            text_buf.extend_from_slice(ft.text.as_bytes());
            tokens.push((offset, len, kind_to_pg_type(ft.kind)));
        }

        Box::into_raw(Box::new(KhamState { text_buf, tokens, cursor: 0 })) as *mut c_void
    });

    result.unwrap_or(std::ptr::null_mut())
}

/// Write the next token into `*token` / `*tokenlen` and return its PG type.
///
/// Sets `*token` to point directly into `KhamState::text_buf` — no copy.
/// Returns `0` when all tokens have been consumed.
///
/// # Safety
///
/// `state` must be a valid [`KhamState`] pointer from [`kham_start_impl`].
/// `token` and `tokenlen` must be valid non-null output pointers.
#[no_mangle]
pub unsafe extern "C" fn kham_gettoken_impl(
    state: *mut c_void,
    token: *mut *const c_char,
    tokenlen: *mut c_int,
) -> c_int {
    if state.is_null() || token.is_null() || tokenlen.is_null() {
        return 0;
    }

    let state = unsafe { &mut *(state as *mut KhamState) };

    if state.cursor >= state.tokens.len() {
        return 0; // end of document
    }

    let (offset, len, pg_type) = state.tokens[state.cursor];
    state.cursor += 1;

    unsafe {
        *token = state.text_buf.as_ptr().add(offset) as *const c_char;
        *tokenlen = len as c_int;
    }

    pg_type
}

/// Free the [`KhamState`] allocated by [`kham_start_impl`].
///
/// # Safety
///
/// `state` must be a valid pointer from [`kham_start_impl`] and must not have
/// been freed already.  Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_end_impl(state: *mut c_void) {
    if !state.is_null() {
        unsafe { drop(Box::from_raw(state as *mut KhamState)) };
    }
}

// ---------------------------------------------------------------------------
// Dictionary impl — kham_fts_dict / kham_fts_dict_udom83 / kham_fts_dict_metasound
//
// Three custom PG text-search dictionaries that expand each Thai / Named token
// into up to six colocated lexemes at the same tsvector position:
//   1. The normalised word itself
//   2. Its soundex code          (phonetic fuzzy search)
//   3. Its RTGS romanization     (Latin-script search, if in the map)
//   4. ASCII number form         (for Thai digit strings / number words)
//   5. POS lexeme "pos_<tag>"    (part-of-speech filtering, e.g. pos_verb)
//
// Stopwords are suppressed: if the built-in stopword list contains the token,
// the function returns 0 (NULL) so PostgreSQL drops it from the tsvector.
//
// kham_fts_dict:           lk82 soundex  (default, most widely used)
// kham_fts_dict_udom83:    udom83 soundex (finer sibilant/liquid distinctions)
// kham_fts_dict_metasound: MetaSound     (per-syllable encoding)
// ---------------------------------------------------------------------------

/// Up to 6 lexeme slots, each holding a null-terminated UTF-8 string (≤ 127 bytes).
/// Filled by the `*_impl` functions and consumed by `*_shim` in C.
#[repr(C)]
pub struct KhamDictOut {
    pub count: c_int,
    pub words: [[u8; 128]; 6],
}

fn write_slot(slot: &mut [u8; 128], src: &[u8]) {
    let n = src.len().min(127);
    slot[..n].copy_from_slice(&src[..n]);
    slot[n] = 0;
}

// ---------------------------------------------------------------------------
// Per-word lexeme cache
//
// `segment_for_fts` on a single word (soundex + RTGS + POS expansion) costs
// ~0.05 ms.  The same common Thai words repeat across many documents, so we
// cache the expanded lexeme list keyed on the raw word string.  On a cache hit
// the dictionary callback becomes a few Vec clones + memcpy into KhamDictOut.
//
// Cache capacity: 4 096 unique words per algorithm variant.  Thai working
// vocabulary for most corpora fits well within this limit.  PostgreSQL backends
// are single-threaded, so Mutex contention is zero in practice.
// ---------------------------------------------------------------------------

const DICT_CACHE_CAP: usize = 4096;

/// Cached result for one word: either suppressed (stopword) or up to 6 lexemes.
#[derive(Clone)]
enum CachedLexeme {
    Stopword,
    Words(Vec<String>),
}

/// Bundles a dictionary tokenizer with its per-word LRU cache.
struct DictFts {
    fts: FtsTokenizer,
    cache: Mutex<LruCache<String, CachedLexeme>>,
}

impl DictFts {
    fn new(fts: FtsTokenizer) -> Self {
        let cap = NonZeroUsize::new(DICT_CACHE_CAP).unwrap();
        Self { fts, cache: Mutex::new(LruCache::new(cap)) }
    }
}

// ---------------------------------------------------------------------------
// Lazy DictFts instances — one per algorithm, shared across calls in a single
// backend process.  PG is multi-process so OnceLock is safe here.
// ---------------------------------------------------------------------------

static DICT_FTS: OnceLock<DictFts> = OnceLock::new();
static DICT_FTS_UDOM83: OnceLock<DictFts> = OnceLock::new();
static DICT_FTS_METASOUND: OnceLock<DictFts> = OnceLock::new();

fn dict_fts() -> &'static DictFts {
    DICT_FTS.get_or_init(|| {
        DictFts::new(
            FtsTokenizer::builder()
                .soundex(SoundexAlgorithm::Lk82)
                .romanization(RomanizationMap::builtin())
                .build(),
        )
    })
}

fn dict_fts_udom83() -> &'static DictFts {
    DICT_FTS_UDOM83.get_or_init(|| {
        DictFts::new(
            FtsTokenizer::builder()
                .soundex(SoundexAlgorithm::Udom83)
                .romanization(RomanizationMap::builtin())
                .build(),
        )
    })
}

fn dict_fts_metasound() -> &'static DictFts {
    DICT_FTS_METASOUND.get_or_init(|| {
        DictFts::new(
            FtsTokenizer::builder()
                .soundex(SoundexAlgorithm::MetaSound)
                .romanization(RomanizationMap::builtin())
                .build(),
        )
    })
}

// ---------------------------------------------------------------------------
// Core lexize logic (safe, called from unsafe impls)
// ---------------------------------------------------------------------------

/// Compute the expanded lexeme list for `word` using `fts` (no cache).
fn compute_lexeme(word: &str, fts: &FtsTokenizer) -> CachedLexeme {
    let word_owned = word.to_owned();
    let ft = match catch_unwind(std::panic::AssertUnwindSafe(|| {
        fts.segment_for_fts(&word_owned).into_iter().next()
    })) {
        Ok(Some(ft)) => ft,
        _ => return CachedLexeme::Words(vec![word.to_owned()]),
    };

    if ft.is_stop {
        return CachedLexeme::Stopword;
    }

    let mut lexemes: Vec<String> = Vec::with_capacity(6);
    lexemes.push(ft.text.clone());

    for syn in &ft.synonyms {
        if lexemes.len() >= 5 {
            break;
        }
        lexemes.push(syn.clone());
    }

    if let Some(pos) = ft.pos {
        if lexemes.len() < 6 {
            lexemes.push(format!("pos_{}", pos.as_tag().to_ascii_lowercase()));
        }
    }

    CachedLexeme::Words(lexemes)
}

/// Write a [`CachedLexeme`] into `out` and return the PG count value.
fn apply_lexeme(cached: &CachedLexeme, out: &mut KhamDictOut) -> c_int {
    match cached {
        CachedLexeme::Stopword => {
            out.count = 0;
            0
        }
        CachedLexeme::Words(lexemes) => {
            let count = lexemes.len();
            for (i, lex) in lexemes.iter().enumerate() {
                write_slot(&mut out.words[i], lex.as_bytes());
            }
            out.count = count as c_int;
            count as c_int
        }
    }
}

/// Expand `word` into up to 6 colocated lexemes, consulting the per-word cache
/// before calling `segment_for_fts`.
///
/// Returns 0 if the word is a stopword (PG drops it from the tsvector).
/// Returns the number of lexemes written into `out` otherwise (≥ 1).
fn dict_lexize_inner(word: &str, out: &mut KhamDictOut, dict: &DictFts) -> c_int {
    // Fast path: cache hit.
    {
        let mut cache = dict.cache.lock().unwrap();
        if let Some(cached) = cache.get(word) {
            return apply_lexeme(cached, out);
        }
    }

    // Cache miss: compute and store.
    let result = compute_lexeme(word, &dict.fts);
    {
        let mut cache = dict.cache.lock().unwrap();
        cache.put(word.to_owned(), result.clone());
    }

    apply_lexeme(&result, out)
}

/// Handle raw pointer args, decode UTF-8, delegate to [`dict_lexize_inner`].
///
/// # Safety
///
/// `token` must point to `token_len` valid UTF-8 bytes.
/// `out` must be a valid non-null pointer to a zeroed [`KhamDictOut`].
unsafe fn dict_lexize_raw(
    token: *const c_char,
    token_len: c_int,
    out: *mut KhamDictOut,
    dict: &DictFts,
) -> c_int {
    if token.is_null() || token_len <= 0 || out.is_null() {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(token as *const u8, token_len as usize) };
    let word = match std::str::from_utf8(bytes) {
        Ok(s) if !s.is_empty() => s,
        _ => return 0,
    };
    dict_lexize_inner(word, unsafe { &mut *out }, dict)
}

// ---------------------------------------------------------------------------
// Public impl functions called from shim.c
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn kham_dict_lexize_impl(
    token: *const c_char,
    token_len: c_int,
    out: *mut KhamDictOut,
) -> c_int {
    dict_lexize_raw(token, token_len, out, dict_fts())
}

#[no_mangle]
pub unsafe extern "C" fn kham_dict_lexize_udom83_impl(
    token: *const c_char,
    token_len: c_int,
    out: *mut KhamDictOut,
) -> c_int {
    dict_lexize_raw(token, token_len, out, dict_fts_udom83())
}

#[no_mangle]
pub unsafe extern "C" fn kham_dict_lexize_metasound_impl(
    token: *const c_char,
    token_len: c_int,
    out: *mut KhamDictOut,
) -> c_int {
    dict_lexize_raw(token, token_len, out, dict_fts_metasound())
}

// ---------------------------------------------------------------------------
// PostgreSQL fmgr exported symbols
//
// These #[no_mangle] functions are guaranteed to appear in the dynamic symbol
// table of the cdylib regardless of Rust's linker version-script.  They
// trampoline into the C shim functions (*_shim) that handle PG macro
// boilerplate, and provide the module magic and finfo records PG resolves
// via dlsym at extension load time.
// ---------------------------------------------------------------------------

type Datum = usize;
type Fcinfo = *mut c_void;

extern "C" {
    fn kham_pg_magic_impl() -> *const c_void;
    fn kham_start_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_gettoken_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_end_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_lextypes_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_headline_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_dict_lexize_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_dict_lexize_udom83_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_dict_lexize_metasound_shim(fcinfo: Fcinfo) -> Datum;
}

#[repr(C)]
pub struct PgFinfoRecord {
    api_version: c_int,
}

static FINFO_V1: PgFinfoRecord = PgFinfoRecord { api_version: 1 };

#[no_mangle]
pub unsafe extern "C" fn Pg_magic_func() -> *const c_void {
    kham_pg_magic_impl()
}

#[no_mangle]
pub unsafe extern "C" fn kham_start(fcinfo: Fcinfo) -> Datum {
    kham_start_shim(fcinfo)
}

#[no_mangle]
pub unsafe extern "C" fn kham_gettoken(fcinfo: Fcinfo) -> Datum {
    kham_gettoken_shim(fcinfo)
}

#[no_mangle]
pub unsafe extern "C" fn kham_end(fcinfo: Fcinfo) -> Datum {
    kham_end_shim(fcinfo)
}

#[no_mangle]
pub unsafe extern "C" fn kham_lextypes(fcinfo: Fcinfo) -> Datum {
    kham_lextypes_shim(fcinfo)
}

#[no_mangle]
pub unsafe extern "C" fn kham_headline(fcinfo: Fcinfo) -> Datum {
    kham_headline_shim(fcinfo)
}

#[no_mangle]
pub extern "C" fn pg_finfo_kham_start() -> *const PgFinfoRecord {
    &FINFO_V1
}
#[no_mangle]
pub extern "C" fn pg_finfo_kham_gettoken() -> *const PgFinfoRecord {
    &FINFO_V1
}
#[no_mangle]
pub extern "C" fn pg_finfo_kham_end() -> *const PgFinfoRecord {
    &FINFO_V1
}
#[no_mangle]
pub extern "C" fn pg_finfo_kham_lextypes() -> *const PgFinfoRecord {
    &FINFO_V1
}
#[no_mangle]
pub extern "C" fn pg_finfo_kham_headline() -> *const PgFinfoRecord {
    &FINFO_V1
}

#[no_mangle]
pub unsafe extern "C" fn kham_dict_lexize(fcinfo: Fcinfo) -> Datum {
    kham_dict_lexize_shim(fcinfo)
}

#[no_mangle]
pub extern "C" fn pg_finfo_kham_dict_lexize() -> *const PgFinfoRecord {
    &FINFO_V1
}

#[no_mangle]
pub unsafe extern "C" fn kham_dict_lexize_udom83(fcinfo: Fcinfo) -> Datum {
    kham_dict_lexize_udom83_shim(fcinfo)
}

#[no_mangle]
pub extern "C" fn pg_finfo_kham_dict_lexize_udom83() -> *const PgFinfoRecord {
    &FINFO_V1
}

#[no_mangle]
pub unsafe extern "C" fn kham_dict_lexize_metasound(fcinfo: Fcinfo) -> Datum {
    kham_dict_lexize_metasound_shim(fcinfo)
}

#[no_mangle]
pub extern "C" fn pg_finfo_kham_dict_lexize_metasound() -> *const PgFinfoRecord {
    &FINFO_V1
}

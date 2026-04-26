//! kham-pg — PostgreSQL text-search parser extension for Thai.
//!
//! PostgreSQL calls four C callbacks (`kham_start`, `kham_gettoken`,
//! `kham_end`, `kham_lextypes`) that are implemented in `shim.c`.  That shim
//! handles all fmgr macro boilerplate and then delegates to the three
//! `*_impl` functions defined here, which use [`FtsTokenizer`] to segment the
//! document text.
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

use std::os::raw::{c_char, c_int, c_void};
use std::panic::catch_unwind;
use std::sync::OnceLock;

use kham_core::fts::FtsTokenizer;
use kham_core::romanizer::RomanizationMap;
use kham_core::soundex::SoundexAlgorithm;
use kham_core::TokenKind;

// ---------------------------------------------------------------------------
// Parser state
// ---------------------------------------------------------------------------

/// Per-document parser state allocated in [`kham_start_impl`] and freed in
/// [`kham_end_impl`].
struct KhamState {
    /// Pre-computed tokens: UTF-8 bytes + PostgreSQL token-type integer.
    tokens: Vec<(Vec<u8>, c_int)>,
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
// Impl functions called from shim.c
// ---------------------------------------------------------------------------

/// Tokenise `text` (non-null-terminated, `len` bytes of UTF-8) and return an
/// opaque heap pointer to a [`KhamState`].
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

        let fts = FtsTokenizer::new();
        // segment_for_fts: normalise → segment → tag stopwords/synonyms.
        // We include ALL non-whitespace tokens so that PG dictionaries can
        // apply their own stopword / normalisation rules.
        let fts_tokens = fts.segment_for_fts(s);

        let tokens: Vec<(Vec<u8>, c_int)> = fts_tokens
            .into_iter()
            .filter(|t| t.kind != TokenKind::Whitespace)
            .map(|t| (t.text.into_bytes(), kind_to_pg_type(t.kind)))
            .collect();

        Box::into_raw(Box::new(KhamState { tokens, cursor: 0 })) as *mut c_void
    });

    result.unwrap_or(std::ptr::null_mut())
}

/// Write the next token into `*token` / `*tokenlen` and return its PG type.
///
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

    let (text, pg_type) = &state.tokens[state.cursor];
    state.cursor += 1;

    unsafe {
        *token = text.as_ptr() as *const c_char;
        *tokenlen = text.len() as c_int;
    }

    *pg_type
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
// Dictionary impl — kham_fts_dict
//
// A custom PG text-search dictionary that expands each Thai / Named token
// into three lexemes stored at the same tsvector position:
//   1. The normalised word itself
//   2. Its lk82 Thai Soundex code  (phonetic fuzzy search)
//   3. Its RTGS romanization       (Latin-script search, if in the map)
//
// This mirrors the FTS5_TOKEN_COLOCATED approach used in kham-sqlite.
// ---------------------------------------------------------------------------

/// Up to 6 lexeme slots, each holding a null-terminated UTF-8 string (≤ 127 bytes).
/// Filled by [`kham_dict_lexize_impl`] and consumed by `kham_dict_lexize_shim` in C.
#[repr(C)]
pub struct KhamDictOut {
    pub count: c_int,
    pub words: [[u8; 128]; 6],
}

/// Lazy-initialised `FtsTokenizer` shared across all `kham_fts_dict` calls
/// within one backend process.  Each PG backend gets its own copy (PG is
/// multi-process, not multi-thread), so a process-local `OnceLock` is safe.
static DICT_FTS: OnceLock<FtsTokenizer> = OnceLock::new();

fn dict_fts() -> &'static FtsTokenizer {
    DICT_FTS.get_or_init(|| {
        FtsTokenizer::builder()
            .soundex(SoundexAlgorithm::Lk82)
            .romanization(RomanizationMap::builtin())
            .build()
    })
}

fn write_slot(slot: &mut [u8; 128], src: &[u8]) {
    let n = src.len().min(127);
    slot[..n].copy_from_slice(&src[..n]);
    slot[n] = 0;
}

/// Expand `token` into lexemes: `[word, soundex_code?, rtgs?]`.
///
/// Writes results into `out` and sets `out.count`.  Returns `out.count`.
/// Returns `0` on error or if the token should be treated as a stopword.
///
/// # Safety
///
/// `token` must point to `token_len` valid UTF-8 bytes.
/// `out` must be a valid non-null pointer to a zeroed [`KhamDictOut`].
#[no_mangle]
pub unsafe extern "C" fn kham_dict_lexize_impl(
    token: *const c_char,
    token_len: c_int,
    out: *mut KhamDictOut,
) -> c_int {
    if token.is_null() || token_len <= 0 || out.is_null() {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(token as *const u8, token_len as usize) };
    let word = match std::str::from_utf8(bytes) {
        Ok(s) if !s.is_empty() => s,
        _ => return 0,
    };

    let out_ref = unsafe { &mut *out };

    // Slot 0: the word itself (always returned — never a stopword).
    write_slot(&mut out_ref.words[0], word.as_bytes());
    out_ref.count = 1;

    // Slots 1-5: soundex + RTGS synonyms.  Any panic is caught so that at
    // minimum the bare word is still indexed.
    let word_owned = word.to_owned();
    let synonyms: Vec<String> = catch_unwind(std::panic::AssertUnwindSafe(|| {
        dict_fts()
            .segment_for_fts(&word_owned)
            .into_iter()
            .next()
            .map(|ft| ft.synonyms)
            .unwrap_or_default()
    }))
    .unwrap_or_default();

    let mut count: usize = 1;
    for syn in &synonyms {
        if count >= 6 {
            break;
        }
        write_slot(&mut out_ref.words[count], syn.as_bytes());
        count += 1;
    }
    out_ref.count = count as c_int;
    count as c_int
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

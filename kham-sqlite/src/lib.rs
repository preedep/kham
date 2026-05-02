//! kham-sqlite — SQLite FTS5 tokenizer extension for Thai.
//!
//! Registers a custom FTS5 tokenizer named `kham` that provides:
//! - Thai word segmentation (newmm DAG algorithm)
//! - Text normalization (สระลอย reorder, วรรณยุกต์ dedup, Sara Am composition)
//! - Named entity recognition (places, persons, organisations)
//! - Synonym expansion via `FTS5_TOKEN_COLOCATED` (configurable TSV map)
//! - RTGS romanization added as colocated synonyms (กิน → "kin")
//! - Thai phonetic soundex codes as colocated synonyms for fuzzy matching
//! - Thai digit → ASCII normalization (๑๒๓ indexed as "123" via colocated synonym)
//! - POS lexeme colocated tokens (e.g. `posnoun`, `posverb`) for POS filtering
//!
//! ```sql
//! SELECT load_extension('./libkham_sqlite', 'sqlite3_kham_init');
//! -- Default tokenizer (lk82 soundex, stopwords forwarded, trigrams for unknown)
//! CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham');
//! -- Suppress stopwords from the index
//! CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham stopwords on');
//! -- Bigrams instead of trigrams for unknown tokens
//! CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham ngram_size 2');
//! -- Disable n-grams entirely
//! CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham ngram_size 0');
//! -- All options combined
//! CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham soundex udom83 stopwords on ngram_size 4');
//! -- Disable soundex
//! CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham soundex none');
//! INSERT INTO docs VALUES ('กินข้าวกับปลา');
//! SELECT * FROM docs WHERE docs MATCH 'ปลา';
//! SELECT snippet(docs, 0, '<b>', '</b>', '...', 5) FROM docs WHERE docs MATCH 'ปลา';
//! ```
//!
//! ## Normalization and byte offsets
//!
//! `xToken(iStart, iEnd)` reports byte offsets into the **normalized** form of
//! the input text.  `snippet()` and `highlight()` are accurate when documents
//! are stored in normalized form.  For documents with stacked tone marks or
//! unresolved Sara Am, offsets may shift by a few bytes in those spans.
//!
//! ## Architecture
//!
//! ```text
//! SQLite FTS5  ──▶  src/shim.c (C)  ──▶  lib.rs (Rust entry points + callbacks)
//!                   SQLITE_EXTENSION_INIT1/2           │
//!                   kham_sqlite_setup_api()             ▼
//!                   kham_sqlite_get_fts5api()  sqlite3_kham_init / sqlite3_khamsqlite_init
//!                                                       │
//!                                                       ▼
//!                                              xCreate → KhamInstance (cached FtsTokenizer)
//!                                                       │
//!                                                       ▼
//!                                              xTokenize → normalize → segment_for_fts
//!                                                                → xToken (primary)
//!                                                                → xToken FTS5_TOKEN_COLOCATED
//!                                                                         (synonyms + RTGS)
//! ```
//!
//! ## Instance layout
//!
//! `KhamInstance` uses the "C inheritance" pattern: the vtable struct
//! (`KhamFts5Tokenizer`) is the first field, so a `*mut KhamInstance` is
//! pointer-compatible with `*mut KhamFts5Tokenizer`.  SQLite stores and passes
//! back the same pointer it received from `xCreate`, so `xTokenize`/`xDelete`
//! can cast it to `*mut KhamInstance` and access the cached `FtsTokenizer`.
//!
//! `FtsTokenizer::new()` is called once per FTS5 table (in `xCreate`), not per
//! document or query.
//!
//! ## unsafe policy
//!
//! `unsafe` is confined to this file (FFI boundary). `src/shim.c` is plain C.

#![allow(clippy::missing_safety_doc)]

use std::os::raw::{c_char, c_int, c_void};
use std::panic::catch_unwind;

use kham_core::fts::FtsTokenizer;
use kham_core::normalizer;
use kham_core::romanizer::RomanizationMap;
use kham_core::soundex::SoundexAlgorithm;
use kham_core::synonym::SynonymMap;

// ---------------------------------------------------------------------------
// SQLite / FTS5 constants
// ---------------------------------------------------------------------------

const SQLITE_OK: c_int = 0;
const SQLITE_ERROR: c_int = 1;

/// Flag passed to `xToken` to emit a colocated synonym at the same position.
const FTS5_TOKEN_COLOCATED: c_int = 0x0001;

// ---------------------------------------------------------------------------
// FTS5 type definitions — must match sqlite3.h layout exactly.
// ---------------------------------------------------------------------------

/// Callback supplied by SQLite to receive individual tokens.
type XTokenFn = unsafe extern "C" fn(
    p_ctx: *mut c_void,
    t_flags: c_int,
    p_token: *const c_char,
    n_token: c_int,
    i_start: c_int,
    i_end: c_int,
) -> c_int;

/// Matches `struct fts5_tokenizer` in `sqlite3.h` (legacy v1 interface).
///
/// **Must be the first field of `KhamInstance`** — see module-level docs.
#[repr(C)]
pub struct KhamFts5Tokenizer {
    x_create: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const *const c_char,
            c_int,
            *mut *mut KhamFts5Tokenizer,
        ) -> c_int,
    >,
    x_delete: Option<unsafe extern "C" fn(*mut KhamFts5Tokenizer)>,
    x_tokenize: Option<
        unsafe extern "C" fn(
            *mut KhamFts5Tokenizer,
            *mut c_void,
            c_int,
            *const c_char,
            c_int,
            XTokenFn,
        ) -> c_int,
    >,
}

/// Truncated view of `struct fts5_api` — only the first two fields accessed.
#[repr(C)]
struct KhamFts5Api {
    i_version: c_int,
    x_create_tokenizer: Option<
        unsafe extern "C" fn(
            *mut KhamFts5Api,
            *const c_char,
            *mut c_void,
            *const KhamFts5Tokenizer,
            Option<unsafe extern "C" fn(*mut c_void)>,
        ) -> c_int,
    >,
}

// ---------------------------------------------------------------------------
// Per-instance state
// ---------------------------------------------------------------------------

/// Per-FTS5-table tokenizer instance.
///
/// `vtable` **must be the first field** so that a `*mut KhamInstance` is
/// pointer-compatible with SQLite's `*mut Fts5Tokenizer`.  SQLite only
/// sees and passes back the first-field pointer; the `fts` field is
/// invisible to it and lives behind the pointer.
#[repr(C)]
struct KhamInstance {
    vtable: KhamFts5Tokenizer,
    fts: FtsTokenizer,
    /// When `true`, tokens marked `is_stop` are not forwarded to SQLite.
    suppress_stopwords: bool,
}

// ---------------------------------------------------------------------------
// xCreate argument parsing
// ---------------------------------------------------------------------------

/// Parse soundex algorithm from the `xCreate` argument list.
///
/// FTS5 passes `tokenize='kham soundex lk82'` as positional space-separated
/// arguments: `az_arg[0]="kham"`, `az_arg[1]="soundex"`, `az_arg[2]="lk82"`.
/// This function scans for the keyword `"soundex"` and reads the next argument
/// as the algorithm name.
///
/// Supported algorithm values: `"lk82"` (default), `"udom83"`, `"metasound"`, `"none"`.
/// Unknown values fall back to lk82.
fn parse_soundex_arg(az_arg: *const *const c_char, n_arg: c_int) -> Option<SoundexAlgorithm> {
    let n = n_arg as usize;
    // FTS5 strips the tokenizer name ("kham") before calling xCreate, so az_arg[0]
    // is the first user-supplied argument (e.g. "soundex").
    let mut i = 0usize;
    while i < n {
        let ptr = unsafe { *az_arg.add(i) };
        i += 1;
        if ptr.is_null() {
            continue;
        }
        let s = match unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if s == "soundex" {
            // Next argument is the algorithm name.
            let val_ptr = if i < n {
                unsafe { *az_arg.add(i) }
            } else {
                std::ptr::null()
            };
            if val_ptr.is_null() {
                return Some(SoundexAlgorithm::Lk82);
            }
            let val = match unsafe { std::ffi::CStr::from_ptr(val_ptr) }.to_str() {
                Ok(s) => s,
                Err(_) => return Some(SoundexAlgorithm::Lk82),
            };
            return match val {
                "lk82" => Some(SoundexAlgorithm::Lk82),
                "udom83" => Some(SoundexAlgorithm::Udom83),
                "metasound" => Some(SoundexAlgorithm::MetaSound),
                "none" => None,
                _ => Some(SoundexAlgorithm::Lk82),
            };
        }
    }
    Some(SoundexAlgorithm::Lk82) // default when no soundex argument
}

/// Return `true` when `az_arg` contains the pair `"stopwords" "on"`.
///
/// Default: `false` — stopwords are forwarded (backward-compatible).
/// Usage: `tokenize='kham stopwords on'`.
fn parse_stopwords_arg(az_arg: *const *const c_char, n_arg: c_int) -> bool {
    let n = n_arg as usize;
    let mut i = 0usize;
    while i < n {
        let ptr = unsafe { *az_arg.add(i) };
        i += 1;
        if ptr.is_null() {
            continue;
        }
        let s = match unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if s == "stopwords" {
            let val_ptr = if i < n {
                unsafe { *az_arg.add(i) }
            } else {
                std::ptr::null()
            };
            if val_ptr.is_null() {
                return false;
            }
            return matches!(
                unsafe { std::ffi::CStr::from_ptr(val_ptr) }.to_str(),
                Ok("on")
            );
        }
    }
    false
}

/// Parse a file path from the `synonyms <path>` argument pair in `az_arg`.
///
/// Returns the path string when the keyword `"synonyms"` is found followed by
/// a non-null argument.  Returns `None` when the keyword is absent.
///
/// Usage: `tokenize='kham synonyms /path/to/synonyms.tsv'`
fn parse_synonyms_path(az_arg: *const *const c_char, n_arg: c_int) -> Option<String> {
    let n = n_arg as usize;
    let mut i = 0usize;
    while i < n {
        let ptr = unsafe { *az_arg.add(i) };
        i += 1;
        if ptr.is_null() {
            continue;
        }
        let s = match unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if s == "synonyms" {
            let val_ptr = if i < n {
                unsafe { *az_arg.add(i) }
            } else {
                std::ptr::null()
            };
            if val_ptr.is_null() {
                return None;
            }
            return unsafe { std::ffi::CStr::from_ptr(val_ptr) }
                .to_str()
                .ok()
                .map(|p| p.to_string());
        }
    }
    None
}

/// Parse a file path from the `dict <path>` argument pair in `az_arg`.
///
/// Returns the path string when the keyword `"dict"` is found followed by a
/// non-null argument.  Returns `None` when the keyword is absent.
///
/// Usage: `tokenize='kham dict ''/path/to/words.txt'''`
fn parse_dict_path(az_arg: *const *const c_char, n_arg: c_int) -> Option<String> {
    let n = n_arg as usize;
    let mut i = 0usize;
    while i < n {
        let ptr = unsafe { *az_arg.add(i) };
        i += 1;
        if ptr.is_null() {
            continue;
        }
        let s = match unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if s == "dict" {
            let val_ptr = if i < n {
                unsafe { *az_arg.add(i) }
            } else {
                std::ptr::null()
            };
            if val_ptr.is_null() {
                return None;
            }
            return unsafe { std::ffi::CStr::from_ptr(val_ptr) }
                .to_str()
                .ok()
                .map(|p| p.to_string());
        }
    }
    None
}

/// Parse n-gram size from the `xCreate` argument list.
///
/// Scans for the keyword `"ngram_size"` and parses the next argument as a
/// decimal `usize`. Falls back to `3` (trigrams) on missing or invalid value.
/// Set to `0` to disable n-gram generation for unknown tokens entirely.
///
/// Usage: `tokenize='kham ngram_size 2'` (bigrams).
fn parse_ngram_size_arg(az_arg: *const *const c_char, n_arg: c_int) -> usize {
    let n = n_arg as usize;
    let mut i = 0usize;
    while i < n {
        let ptr = unsafe { *az_arg.add(i) };
        i += 1;
        if ptr.is_null() {
            continue;
        }
        let s = match unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if s == "ngram_size" {
            let val_ptr = if i < n {
                unsafe { *az_arg.add(i) }
            } else {
                std::ptr::null()
            };
            if val_ptr.is_null() {
                return 3;
            }
            return match unsafe { std::ffi::CStr::from_ptr(val_ptr) }.to_str() {
                Ok(v) => v.parse::<usize>().unwrap_or(3),
                Err(_) => 3,
            };
        }
    }
    3 // default: trigrams
}

// ---------------------------------------------------------------------------
// FTS5 tokenizer callbacks
// ---------------------------------------------------------------------------

/// Allocate a new per-table tokenizer instance, building the `FtsTokenizer` once.
///
/// Accepted argument pairs (all optional, order-independent):
/// - `soundex <lk82|udom83|metasound|none>` — phonetic algorithm (default: lk82)
/// - `stopwords <on|off>` — suppress stopword tokens from the index (default: off)
/// - `ngram_size <N>` — n-gram size for unknown tokens (default: 3; 0 = disabled)
/// - `synonyms <path>` — path to a TSV synonym map (`canonical\tsyn1\tsyn2…`);
///   **the path must be single-quoted** because `/`, `.`, and `-` are not FTS5
///   bareword characters — use `''` to escape quotes inside a SQL string:
///   `tokenize='kham synonyms ''/path/to/synonyms.tsv'''`
/// - `dict <path>` — path to a newline-separated word list to overlay on the
///   built-in dictionary; same quoting rules apply:
///   `tokenize='kham dict ''/etc/kham/domain_words.txt'''`
///
/// Examples:
/// - `tokenize='kham'`
/// - `tokenize='kham soundex udom83'`
/// - `tokenize='kham stopwords on'`
/// - `tokenize='kham ngram_size 2'`
/// - `tokenize='kham synonyms ''/etc/kham/synonyms.tsv'''`
/// - `tokenize='kham dict ''/etc/kham/domain_words.txt'''`
/// - `tokenize='kham soundex lk82 stopwords on ngram_size 4'`
unsafe extern "C" fn kham_fts5_create(
    _p_ctx: *mut c_void,
    az_arg: *const *const c_char,
    n_arg: c_int,
    pp_out: *mut *mut KhamFts5Tokenizer,
) -> c_int {
    if pp_out.is_null() {
        return SQLITE_ERROR;
    }
    let soundex_algo = parse_soundex_arg(az_arg, n_arg);
    let suppress_stopwords = parse_stopwords_arg(az_arg, n_arg);
    let ngram_size = parse_ngram_size_arg(az_arg, n_arg);
    let mut builder = FtsTokenizer::builder()
        .romanization(RomanizationMap::builtin())
        .number_normalize(true)
        .ngram_size(ngram_size);
    if let Some(algo) = soundex_algo {
        builder = builder.soundex(algo);
    }
    if let Some(path) = parse_synonyms_path(az_arg, n_arg) {
        match std::fs::read_to_string(&path) {
            Ok(data) => builder = builder.synonyms(SynonymMap::from_tsv(&data)),
            Err(_) => return SQLITE_ERROR,
        }
    }
    if let Some(path) = parse_dict_path(az_arg, n_arg) {
        match std::fs::read_to_string(&path) {
            Ok(data) => builder = builder.dict_merge(&data),
            Err(_) => return SQLITE_ERROR,
        }
    }
    let instance = Box::new(KhamInstance {
        vtable: KhamFts5Tokenizer {
            x_create: Some(kham_fts5_create),
            x_delete: Some(kham_fts5_delete),
            x_tokenize: Some(kham_fts5_tokenize),
        },
        // Full NLP pipeline built once per FTS5 table: segmenter + NE + synonyms + RTGS + soundex.
        fts: builder.build(),
        suppress_stopwords,
    });
    // SAFETY: vtable is the first field of KhamInstance (#[repr(C)]),
    // so *mut KhamInstance and *mut KhamFts5Tokenizer alias the same address.
    *pp_out = Box::into_raw(instance) as *mut KhamFts5Tokenizer;
    SQLITE_OK
}

/// Free the [`KhamInstance`] allocated by [`kham_fts5_create`].
unsafe extern "C" fn kham_fts5_delete(p: *mut KhamFts5Tokenizer) {
    if !p.is_null() {
        // SAFETY: p was originally a *mut KhamInstance cast to *mut KhamFts5Tokenizer.
        drop(Box::from_raw(p as *mut KhamInstance));
    }
}

/// Tokenise `p_text[0..n_text]` and report each token to SQLite via `x_token`.
///
/// Pipeline per call:
/// 1. Normalise input (สระลอย, วรรณยุกต์ dedup, Sara Am) — offsets reference normalized text.
/// 2. Run the full FTS pipeline (`segment_for_fts`): segment → NE tag → stopword → POS →
///    synonym expand → RTGS romanization → number normalization.
/// 3. For each non-whitespace token, call `x_token(flags=0, iStart, iEnd)`.
/// 4. For each synonym/RTGS/soundex/number form, call `x_token(FTS5_TOKEN_COLOCATED, …)`.
/// 5. For n-grams (Unknown tokens), call `x_token(FTS5_TOKEN_COLOCATED, …)` for each gram.
/// 6. For POS-tagged tokens, call `x_token(FTS5_TOKEN_COLOCATED, …)` with `"pos<tag>"`.
///    Enables: `WHERE docs MATCH 'กิน AND posverb'`.
unsafe extern "C" fn kham_fts5_tokenize(
    p: *mut KhamFts5Tokenizer,
    p_ctx: *mut c_void,
    _flags: c_int,
    p_text: *const c_char,
    n_text: c_int,
    x_token: XTokenFn,
) -> c_int {
    let result = catch_unwind(|| {
        // Recover the cached FtsTokenizer from the instance.
        // SAFETY: p is a *mut KhamInstance cast to *mut KhamFts5Tokenizer by xCreate.
        let instance = unsafe { &*(p as *mut KhamInstance) };

        // Build a &str over the input buffer (valid for the duration of this call).
        let text = if n_text < 0 {
            match unsafe { std::ffi::CStr::from_ptr(p_text) }.to_str() {
                Ok(s) => s,
                Err(_) => return SQLITE_OK,
            }
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(p_text as *const u8, n_text as usize) };
            match std::str::from_utf8(bytes) {
                Ok(s) => s,
                Err(_) => return SQLITE_OK,
            }
        };

        // Normalize once; FtsTokenizer::segment_for_fts normalizes internally too —
        // both paths call the same normalizer::normalize(), so the results match.
        // We need the normalized string to compute accurate byte offsets.
        let normalized = normalizer::normalize(text);

        // Full FTS pipeline: segment → NE merge → stopword tag → POS → synonyms → RTGS.
        let fts_tokens = instance.fts.segment_for_fts(text);

        // Forward cursor into `normalized` for offset computation.
        let mut scan: usize = 0;

        for ft in &fts_tokens {
            // segment_for_fts already excludes Whitespace, but guard defensively.
            let tok_bytes = ft.text.as_bytes();
            if tok_bytes.is_empty() {
                continue;
            }

            // Locate this token's byte span in the normalized text, advancing the cursor.
            // ft.text is a slice of the normalized form, so find() is always successful
            // unless NE merging produced a text that spans a whitespace gap.
            let (i_start, i_end) = match normalized[scan..].find(ft.text.as_str()) {
                Some(rel) => {
                    let start = scan + rel;
                    let end = start + tok_bytes.len();
                    scan = end;
                    (start as c_int, end as c_int)
                }
                None => {
                    // Fallback: use scan position as a point span (degenerate case).
                    let pos = scan.min(normalized.len()) as c_int;
                    (pos, pos)
                }
            };

            // Skip stopwords when suppression is enabled.
            if instance.suppress_stopwords && ft.is_stop {
                continue;
            }

            // Emit primary token.
            // SAFETY: tok_bytes points into ft.text which is alive for this loop body.
            let rc = unsafe {
                x_token(
                    p_ctx,
                    0,
                    tok_bytes.as_ptr() as *const c_char,
                    tok_bytes.len() as c_int,
                    i_start,
                    i_end,
                )
            };
            if rc != SQLITE_OK {
                return rc;
            }

            // Emit colocated synonyms (synonym map + RTGS romanization + soundex).
            for syn in &ft.synonyms {
                let syn_bytes = syn.as_bytes();
                if syn_bytes.is_empty() {
                    continue;
                }
                let rc = unsafe {
                    x_token(
                        p_ctx,
                        FTS5_TOKEN_COLOCATED,
                        syn_bytes.as_ptr() as *const c_char,
                        syn_bytes.len() as c_int,
                        i_start,
                        i_end,
                    )
                };
                if rc != SQLITE_OK {
                    return rc;
                }
            }

            // Emit char n-grams as colocated synonyms (populated for Unknown tokens only).
            for tri in &ft.trigrams {
                let tri_bytes = tri.as_bytes();
                if tri_bytes.is_empty() {
                    continue;
                }
                let rc = unsafe {
                    x_token(
                        p_ctx,
                        FTS5_TOKEN_COLOCATED,
                        tri_bytes.as_ptr() as *const c_char,
                        tri_bytes.len() as c_int,
                        i_start,
                        i_end,
                    )
                };
                if rc != SQLITE_OK {
                    return rc;
                }
            }

            // Emit POS lexeme as colocated token (e.g. "posnoun", "posverb").
            // Uses concatenated form (no underscore) so the kham tokenizer treats it
            // as a single Latin token in queries: WHERE docs MATCH 'posverb'.
            // Note: kham-pg uses pos_noun (tsquery doesn't re-tokenize terms), but
            // FTS5 tokenizes the MATCH argument with the same kham tokenizer, so
            // pos_noun would split into three tokens — concatenated form avoids this.
            if let Some(pos) = ft.pos {
                let pos_lexeme = format!("pos{}", pos.as_tag().to_ascii_lowercase());
                let pos_bytes = pos_lexeme.as_bytes();
                let rc = unsafe {
                    x_token(
                        p_ctx,
                        FTS5_TOKEN_COLOCATED,
                        pos_bytes.as_ptr() as *const c_char,
                        pos_bytes.len() as c_int,
                        i_start,
                        i_end,
                    )
                };
                if rc != SQLITE_OK {
                    return rc;
                }
            }
        }

        SQLITE_OK
    });

    result.unwrap_or(SQLITE_ERROR)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Factory template passed to `xCreateTokenizer`.  SQLite copies the function
/// pointers from it; the static never acts as a tokenizer instance itself.
static KHAM_TOKENIZER: KhamFts5Tokenizer = KhamFts5Tokenizer {
    x_create: Some(kham_fts5_create),
    x_delete: Some(kham_fts5_delete),
    x_tokenize: Some(kham_fts5_tokenize),
};

fn register_tokenizer(fts5_api_ptr: *mut c_void) -> c_int {
    let api = fts5_api_ptr as *mut KhamFts5Api;
    if api.is_null() {
        return SQLITE_ERROR;
    }
    let x_create = match unsafe { (*api).x_create_tokenizer } {
        Some(f) => f,
        None => return SQLITE_ERROR,
    };
    let name = c"kham".as_ptr();
    unsafe { x_create(api, name, std::ptr::null_mut(), &KHAM_TOKENIZER, None) }
}

// ---------------------------------------------------------------------------
// C helpers from shim.c
// ---------------------------------------------------------------------------

extern "C" {
    fn kham_sqlite_setup_api(p_api: *const c_void);
    fn kham_sqlite_get_fts5api(db: *mut c_void) -> *mut c_void;
}

// ---------------------------------------------------------------------------
// Extension entry points — #[no_mangle] guarantees presence in dylib symbol table
// ---------------------------------------------------------------------------

unsafe fn do_init(db: *mut c_void, p_api: *const c_void) -> c_int {
    kham_sqlite_setup_api(p_api);
    let fts5 = kham_sqlite_get_fts5api(db);
    if fts5.is_null() {
        return SQLITE_ERROR;
    }
    register_tokenizer(fts5)
}

/// Explicit entry point: `load_extension('libkham_sqlite', 'sqlite3_kham_init')`
#[no_mangle]
pub unsafe extern "C" fn sqlite3_kham_init(
    db: *mut c_void,
    _p_err_msg: *mut *mut c_char,
    p_api: *const c_void,
) -> c_int {
    do_init(db, p_api)
}

/// Implicit entry point (SQLite derives from filename by stripping underscores):
/// `load_extension('libkham_sqlite')`
#[no_mangle]
pub unsafe extern "C" fn sqlite3_khamsqlite_init(
    db: *mut c_void,
    _p_err_msg: *mut *mut c_char,
    p_api: *const c_void,
) -> c_int {
    do_init(db, p_api)
}

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

use kham_core::fts::FtsTokenizer;
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
        // TODO: map to type 7 once SQL install script adds the "named" lextypes entry
        TokenKind::Named(_) => 1,
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

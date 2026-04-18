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

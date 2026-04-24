//! kham-sqlite — SQLite FTS5 tokenizer extension for Thai.
//!
//! This crate exposes a loadable SQLite extension that registers a custom FTS5
//! tokenizer named `kham`.  After loading, create an FTS5 table with:
//!
//! ```sql
//! SELECT load_extension('./libkham_sqlite');
//! CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham');
//! INSERT INTO docs VALUES ('กินข้าวกับปลา');
//! SELECT * FROM docs WHERE docs MATCH 'ปลา';
//! ```
//!
//! ## Architecture
//!
//! ```text
//! SQLite FTS5  ──▶  src/shim.c (C)  ──▶  kham_register_tokenizer() (Rust)
//!                   SQLITE_EXTENSION_INIT1/2              │
//!                   get_fts5_api()                        ▼
//!                   sqlite3_kham_init()          xCreate / xDelete / xTokenize
//!                                                         │
//!                                                         ▼
//!                                                kham_core::Tokenizer::segment()
//! ```
//!
//! ## Token lifecycle
//!
//! For each FTS5 table, SQLite calls `xCreate` once to allocate a
//! [`KhamFts5Tokenizer`] instance and `xDelete` when the table is dropped.
//! `xTokenize` is called for every document indexed and every query tokenised.
//!
//! ## unsafe policy
//!
//! `unsafe` is confined to this file (FFI boundary). `src/shim.c` is plain C.

#![allow(clippy::missing_safety_doc)]

use std::os::raw::{c_char, c_int, c_void};
use std::panic::catch_unwind;

use kham_core::{TokenKind, Tokenizer};

// ---------------------------------------------------------------------------
// SQLite / FTS5 constants
// ---------------------------------------------------------------------------

const SQLITE_OK: c_int = 0;
const SQLITE_ERROR: c_int = 1;

// ---------------------------------------------------------------------------
// FTS5 type definitions — must match sqlite3.h layout exactly.
//
// We define only the fields we access.  Receiving a *mut via pointer means
// the actual struct size does not matter — only field offsets must align,
// which #[repr(C)] guarantees by following C's alignment rules.
// ---------------------------------------------------------------------------

/// Callback supplied by SQLite to receive individual tokens.
///
/// `p_ctx`   — opaque context forwarded from `xTokenize`
/// `t_flags` — `FTS5_TOKEN_COLOCATED` (0x1) for synonym/variant tokens
/// `p_token` — pointer to token bytes (need not be NUL-terminated)
/// `n_token` — byte length of token
/// `i_start` — byte offset of token start in the original document
/// `i_end`   — byte offset one past the token end in the original document
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
/// New code may use `fts5_tokenizer_v2` (with locale support), but the v1
/// interface is sufficient for Thai and keeps the implementation simpler.
#[repr(C)]
pub struct KhamFts5Tokenizer {
    x_create: Option<
        unsafe extern "C" fn(
            *mut c_void,                 // pUserData passed to xCreateTokenizer
            *const *const c_char,        // azArg  (tokenizer arguments)
            c_int,                       // nArg
            *mut *mut KhamFts5Tokenizer, // ppOut — caller sets *ppOut
        ) -> c_int,
    >,
    x_delete: Option<unsafe extern "C" fn(*mut KhamFts5Tokenizer)>,
    x_tokenize: Option<
        unsafe extern "C" fn(
            *mut KhamFts5Tokenizer,
            *mut c_void,   // pCtx
            c_int,         // flags (FTS5_TOKENIZE_*)
            *const c_char, // pText
            c_int,         // nText  (-1 = NUL-terminated)
            XTokenFn,      // xToken callback
        ) -> c_int,
    >,
}

/// Truncated view of `struct fts5_api` in `sqlite3.h`.
///
/// The actual struct has 5 fields; we only access the first two
/// (`iVersion` and `xCreateTokenizer`), so we define only those.
#[repr(C)]
struct KhamFts5Api {
    i_version: c_int,
    x_create_tokenizer: Option<
        unsafe extern "C" fn(
            *mut KhamFts5Api,
            *const c_char,                             // zName
            *mut c_void,                               // pUserData
            *const KhamFts5Tokenizer,                  // pTokenizer (SQLite copies function ptrs)
            Option<unsafe extern "C" fn(*mut c_void)>, // xDestroy for pUserData
        ) -> c_int,
    >,
}

// ---------------------------------------------------------------------------
// FTS5 tokenizer callbacks
// ---------------------------------------------------------------------------

/// Allocate a new per-table tokenizer instance.
///
/// kham has no per-instance configuration, so we just heap-allocate a copy of
/// the tokenizer struct (SQLite will call `xDelete` to free it).
unsafe extern "C" fn kham_fts5_create(
    _p_ctx: *mut c_void,
    _az_arg: *const *const c_char,
    _n_arg: c_int,
    pp_out: *mut *mut KhamFts5Tokenizer,
) -> c_int {
    if pp_out.is_null() {
        return SQLITE_ERROR;
    }
    let instance = Box::new(KhamFts5Tokenizer {
        x_create: Some(kham_fts5_create),
        x_delete: Some(kham_fts5_delete),
        x_tokenize: Some(kham_fts5_tokenize),
    });
    *pp_out = Box::into_raw(instance);
    SQLITE_OK
}

/// Free a tokenizer instance created by [`kham_fts5_create`].
unsafe extern "C" fn kham_fts5_delete(p: *mut KhamFts5Tokenizer) {
    if !p.is_null() {
        drop(Box::from_raw(p));
    }
}

/// Tokenise `p_text[0..n_text]` and report each token to SQLite via `x_token`.
///
/// Uses [`kham_core::Tokenizer`] which returns zero-copy `Token<'_>` slices
/// with byte-offset spans — exactly what SQLite's `xToken(iStart, iEnd)` needs.
///
/// Whitespace tokens are suppressed (the default `Tokenizer::new()` already
/// drops them).  All other token kinds (Thai, Latin, Number, Punctuation,
/// Emoji, Unknown, Named) are forwarded so SQLite's FTS5 engine can apply its
/// own token-type filters if desired.
unsafe extern "C" fn kham_fts5_tokenize(
    _p: *mut KhamFts5Tokenizer,
    p_ctx: *mut c_void,
    _flags: c_int,
    p_text: *const c_char,
    n_text: c_int,
    x_token: XTokenFn,
) -> c_int {
    let result = catch_unwind(|| {
        // Build a &str over the input buffer.  SQLite guarantees the buffer is
        // valid for the duration of this call.  n_text == -1 means NUL-terminated.
        let text = if n_text < 0 {
            // SAFETY: caller guarantees NUL-terminated UTF-8
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

        // `Tokenizer::new()` has keep_whitespace = false, so the returned
        // Vec contains only non-whitespace tokens.
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.segment(text);

        for token in &tokens {
            // Guard: skip any residual whitespace (defensive, should be none)
            if token.kind == TokenKind::Whitespace {
                continue;
            }

            let tok_bytes = token.text.as_bytes();
            let i_start = token.span.start as c_int;
            let i_end = token.span.end as c_int;

            // SAFETY: tok_bytes points into the original `text` buffer which
            // is valid for this entire call.
            let rc = unsafe {
                x_token(
                    p_ctx,
                    0, // no FTS5_TOKEN_COLOCATED flag for primary tokens
                    tok_bytes.as_ptr() as *const c_char,
                    tok_bytes.len() as c_int,
                    i_start,
                    i_end,
                )
            };
            if rc != SQLITE_OK {
                return rc;
            }
        }

        SQLITE_OK
    });

    result.unwrap_or(SQLITE_ERROR)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Factory template; SQLite copies function pointers from this when
/// `xCreateTokenizer` is called, so it only needs to live until that call returns.
static KHAM_TOKENIZER: KhamFts5Tokenizer = KhamFts5Tokenizer {
    x_create: Some(kham_fts5_create),
    x_delete: Some(kham_fts5_delete),
    x_tokenize: Some(kham_fts5_tokenize),
};

/// Register the `kham` FTS5 tokenizer with the given `fts5_api*`.
fn register_tokenizer(fts5_api_ptr: *mut c_void) -> c_int {
    let api = fts5_api_ptr as *mut KhamFts5Api;
    if api.is_null() {
        return SQLITE_ERROR;
    }
    let x_create = match unsafe { (*api).x_create_tokenizer } {
        Some(f) => f,
        None => return SQLITE_ERROR,
    };
    let name = b"kham\0".as_ptr() as *const c_char;
    unsafe { x_create(api, name, std::ptr::null_mut(), &KHAM_TOKENIZER, None) }
}

// ---------------------------------------------------------------------------
// C helpers from shim.c
// ---------------------------------------------------------------------------

extern "C" {
    /// Store the SQLite API vtable (wraps SQLITE_EXTENSION_INIT2).
    fn kham_sqlite_setup_api(p_api: *const c_void);
    /// Return the fts5_api* for `db` via the bind-pointer trick.
    fn kham_sqlite_get_fts5api(db: *mut c_void) -> *mut c_void;
}

// ---------------------------------------------------------------------------
// Extension entry points
//
// Both symbols are #[no_mangle] so they are guaranteed to appear in the
// cdylib symbol table regardless of linker dead-stripping.
//
// SQLite derives the entry point name from the filename:
//   libkham_sqlite.dylib → strips underscores → sqlite3_khamsqlite_init
// The explicit form is always available as sqlite3_kham_init.
// ---------------------------------------------------------------------------

unsafe fn do_init(db: *mut c_void, p_api: *const c_void) -> c_int {
    // Set up the sqlite3_api vtable so C shim helpers can call SQLite functions.
    kham_sqlite_setup_api(p_api);
    let fts5 = kham_sqlite_get_fts5api(db);
    if fts5.is_null() {
        return SQLITE_ERROR;
    }
    register_tokenizer(fts5)
}

/// Extension entry point for explicit loading:
/// `SELECT load_extension('./libkham_sqlite', 'sqlite3_kham_init');`
#[no_mangle]
pub unsafe extern "C" fn sqlite3_kham_init(
    db: *mut c_void,
    _p_err_msg: *mut *mut c_char,
    p_api: *const c_void,
) -> c_int {
    do_init(db, p_api)
}

/// Extension entry point for implicit loading (SQLite 3.44+ strips underscores
/// from the library basename when deriving the symbol name):
/// `SELECT load_extension('./libkham_sqlite');`
#[no_mangle]
pub unsafe extern "C" fn sqlite3_khamsqlite_init(
    db: *mut c_void,
    _p_err_msg: *mut *mut c_char,
    p_api: *const c_void,
) -> c_int {
    do_init(db, p_api)
}

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
//! SQLite FTS5  ──▶  src/shim.c (C)  ──▶  lib.rs (Rust entry points + callbacks)
//!                   SQLITE_EXTENSION_INIT1/2           │
//!                   kham_sqlite_setup_api()             ▼
//!                   kham_sqlite_get_fts5api()  sqlite3_kham_init / sqlite3_khamsqlite_init
//!                                                       │
//!                                                       ▼
//!                                              xCreate → KhamInstance (cached Tokenizer)
//!                                                       │
//!                                                       ▼
//!                                              xTokenize → Tokenizer::segment()
//! ```
//!
//! ## Instance layout
//!
//! `KhamInstance` uses the "C inheritance" pattern: the vtable struct
//! (`KhamFts5Tokenizer`) is the first field, so a `*mut KhamInstance` is
//! pointer-compatible with `*mut KhamFts5Tokenizer`.  SQLite stores and passes
//! back the same pointer it received from `xCreate`, so `xTokenize`/`xDelete`
//! can cast it to `*mut KhamInstance` and access the cached `Tokenizer`.
//!
//! `Tokenizer::new()` is called once per FTS5 table (in `xCreate`), not per
//! document.  This avoids rebuilding the Double-Array Trie on every token call.
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
/// sees and passes back the first-field pointer; the `tokenizer` field is
/// invisible to it and lives behind the pointer.
#[repr(C)]
struct KhamInstance {
    vtable: KhamFts5Tokenizer,
    tokenizer: Tokenizer,
}

// ---------------------------------------------------------------------------
// FTS5 tokenizer callbacks
// ---------------------------------------------------------------------------

/// Allocate a new per-table tokenizer instance, building the `Tokenizer` once.
unsafe extern "C" fn kham_fts5_create(
    _p_ctx: *mut c_void,
    _az_arg: *const *const c_char,
    _n_arg: c_int,
    pp_out: *mut *mut KhamFts5Tokenizer,
) -> c_int {
    if pp_out.is_null() {
        return SQLITE_ERROR;
    }
    let instance = Box::new(KhamInstance {
        vtable: KhamFts5Tokenizer {
            x_create: Some(kham_fts5_create),
            x_delete: Some(kham_fts5_delete),
            x_tokenize: Some(kham_fts5_tokenize),
        },
        // Dict construction happens here — once per FTS5 table, not per document.
        tokenizer: Tokenizer::new(),
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
/// Uses the per-instance [`Tokenizer`] (built once in `xCreate`) which returns
/// zero-copy `Token<'_>` slices with byte-offset spans — exactly what SQLite's
/// `xToken(iStart, iEnd)` needs for `highlight()` and `snippet()`.
unsafe extern "C" fn kham_fts5_tokenize(
    p: *mut KhamFts5Tokenizer,
    p_ctx: *mut c_void,
    _flags: c_int,
    p_text: *const c_char,
    n_text: c_int,
    x_token: XTokenFn,
) -> c_int {
    let result = catch_unwind(|| {
        // Recover the cached Tokenizer from the instance.
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

        let tokens = instance.tokenizer.segment(text);

        for token in &tokens {
            if token.kind == TokenKind::Whitespace {
                continue;
            }

            let tok_bytes = token.text.as_bytes();
            let i_start = token.span.start as c_int;
            let i_end = token.span.end as c_int;

            // SAFETY: tok_bytes points into `text`, which is valid for this call.
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
    let name = b"kham\0".as_ptr() as *const c_char;
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

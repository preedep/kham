//! C FFI for kham-core.
//!
//! Generate the header with:
//! ```bash
//! cbindgen --config kham-capi/cbindgen.toml --crate kham-capi --output kham.h
//! ```
//!
//! Link and use from C:
//! ```c
//! #include "kham.h"
//!
//! KhamTokens* tokens = kham_segment("กินข้าว");
//! for (size_t i = 0; i < tokens->len; i++) {
//!     printf("%s\n", tokens->words[i]);
//! }
//! kham_tokens_free(tokens);
//! ```

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use kham_core::Tokenizer;

/// Heap-allocated array of null-terminated token strings.
///
/// Must be freed with [`kham_tokens_free`].
#[repr(C)]
pub struct KhamTokens {
    /// Pointer to an array of `len` null-terminated UTF-8 strings.
    pub words: *mut *mut c_char,
    /// Number of tokens.
    pub len: usize,
}

/// Segment `text` into Thai tokens.
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * The returned pointer must be freed with [`kham_tokens_free`].
/// * Returns `NULL` if `text` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_segment(text: *const c_char) -> *mut KhamTokens {
    if text.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(text) };
    let s = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let tok = Tokenizer::new();
    let tokens = tok.segment(s);

    let mut words: Vec<*mut c_char> = tokens
        .iter()
        .map(|t| {
            CString::new(t.text).unwrap_or_default().into_raw()
        })
        .collect();

    let len = words.len();
    words.shrink_to_fit();
    let ptr = words.as_mut_ptr();
    std::mem::forget(words);

    Box::into_raw(Box::new(KhamTokens { words: ptr, len }))
}

/// Free a [`KhamTokens`] value returned by [`kham_segment`].
///
/// # Safety
///
/// * `tokens` must have been allocated by [`kham_segment`].
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_tokens_free(tokens: *mut KhamTokens) {
    if tokens.is_null() {
        return;
    }
    let tokens = unsafe { Box::from_raw(tokens) };
    let words = unsafe {
        Vec::from_raw_parts(tokens.words, tokens.len, tokens.len)
    };
    for w in words {
        if !w.is_null() {
            drop(unsafe { CString::from_raw(w) });
        }
    }
}

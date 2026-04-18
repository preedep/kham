//! C FFI for kham-core.
//!
//! Generate the header with:
//! ```bash
//! cbindgen --config kham-capi/cbindgen.toml --crate kham-capi --output kham-capi/include/kham.h
//! ```
//!
//! Link and use from C:
//! ```c
//! #include "kham.h"
//!
//! // Simple: array of token strings
//! KhamTokens* tokens = kham_segment("กินข้าว");
//! for (size_t i = 0; i < tokens->len; i++) {
//!     printf("%s\n", tokens->words[i]);
//! }
//! kham_tokens_free(tokens);
//!
//! // Rich: array of KhamToken with full span information
//! KhamTokenList* list = kham_segment_tokens("ธนาคาร100แห่ง");
//! for (size_t i = 0; i < list->len; i++) {
//!     KhamToken t = list->tokens[i];
//!     printf("%s  char %zu..%zu  %s\n", t.text, t.char_start, t.char_end, t.kind);
//! }
//! kham_token_list_free(list);
//! ```

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use kham_core::fts::FtsTokenizer;
use kham_core::{TokenKind, Tokenizer};

// ---------------------------------------------------------------------------
// Token kind helper
// ---------------------------------------------------------------------------

fn kind_cstring(kind: TokenKind) -> CString {
    CString::new(match kind {
        TokenKind::Thai => "Thai",
        TokenKind::Latin => "Latin",
        TokenKind::Number => "Number",
        TokenKind::Punctuation => "Punctuation",
        TokenKind::Emoji => "Emoji",
        TokenKind::Whitespace => "Whitespace",
        TokenKind::Unknown => "Unknown",
    })
    .unwrap()
}

// ---------------------------------------------------------------------------
// Legacy API — simple word-string array
// ---------------------------------------------------------------------------

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

/// Segment `text` into Thai tokens, returning an array of token strings.
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
        .map(|t| CString::new(t.text).unwrap_or_default().into_raw())
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
    let words = unsafe { Vec::from_raw_parts(tokens.words, tokens.len, tokens.len) };
    for w in words {
        if !w.is_null() {
            drop(unsafe { CString::from_raw(w) });
        }
    }
}

// ---------------------------------------------------------------------------
// Rich API — structured token with span information
// ---------------------------------------------------------------------------

/// A single token with text, byte/char span, and kind.
///
/// All pointer fields are heap-allocated and owned by the containing
/// [`KhamTokenList`]. Free the list with [`kham_token_list_free`] — do not
/// free individual fields directly.
#[repr(C)]
pub struct KhamToken {
    /// Null-terminated UTF-8 token text.
    pub text: *mut c_char,
    /// Start byte offset in the original UTF-8 input string.
    pub byte_start: usize,
    /// End byte offset in the original UTF-8 input string.
    pub byte_end: usize,
    /// Start Unicode scalar-value (char) offset in the original input string.
    pub char_start: usize,
    /// End Unicode scalar-value (char) offset in the original input string.
    pub char_end: usize,
    /// Null-terminated token kind string: `"Thai"`, `"Latin"`, `"Number"`,
    /// `"Punctuation"`, `"Emoji"`, `"Whitespace"`, or `"Unknown"`.
    pub kind: *mut c_char,
}

/// Heap-allocated array of [`KhamToken`] values.
///
/// Must be freed with [`kham_token_list_free`].
#[repr(C)]
pub struct KhamTokenList {
    /// Pointer to an array of `len` [`KhamToken`] values.
    pub tokens: *mut KhamToken,
    /// Number of tokens.
    pub len: usize,
}

/// Segment `text` into tokens, returning full span and kind information.
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * The returned pointer must be freed with [`kham_token_list_free`].
/// * Returns `NULL` if `text` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_segment_tokens(text: *const c_char) -> *mut KhamTokenList {
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

    let mut c_tokens: Vec<KhamToken> = tokens
        .iter()
        .map(|t| KhamToken {
            text: CString::new(t.text).unwrap_or_default().into_raw(),
            byte_start: t.span.start,
            byte_end: t.span.end,
            char_start: t.char_span.start,
            char_end: t.char_span.end,
            kind: kind_cstring(t.kind).into_raw(),
        })
        .collect();

    let len = c_tokens.len();
    c_tokens.shrink_to_fit();
    let ptr = c_tokens.as_mut_ptr();
    std::mem::forget(c_tokens);

    Box::into_raw(Box::new(KhamTokenList { tokens: ptr, len }))
}

/// Free a [`KhamTokenList`] value returned by [`kham_segment_tokens`].
///
/// # Safety
///
/// * `list` must have been allocated by [`kham_segment_tokens`].
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_token_list_free(list: *mut KhamTokenList) {
    if list.is_null() {
        return;
    }
    let list = unsafe { Box::from_raw(list) };
    let tokens = unsafe { Vec::from_raw_parts(list.tokens, list.len, list.len) };
    for t in tokens {
        if !t.text.is_null() {
            drop(unsafe { CString::from_raw(t.text) });
        }
        if !t.kind.is_null() {
            drop(unsafe { CString::from_raw(t.kind) });
        }
    }
}

// ---------------------------------------------------------------------------
// FTS API — FtsTokenizer pipeline results
// ---------------------------------------------------------------------------

/// A single FTS token with stopword flag, synonym list, and trigrams.
///
/// All pointer fields are heap-allocated and owned by the containing
/// [`KhamFtsTokenList`]. Free the list with [`kham_fts_token_list_free`] —
/// do not free individual fields directly.
#[repr(C)]
pub struct KhamFtsToken {
    /// Null-terminated UTF-8 token text.
    pub text: *mut c_char,
    /// Ordinal position in the non-whitespace token sequence (0-based).
    pub position: usize,
    /// Null-terminated token kind string: `"Thai"`, `"Latin"`, `"Number"`,
    /// `"Punctuation"`, `"Emoji"`, `"Whitespace"`, or `"Unknown"`.
    pub kind: *mut c_char,
    /// `true` if this token matches the built-in stopword list.
    pub is_stop: bool,
    /// Heap-allocated array of `synonyms_len` null-terminated synonym strings.
    pub synonyms: *mut *mut c_char,
    /// Number of entries in `synonyms`.
    pub synonyms_len: usize,
    /// Heap-allocated array of `trigrams_len` null-terminated trigram strings.
    /// Populated only for `TokenKind::Unknown` tokens.
    pub trigrams: *mut *mut c_char,
    /// Number of entries in `trigrams`.
    pub trigrams_len: usize,
}

/// Heap-allocated array of [`KhamFtsToken`] values.
///
/// Must be freed with [`kham_fts_token_list_free`].
#[repr(C)]
pub struct KhamFtsTokenList {
    /// Pointer to an array of `len` [`KhamFtsToken`] values.
    pub tokens: *mut KhamFtsToken,
    /// Number of tokens.
    pub len: usize,
}

fn strings_to_c_array(strings: Vec<String>) -> (*mut *mut c_char, usize) {
    let mut v: Vec<*mut c_char> = strings
        .into_iter()
        .map(|s| CString::new(s).unwrap_or_default().into_raw())
        .collect();
    let len = v.len();
    v.shrink_to_fit();
    let ptr = v.as_mut_ptr();
    std::mem::forget(v);
    (ptr, len)
}

/// Segment `text` through the FTS pipeline and return annotated tokens.
///
/// Uses the built-in stopword list, no synonyms, and trigram size 3.
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * The returned pointer must be freed with [`kham_fts_token_list_free`].
/// * Returns `NULL` if `text` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_fts_segment(text: *const c_char) -> *mut KhamFtsTokenList {
    if text.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(text) };
    let s = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let fts = FtsTokenizer::new();
    let fts_tokens = fts.segment_for_fts(s);

    let mut c_tokens: Vec<KhamFtsToken> = fts_tokens
        .into_iter()
        .map(|t| {
            let (synonyms, synonyms_len) = strings_to_c_array(t.synonyms);
            let (trigrams, trigrams_len) = strings_to_c_array(t.trigrams);
            KhamFtsToken {
                text: CString::new(t.text).unwrap_or_default().into_raw(),
                position: t.position,
                kind: kind_cstring(t.kind).into_raw(),
                is_stop: t.is_stop,
                synonyms,
                synonyms_len,
                trigrams,
                trigrams_len,
            }
        })
        .collect();

    let len = c_tokens.len();
    c_tokens.shrink_to_fit();
    let ptr = c_tokens.as_mut_ptr();
    std::mem::forget(c_tokens);

    Box::into_raw(Box::new(KhamFtsTokenList { tokens: ptr, len }))
}

/// Free a [`KhamFtsTokenList`] value returned by [`kham_fts_segment`].
///
/// # Safety
///
/// * `list` must have been allocated by [`kham_fts_segment`].
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_fts_token_list_free(list: *mut KhamFtsTokenList) {
    if list.is_null() {
        return;
    }
    let list = unsafe { Box::from_raw(list) };
    let tokens = unsafe { Vec::from_raw_parts(list.tokens, list.len, list.len) };
    for t in tokens {
        if !t.text.is_null() {
            drop(unsafe { CString::from_raw(t.text) });
        }
        if !t.kind.is_null() {
            drop(unsafe { CString::from_raw(t.kind) });
        }
        if !t.synonyms.is_null() {
            let syns = unsafe { Vec::from_raw_parts(t.synonyms, t.synonyms_len, t.synonyms_len) };
            for s in syns {
                if !s.is_null() {
                    drop(unsafe { CString::from_raw(s) });
                }
            }
        }
        if !t.trigrams.is_null() {
            let grams = unsafe { Vec::from_raw_parts(t.trigrams, t.trigrams_len, t.trigrams_len) };
            for g in grams {
                if !g.is_null() {
                    drop(unsafe { CString::from_raw(g) });
                }
            }
        }
    }
}

/// Collect all FTS lexemes for `text` as a flat null-terminated string array.
///
/// Lexemes are: non-stop token texts, plus synonym expansions and trigrams for
/// unknown tokens. Writes the count to `*out_len`.
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * `out_len` must be a valid non-null pointer to a `usize`.
/// * The returned pointer must be freed with [`kham_fts_lexemes_free`] using
///   the same `len` written to `*out_len`.
/// * Returns `NULL` if `text` is null, `out_len` is null, or input is invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_fts_lexemes(
    text: *const c_char,
    out_len: *mut usize,
) -> *mut *mut c_char {
    if text.is_null() || out_len.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(text) };
    let s = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let lexemes = FtsTokenizer::new().lexemes(s);
    let (ptr, len) = strings_to_c_array(lexemes);
    unsafe { *out_len = len };
    ptr
}

/// Free a lexeme array returned by [`kham_fts_lexemes`].
///
/// # Safety
///
/// * `lexemes` must have been allocated by [`kham_fts_lexemes`] with the
///   matching `len` written to `*out_len`.
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_fts_lexemes_free(lexemes: *mut *mut c_char, len: usize) {
    if lexemes.is_null() {
        return;
    }
    let v = unsafe { Vec::from_raw_parts(lexemes, len, len) };
    for s in v {
        if !s.is_null() {
            drop(unsafe { CString::from_raw(s) });
        }
    }
}

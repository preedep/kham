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

use kham_core::{
    fts::FtsTokenizer,
    keyword::KeyExtractor,
    normalizer::normalize as core_normalize,
    number::{
        parse_thai_baht as core_parse_baht, parse_thai_word as core_parse_thai_word,
        thai_digits_to_ascii as core_thai_digits_to_ascii, to_thai_baht_text as core_to_baht_text,
        u64_to_thai_word as core_number_to_word,
    },
    romanizer::RomanizationMap,
    sentence::split_sentences as core_split,
    soundex::{
        soundex as core_soundex, sounds_like as core_sounds_like,
        sounds_like_cross_lang as core_sounds_like_cross,
        thai_english_soundex as core_thai_english_soundex, SoundexAlgorithm,
    },
    spell::SpellChecker,
    TokenKind, Tokenizer,
};

// ---------------------------------------------------------------------------
// Helpers
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
        TokenKind::Named(ne) => ne.as_str(),
    })
    .unwrap()
}

fn strings_to_c_array(strings: Vec<String>) -> (*mut *mut c_char, usize) {
    let mut v: Vec<*mut c_char> = strings
        .into_iter()
        .map(|s| CString::new(s).unwrap_or_default().into_raw())
        .collect();
    let len = v.len();
    let ptr = v.as_mut_ptr();
    std::mem::forget(v);
    (ptr, len)
}

unsafe fn free_c_array(ptr: *mut *mut c_char, len: usize) {
    let v = unsafe { Vec::from_raw_parts(ptr, len, len) };
    for s in v {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Parse an optional algo string (`"lk82"`, `"udom83"`, `"metasound"`).
/// NULL or unrecognised → [`SoundexAlgorithm::Lk82`].
unsafe fn parse_algo(algo: *const c_char) -> SoundexAlgorithm {
    if algo.is_null() {
        return SoundexAlgorithm::Lk82;
    }
    match unsafe { CStr::from_ptr(algo) }.to_str().unwrap_or("") {
        "udom83" => SoundexAlgorithm::Udom83,
        "metasound" => SoundexAlgorithm::MetaSound,
        _ => SoundexAlgorithm::Lk82,
    }
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
    ///
    /// Note: `"Person"`, `"Place"`, and `"Org"` are never produced by
    /// [`kham_segment_tokens`] because NE tagging is not part of the basic
    /// segmentation pipeline. Use [`kham_fts_segment`] to obtain Named tokens.
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

/// A single FTS token with stopword flag, synonym list, trigrams, POS, NE, and romanization.
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
    /// Null-terminated token kind string.
    ///
    /// Possible values: `"Thai"`, `"Latin"`, `"Number"`, `"Punctuation"`,
    /// `"Emoji"`, `"Whitespace"`, `"Unknown"`.
    /// Named entity tokens produced by the NE gazetteer use `"Person"`,
    /// `"Place"`, or `"Org"` instead of `"Thai"`.
    pub kind: *mut c_char,
    /// `true` if this token matches the built-in stopword list.
    pub is_stop: bool,
    /// RTGS romanization of the token text. Never NULL; equals the original
    /// text for non-Thai or out-of-vocabulary tokens.
    pub roman: *mut c_char,
    /// POS tag string, or `NULL` if the word is not in the POS dictionary.
    ///
    /// Possible values: `"Noun"`, `"Verb"`, `"Adj"`, `"Adv"`, `"Particle"`,
    /// `"ProperNoun"`, `"Pronoun"`, `"Numeral"`, `"Classifier"`,
    /// `"Conjunction"`, `"Auxiliary"`, `"Determiner"`, `"Preposition"`.
    pub pos: *mut c_char,
    /// Named entity category, or `NULL` if not in the NE gazetteer.
    ///
    /// Possible values: `"Person"`, `"Place"`, `"Org"`.
    pub ne: *mut c_char,
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

/// Segment `text` through the FTS pipeline and return annotated tokens.
///
/// Uses the built-in stopword list, no synonyms, and trigram size 3.
/// Each token includes POS tag, NE category, and RTGS romanization.
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
    let roman_map = RomanizationMap::builtin();
    let fts_tokens = fts.segment_for_fts(s);

    let mut c_tokens: Vec<KhamFtsToken> = fts_tokens
        .into_iter()
        .map(|t| {
            let roman = roman_map.romanize_or_raw(&t.text).to_owned();
            let pos_ptr = t
                .pos
                .map(|p| CString::new(p.as_str()).unwrap_or_default().into_raw())
                .unwrap_or(std::ptr::null_mut());
            let ne_ptr =
                t.ne.map(|n| CString::new(n.as_str()).unwrap_or_default().into_raw())
                    .unwrap_or(std::ptr::null_mut());
            let (synonyms, synonyms_len) = strings_to_c_array(t.synonyms);
            let (trigrams, trigrams_len) = strings_to_c_array(t.trigrams);
            KhamFtsToken {
                text: CString::new(t.text).unwrap_or_default().into_raw(),
                position: t.position,
                kind: kind_cstring(t.kind).into_raw(),
                is_stop: t.is_stop,
                roman: CString::new(roman).unwrap_or_default().into_raw(),
                pos: pos_ptr,
                ne: ne_ptr,
                synonyms,
                synonyms_len,
                trigrams,
                trigrams_len,
            }
        })
        .collect();

    let len = c_tokens.len();
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
        drop(unsafe { CString::from_raw(t.text) });
        drop(unsafe { CString::from_raw(t.kind) });
        drop(unsafe { CString::from_raw(t.roman) });
        if !t.pos.is_null() {
            drop(unsafe { CString::from_raw(t.pos) });
        }
        if !t.ne.is_null() {
            drop(unsafe { CString::from_raw(t.ne) });
        }
        unsafe { free_c_array(t.synonyms, t.synonyms_len) };
        unsafe { free_c_array(t.trigrams, t.trigrams_len) };
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
    if !lexemes.is_null() {
        unsafe { free_c_array(lexemes, len) };
    }
}

// ---------------------------------------------------------------------------
// String free — used by all functions that return *mut c_char
// ---------------------------------------------------------------------------

/// Free a null-terminated string returned by any kham function that returns
/// `char *` (e.g. [`kham_normalize`], [`kham_soundex`], [`kham_number_to_thai_word`]).
///
/// # Safety
///
/// * `s` must have been allocated by a kham function.
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/// Normalize Thai text: deduplicate tone marks and compose sara am.
///
/// Returns a newly allocated null-terminated UTF-8 string.
/// Free with [`kham_string_free`].
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * Returns `NULL` if `text` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_normalize(text: *const c_char) -> *mut c_char {
    if text.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    CString::new(core_normalize(s).as_bytes())
        .unwrap_or_default()
        .into_raw()
}

// ---------------------------------------------------------------------------
// Romanization
// ---------------------------------------------------------------------------

/// A token text paired with its RTGS romanization.
///
/// Owned by the enclosing [`KhamRomanTokenList`]; do not free fields directly.
#[repr(C)]
pub struct KhamRomanToken {
    /// Null-terminated original token text.
    pub text: *mut c_char,
    /// Null-terminated RTGS romanization.
    pub roman: *mut c_char,
}

/// Heap-allocated array of [`KhamRomanToken`] values.
///
/// Must be freed with [`kham_roman_token_list_free`].
#[repr(C)]
pub struct KhamRomanTokenList {
    /// Pointer to an array of `len` [`KhamRomanToken`] values.
    pub tokens: *mut KhamRomanToken,
    /// Number of tokens.
    pub len: usize,
}

/// Segment `text` and return each token paired with its RTGS romanization.
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * The returned pointer must be freed with [`kham_roman_token_list_free`].
/// * Returns `NULL` if `text` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_romanize(text: *const c_char) -> *mut KhamRomanTokenList {
    if text.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let map = RomanizationMap::builtin();
    let tok = Tokenizer::new();
    let mut c_tokens: Vec<KhamRomanToken> = tok
        .segment(s)
        .iter()
        .map(|t| KhamRomanToken {
            text: CString::new(t.text).unwrap_or_default().into_raw(),
            roman: CString::new(map.romanize_or_raw(t.text))
                .unwrap_or_default()
                .into_raw(),
        })
        .collect();

    let len = c_tokens.len();
    let ptr = c_tokens.as_mut_ptr();
    std::mem::forget(c_tokens);

    Box::into_raw(Box::new(KhamRomanTokenList { tokens: ptr, len }))
}

/// Free a [`KhamRomanTokenList`] value returned by [`kham_romanize`].
///
/// # Safety
///
/// * `list` must have been allocated by [`kham_romanize`].
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_roman_token_list_free(list: *mut KhamRomanTokenList) {
    if list.is_null() {
        return;
    }
    let list = unsafe { Box::from_raw(list) };
    let tokens = unsafe { Vec::from_raw_parts(list.tokens, list.len, list.len) };
    for t in tokens {
        if !t.text.is_null() {
            drop(unsafe { CString::from_raw(t.text) });
        }
        if !t.roman.is_null() {
            drop(unsafe { CString::from_raw(t.roman) });
        }
    }
}

// ---------------------------------------------------------------------------
// Sentence splitting
// ---------------------------------------------------------------------------

/// A sentence span with text and Unicode char offsets.
///
/// Owned by the enclosing [`KhamSentenceList`]; do not free fields directly.
#[repr(C)]
pub struct KhamSentence {
    /// Null-terminated UTF-8 sentence text.
    pub text: *mut c_char,
    /// Start Unicode scalar-value offset in the original input string.
    pub char_start: usize,
    /// End Unicode scalar-value offset in the original input string.
    pub char_end: usize,
}

/// Heap-allocated array of [`KhamSentence`] values.
///
/// Must be freed with [`kham_sentence_list_free`].
#[repr(C)]
pub struct KhamSentenceList {
    /// Pointer to an array of `len` [`KhamSentence`] values.
    pub sentences: *mut KhamSentence,
    /// Number of sentences.
    pub len: usize,
}

/// Split `text` into sentence spans.
///
/// Boundaries: Thai markers (ฯ ๚ ๛), newlines, and Western punctuation
/// (`! ? .` followed by a space).
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * The returned pointer must be freed with [`kham_sentence_list_free`].
/// * Returns `NULL` if `text` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_split_sentences(text: *const c_char) -> *mut KhamSentenceList {
    if text.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let mut c_sents: Vec<KhamSentence> = core_split(s)
        .into_iter()
        .map(|sent| KhamSentence {
            text: CString::new(sent.text).unwrap_or_default().into_raw(),
            char_start: sent.char_span.start,
            char_end: sent.char_span.end,
        })
        .collect();

    let len = c_sents.len();
    let ptr = c_sents.as_mut_ptr();
    std::mem::forget(c_sents);

    Box::into_raw(Box::new(KhamSentenceList {
        sentences: ptr,
        len,
    }))
}

/// Free a [`KhamSentenceList`] value returned by [`kham_split_sentences`].
///
/// # Safety
///
/// * `list` must have been allocated by [`kham_split_sentences`].
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_sentence_list_free(list: *mut KhamSentenceList) {
    if list.is_null() {
        return;
    }
    let list = unsafe { Box::from_raw(list) };
    let sents = unsafe { Vec::from_raw_parts(list.sentences, list.len, list.len) };
    for s in sents {
        if !s.text.is_null() {
            drop(unsafe { CString::from_raw(s.text) });
        }
    }
}

// ---------------------------------------------------------------------------
// Soundex
// ---------------------------------------------------------------------------

/// Encode `word` using the specified phonetic algorithm.
///
/// `algo` is one of `"lk82"` (default), `"udom83"`, or `"metasound"`.
/// Pass `NULL` for `algo` to use the default (`"lk82"`).
///
/// Returns a newly allocated null-terminated string.
/// Free with [`kham_string_free`].
///
/// # Safety
///
/// * `word` must be a valid null-terminated UTF-8 string.
/// * `algo` may be `NULL` (→ lk82) or a valid null-terminated string.
/// * Returns `NULL` if `word` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_soundex(word: *const c_char, algo: *const c_char) -> *mut c_char {
    if word.is_null() {
        return std::ptr::null_mut();
    }
    let w = match unsafe { CStr::from_ptr(word) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let alg = unsafe { parse_algo(algo) };
    CString::new(core_soundex(w, alg))
        .unwrap_or_default()
        .into_raw()
}

/// Return `true` if `a` and `b` sound alike under the given algorithm.
///
/// `algo` is one of `"lk82"` (default), `"udom83"`, or `"metasound"`.
/// Pass `NULL` for `algo` to use the default.
///
/// # Safety
///
/// * `a` and `b` must be valid null-terminated UTF-8 strings.
/// * `algo` may be `NULL`.
/// * Returns `false` if either input is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_sounds_like(
    a: *const c_char,
    b: *const c_char,
    algo: *const c_char,
) -> bool {
    if a.is_null() || b.is_null() {
        return false;
    }
    let (Ok(sa), Ok(sb)) = (
        unsafe { CStr::from_ptr(a) }.to_str(),
        unsafe { CStr::from_ptr(b) }.to_str(),
    ) else {
        return false;
    };
    let alg = unsafe { parse_algo(algo) };
    core_sounds_like(sa, sb, alg)
}

/// Compute the Thai–English cross-language soundex for `word`.
///
/// Accepts both Thai script and ASCII input.
/// Returns a newly allocated null-terminated string.
/// Free with [`kham_string_free`].
///
/// # Safety
///
/// * `word` must be a valid null-terminated UTF-8 string.
/// * Returns `NULL` if `word` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_thai_english_soundex(word: *const c_char) -> *mut c_char {
    if word.is_null() {
        return std::ptr::null_mut();
    }
    let w = match unsafe { CStr::from_ptr(word) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    CString::new(core_thai_english_soundex(w))
        .unwrap_or_default()
        .into_raw()
}

/// Return `true` if `a` and `b` are phonetically similar across Thai and English.
///
/// # Safety
///
/// * `a` and `b` must be valid null-terminated UTF-8 strings.
/// * Returns `false` if either input is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_sounds_like_cross_lang(a: *const c_char, b: *const c_char) -> bool {
    if a.is_null() || b.is_null() {
        return false;
    }
    let (Ok(sa), Ok(sb)) = (
        unsafe { CStr::from_ptr(a) }.to_str(),
        unsafe { CStr::from_ptr(b) }.to_str(),
    ) else {
        return false;
    };
    core_sounds_like_cross(sa, sb)
}

// ---------------------------------------------------------------------------
// Number conversion
// ---------------------------------------------------------------------------

/// Convert Thai digit characters (๐–๙) in `text` to ASCII digits.
///
/// Returns a newly allocated null-terminated UTF-8 string.
/// Free with [`kham_string_free`].
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * Returns `NULL` if `text` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_thai_digits_to_ascii(text: *const c_char) -> *mut c_char {
    if text.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    CString::new(core_thai_digits_to_ascii(s))
        .unwrap_or_default()
        .into_raw()
}

/// Convert the integer `n` to its Thai cardinal word representation.
///
/// Returns a newly allocated null-terminated UTF-8 string.
/// Free with [`kham_string_free`].
#[no_mangle]
pub extern "C" fn kham_number_to_thai_word(n: u64) -> *mut c_char {
    CString::new(core_number_to_word(n))
        .unwrap_or_default()
        .into_raw()
}

/// Parse a Thai cardinal number word into an integer.
///
/// On success writes the value to `*out_n` and returns `true`.
/// Returns `false` (and does not write to `*out_n`) if the text is not a
/// valid Thai number word.
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * `out_n` must be a valid non-null pointer to a `uint64_t`.
/// * Returns `false` if `text` or `out_n` is null.
#[no_mangle]
pub unsafe extern "C" fn kham_thai_word_to_number(text: *const c_char, out_n: *mut u64) -> bool {
    if text.is_null() || out_n.is_null() {
        return false;
    }
    let s = match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    match core_parse_thai_word(s) {
        Some(n) => {
            unsafe { *out_n = n };
            true
        }
        None => false,
    }
}

/// Render a Baht amount as Thai currency text.
///
/// `satang` must be 0–99.
/// Returns a newly allocated null-terminated UTF-8 string.
/// Free with [`kham_string_free`].
#[no_mangle]
pub extern "C" fn kham_number_to_baht_text(baht: u64, satang: u8) -> *mut c_char {
    CString::new(core_to_baht_text(baht, satang))
        .unwrap_or_default()
        .into_raw()
}

/// A parsed Thai Baht currency amount.
#[repr(C)]
pub struct KhamBahtAmount {
    /// Whole baht amount.
    pub baht: u64,
    /// Satang (0–99).
    pub satang: u8,
}

/// Parse a Thai Baht currency string.
///
/// Returns a heap-allocated [`KhamBahtAmount`] on success, or `NULL` if the
/// string is not a valid Thai Baht amount.
/// Free with [`kham_baht_amount_free`].
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * Returns `NULL` if `text` is null, contains invalid UTF-8, or is not a
///   valid Thai Baht string.
#[no_mangle]
pub unsafe extern "C" fn kham_parse_baht_text(text: *const c_char) -> *mut KhamBahtAmount {
    if text.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    match core_parse_baht(s) {
        Some(amt) => Box::into_raw(Box::new(KhamBahtAmount {
            baht: amt.baht,
            satang: amt.satang,
        })),
        None => std::ptr::null_mut(),
    }
}

/// Free a [`KhamBahtAmount`] returned by [`kham_parse_baht_text`].
///
/// # Safety
///
/// * `amt` must have been allocated by [`kham_parse_baht_text`].
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_baht_amount_free(amt: *mut KhamBahtAmount) {
    if !amt.is_null() {
        drop(unsafe { Box::from_raw(amt) });
    }
}

// ---------------------------------------------------------------------------
// Spell checking
// ---------------------------------------------------------------------------

/// A single spelling suggestion.
///
/// Owned by the enclosing [`KhamSpellList`]; do not free fields directly.
#[repr(C)]
pub struct KhamSpellSuggestion {
    /// Null-terminated UTF-8 candidate word.
    pub word: *mut c_char,
    /// Levenshtein edit distance from the input (0–2).
    pub edit_distance: u8,
    /// `true` if the lk82 phonetic codes of input and candidate match.
    pub soundex_match: bool,
    /// TNC corpus frequency score; `0` if not in the table.
    pub freq_score: u32,
}

/// Heap-allocated array of [`KhamSpellSuggestion`] values.
///
/// Must be freed with [`kham_spell_list_free`].
#[repr(C)]
pub struct KhamSpellList {
    /// Pointer to an array of `len` [`KhamSpellSuggestion`] values.
    pub suggestions: *mut KhamSpellSuggestion,
    /// Number of suggestions.
    pub len: usize,
}

/// Return up to `max_n` spelling suggestions for `word`.
///
/// Only candidates within Levenshtein edit distance ≤ 2 are returned, sorted
/// by phonetic match (lk82), then edit distance, then TNC corpus frequency.
///
/// # Safety
///
/// * `word` must be a valid null-terminated UTF-8 string.
/// * The returned pointer must be freed with [`kham_spell_list_free`].
/// * Returns `NULL` if `word` is null, contains invalid UTF-8, or `max_n` is 0.
#[no_mangle]
pub unsafe extern "C" fn kham_spell_suggestions(
    word: *const c_char,
    max_n: usize,
) -> *mut KhamSpellList {
    if word.is_null() || max_n == 0 {
        return std::ptr::null_mut();
    }
    let w = match unsafe { CStr::from_ptr(word) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let suggestions = SpellChecker::builtin().suggestions(w, max_n);
    let mut c_suggs: Vec<KhamSpellSuggestion> = suggestions
        .into_iter()
        .map(|s| KhamSpellSuggestion {
            word: CString::new(s.word).unwrap_or_default().into_raw(),
            edit_distance: s.edit_distance,
            soundex_match: s.soundex_match,
            freq_score: s.freq_score,
        })
        .collect();

    let len = c_suggs.len();
    let ptr = c_suggs.as_mut_ptr();
    std::mem::forget(c_suggs);

    Box::into_raw(Box::new(KhamSpellList {
        suggestions: ptr,
        len,
    }))
}

/// Free a [`KhamSpellList`] returned by [`kham_spell_suggestions`].
///
/// # Safety
///
/// * `list` must have been allocated by [`kham_spell_suggestions`].
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_spell_list_free(list: *mut KhamSpellList) {
    if list.is_null() {
        return;
    }
    let list = unsafe { Box::from_raw(list) };
    let suggs = unsafe { Vec::from_raw_parts(list.suggestions, list.len, list.len) };
    for s in suggs {
        if !s.word.is_null() {
            drop(unsafe { CString::from_raw(s.word) });
        }
    }
}

// ---------------------------------------------------------------------------
// Keyword extraction
// ---------------------------------------------------------------------------

/// A single keyword with relevance score and occurrence count.
///
/// Owned by the enclosing [`KhamKeywordList`]; do not free fields directly.
#[repr(C)]
pub struct KhamKeyword {
    /// Null-terminated UTF-8 keyword text.
    pub word: *mut c_char,
    /// TF × IDF_proxy relevance score. Higher means more document-distinctive.
    pub score: f32,
    /// Raw occurrence count of this word in the document.
    pub count: usize,
}

/// Heap-allocated array of [`KhamKeyword`] values.
///
/// Must be freed with [`kham_keyword_list_free`].
#[repr(C)]
pub struct KhamKeywordList {
    /// Pointer to an array of `len` [`KhamKeyword`] values.
    pub keywords: *mut KhamKeyword,
    /// Number of keywords.
    pub len: usize,
}

/// Extract up to `max_n` keywords from `text`, ranked by TF × IDF_proxy.
///
/// Stopwords and single-character tokens are excluded. Returns `NULL` when
/// `text` is null or `max_n` is 0.
///
/// # Safety
///
/// * `text` must be a valid null-terminated UTF-8 string.
/// * The returned pointer must be freed with [`kham_keyword_list_free`].
/// * Returns `NULL` if `text` is null or contains invalid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn kham_keywords(
    text: *const c_char,
    max_n: usize,
) -> *mut KhamKeywordList {
    if text.is_null() || max_n == 0 {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let keywords = KeyExtractor::builtin().extract(s, max_n);
    let mut c_kws: Vec<KhamKeyword> = keywords
        .into_iter()
        .map(|k| KhamKeyword {
            word: CString::new(k.word).unwrap_or_default().into_raw(),
            score: k.score,
            count: k.count,
        })
        .collect();

    let len = c_kws.len();
    let ptr = c_kws.as_mut_ptr();
    std::mem::forget(c_kws);

    Box::into_raw(Box::new(KhamKeywordList {
        keywords: ptr,
        len,
    }))
}

/// Free a [`KhamKeywordList`] returned by [`kham_keywords`].
///
/// # Safety
///
/// * `list` must have been allocated by [`kham_keywords`].
/// * Must not be called more than once on the same pointer.
/// * Passing `NULL` is safe (no-op).
#[no_mangle]
pub unsafe extern "C" fn kham_keyword_list_free(list: *mut KhamKeywordList) {
    if list.is_null() {
        return;
    }
    let list = unsafe { Box::from_raw(list) };
    let kws = unsafe { Vec::from_raw_parts(list.keywords, list.len, list.len) };
    for k in kws {
        if !k.word.is_null() {
            drop(unsafe { CString::from_raw(k.word) });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    unsafe fn ptr_to_str<'a>(ptr: *const c_char) -> &'a str {
        CStr::from_ptr(ptr).to_str().expect("valid UTF-8")
    }

    // --- kham_segment ---

    #[test]
    fn segment_null_returns_null() {
        assert!(unsafe { kham_segment(std::ptr::null()) }.is_null());
    }

    #[test]
    fn segment_basic_returns_tokens() {
        let input = CString::new("กินข้าวกับปลา").unwrap();
        let result = unsafe { kham_segment(input.as_ptr()) };
        assert!(!result.is_null());
        let list = unsafe { &*result };
        assert!(list.len >= 2, "expected multiple tokens, got {}", list.len);
        for i in 0..list.len {
            assert!(!unsafe { ptr_to_str(*list.words.add(i)) }.is_empty());
        }
        unsafe { kham_tokens_free(result) };
    }

    #[test]
    fn tokens_free_null_is_safe() {
        unsafe { kham_tokens_free(std::ptr::null_mut()) };
    }

    // --- kham_segment_tokens ---

    #[test]
    fn segment_tokens_null_returns_null() {
        assert!(unsafe { kham_segment_tokens(std::ptr::null()) }.is_null());
    }

    #[test]
    fn segment_tokens_spans_are_consistent() {
        let src = "ธนาคาร100แห่ง";
        let input = CString::new(src).unwrap();
        let result = unsafe { kham_segment_tokens(input.as_ptr()) };
        assert!(!result.is_null());
        let list = unsafe { &*result };
        for i in 0..list.len {
            let t = unsafe { &*list.tokens.add(i) };
            let text = unsafe { ptr_to_str(t.text) };
            assert_eq!(
                text.len(),
                t.byte_end - t.byte_start,
                "byte span mismatch for token {text:?}"
            );
            assert!(t.char_start <= t.char_end);
        }
        unsafe { kham_token_list_free(result) };
    }

    #[test]
    fn token_list_free_null_is_safe() {
        unsafe { kham_token_list_free(std::ptr::null_mut()) };
    }

    // --- kham_fts_segment ---

    #[test]
    fn fts_segment_null_returns_null() {
        assert!(unsafe { kham_fts_segment(std::ptr::null()) }.is_null());
    }

    #[test]
    fn fts_segment_returns_kind_and_roman() {
        let input = CString::new("กินข้าว").unwrap();
        let result = unsafe { kham_fts_segment(input.as_ptr()) };
        assert!(!result.is_null());
        let list = unsafe { &*result };
        assert!(list.len > 0);
        for i in 0..list.len {
            let t = unsafe { &*list.tokens.add(i) };
            assert!(!t.text.is_null(), "text must not be null");
            assert!(!t.kind.is_null(), "kind must not be null");
            assert!(!t.roman.is_null(), "roman must not be null");
        }
        unsafe { kham_fts_token_list_free(result) };
    }

    #[test]
    fn fts_token_list_free_null_is_safe() {
        unsafe { kham_fts_token_list_free(std::ptr::null_mut()) };
    }

    // --- kham_fts_lexemes ---

    #[test]
    fn fts_lexemes_null_returns_null() {
        let mut len: usize = 0;
        assert!(unsafe { kham_fts_lexemes(std::ptr::null(), &mut len) }.is_null());
    }

    #[test]
    fn fts_lexemes_returns_non_empty_for_thai() {
        let input = CString::new("กินข้าว").unwrap();
        let mut len: usize = 0;
        let result = unsafe { kham_fts_lexemes(input.as_ptr(), &mut len) };
        assert!(!result.is_null());
        assert!(len > 0);
        for i in 0..len {
            assert!(!unsafe { *result.add(i) }.is_null());
        }
        unsafe { kham_fts_lexemes_free(result, len) };
    }

    // --- kham_string_free ---

    #[test]
    fn string_free_null_is_safe() {
        unsafe { kham_string_free(std::ptr::null_mut()) };
    }

    // --- kham_normalize ---

    #[test]
    fn normalize_null_returns_null() {
        assert!(unsafe { kham_normalize(std::ptr::null()) }.is_null());
    }

    #[test]
    fn normalize_already_normal_is_unchanged() {
        let input = CString::new("กินข้าว").unwrap();
        let result = unsafe { kham_normalize(input.as_ptr()) };
        assert!(!result.is_null());
        assert_eq!(unsafe { ptr_to_str(result) }, "กินข้าว");
        unsafe { kham_string_free(result) };
    }

    // --- kham_soundex / kham_sounds_like ---

    #[test]
    fn soundex_null_word_returns_null() {
        assert!(unsafe { kham_soundex(std::ptr::null(), std::ptr::null()) }.is_null());
    }

    #[test]
    fn soundex_lk82_is_four_chars() {
        let word = CString::new("กาน").unwrap();
        let result = unsafe { kham_soundex(word.as_ptr(), std::ptr::null()) };
        assert!(!result.is_null());
        assert_eq!(unsafe { ptr_to_str(result) }.len(), 4);
        unsafe { kham_string_free(result) };
    }

    #[test]
    fn soundex_udom83_is_four_chars() {
        let word = CString::new("กาน").unwrap();
        let algo = CString::new("udom83").unwrap();
        let result = unsafe { kham_soundex(word.as_ptr(), algo.as_ptr()) };
        assert!(!result.is_null());
        assert_eq!(unsafe { ptr_to_str(result) }.len(), 4);
        unsafe { kham_string_free(result) };
    }

    #[test]
    fn sounds_like_null_returns_false() {
        let a = CString::new("กาน").unwrap();
        assert!(!unsafe { kham_sounds_like(std::ptr::null(), a.as_ptr(), std::ptr::null()) });
        assert!(!unsafe { kham_sounds_like(a.as_ptr(), std::ptr::null(), std::ptr::null()) });
    }

    #[test]
    fn sounds_like_homophones_true() {
        let a = CString::new("กาน").unwrap();
        let b = CString::new("ขาน").unwrap();
        assert!(unsafe { kham_sounds_like(a.as_ptr(), b.as_ptr(), std::ptr::null()) });
    }

    // --- kham_thai_english_soundex / kham_sounds_like_cross_lang ---

    #[test]
    fn thai_english_soundex_null_returns_null() {
        assert!(unsafe { kham_thai_english_soundex(std::ptr::null()) }.is_null());
    }

    #[test]
    fn thai_english_soundex_returns_non_empty() {
        let word = CString::new("กิน").unwrap();
        let result = unsafe { kham_thai_english_soundex(word.as_ptr()) };
        assert!(!result.is_null());
        assert!(!unsafe { ptr_to_str(result) }.is_empty());
        unsafe { kham_string_free(result) };
    }

    #[test]
    fn sounds_like_cross_lang_null_returns_false() {
        let a = CString::new("กิน").unwrap();
        assert!(!unsafe { kham_sounds_like_cross_lang(std::ptr::null(), a.as_ptr()) });
        assert!(!unsafe { kham_sounds_like_cross_lang(a.as_ptr(), std::ptr::null()) });
    }

    // --- kham_thai_digits_to_ascii ---

    #[test]
    fn thai_digits_to_ascii_null_returns_null() {
        assert!(unsafe { kham_thai_digits_to_ascii(std::ptr::null()) }.is_null());
    }

    #[test]
    fn thai_digits_to_ascii_converts_thai_digits() {
        let input = CString::new("๑๒๓").unwrap();
        let result = unsafe { kham_thai_digits_to_ascii(input.as_ptr()) };
        assert!(!result.is_null());
        assert_eq!(unsafe { ptr_to_str(result) }, "123");
        unsafe { kham_string_free(result) };
    }

    // --- kham_number_to_thai_word / kham_thai_word_to_number ---

    #[test]
    fn number_to_thai_word_zero() {
        let result = kham_number_to_thai_word(0);
        assert!(!result.is_null());
        assert_eq!(unsafe { ptr_to_str(result) }, "ศูนย์");
        unsafe { kham_string_free(result) };
    }

    #[test]
    fn thai_word_to_number_roundtrip() {
        let input = CString::new("หนึ่งร้อย").unwrap();
        let mut n: u64 = 0;
        assert!(unsafe { kham_thai_word_to_number(input.as_ptr(), &mut n) });
        assert_eq!(n, 100);
    }

    #[test]
    fn thai_word_to_number_null_out_returns_false() {
        let input = CString::new("หนึ่ง").unwrap();
        assert!(!unsafe { kham_thai_word_to_number(input.as_ptr(), std::ptr::null_mut()) });
    }

    #[test]
    fn thai_word_to_number_invalid_returns_false() {
        let input = CString::new("กินข้าว").unwrap();
        let mut n: u64 = 0;
        assert!(!unsafe { kham_thai_word_to_number(input.as_ptr(), &mut n) });
    }

    // --- kham_number_to_baht_text / kham_parse_baht_text / kham_baht_amount_free ---

    #[test]
    fn baht_text_roundtrip_100_baht() {
        let text_ptr = kham_number_to_baht_text(100, 0);
        assert!(!text_ptr.is_null());
        let parsed = unsafe { kham_parse_baht_text(text_ptr as *const c_char) };
        unsafe { kham_string_free(text_ptr) };
        assert!(!parsed.is_null());
        let amt = unsafe { &*parsed };
        assert_eq!(amt.baht, 100);
        assert_eq!(amt.satang, 0);
        unsafe { kham_baht_amount_free(parsed) };
    }

    #[test]
    fn baht_amount_free_null_is_safe() {
        unsafe { kham_baht_amount_free(std::ptr::null_mut()) };
    }

    // --- kham_romanize ---

    #[test]
    fn romanize_null_returns_null() {
        assert!(unsafe { kham_romanize(std::ptr::null()) }.is_null());
    }

    #[test]
    fn romanize_returns_text_and_roman_fields() {
        let input = CString::new("กิน").unwrap();
        let result = unsafe { kham_romanize(input.as_ptr()) };
        assert!(!result.is_null());
        let list = unsafe { &*result };
        assert!(list.len > 0);
        for i in 0..list.len {
            let t = unsafe { &*list.tokens.add(i) };
            assert!(!t.text.is_null());
            assert!(!t.roman.is_null());
        }
        unsafe { kham_roman_token_list_free(result) };
    }

    #[test]
    fn roman_token_list_free_null_is_safe() {
        unsafe { kham_roman_token_list_free(std::ptr::null_mut()) };
    }

    // --- kham_split_sentences ---

    #[test]
    fn split_sentences_null_returns_null() {
        assert!(unsafe { kham_split_sentences(std::ptr::null()) }.is_null());
    }

    #[test]
    fn split_sentences_on_newline() {
        let input = CString::new("กินข้าว\nดื่มน้ำ").unwrap();
        let result = unsafe { kham_split_sentences(input.as_ptr()) };
        assert!(!result.is_null());
        let list = unsafe { &*result };
        assert!(list.len >= 2, "expected >= 2 sentences, got {}", list.len);
        unsafe { kham_sentence_list_free(result) };
    }

    #[test]
    fn sentence_list_free_null_is_safe() {
        unsafe { kham_sentence_list_free(std::ptr::null_mut()) };
    }
}

//! # kham-core
//!
//! Pure Rust Thai word segmentation engine. `no_std` compatible (requires `alloc`).
//!
//! ## Quick start
//!
//! ```rust
//! use kham_core::Tokenizer;
//!
//! let tokenizer = Tokenizer::new();
//! let tokens = tokenizer.segment("กินข้าวกับปลา");
//! for token in &tokens {
//!     println!("{} ({:?})", token.text, token.kind);
//! }
//! ```
//!
//! ### Mixed script
//!
//! Non-Thai spans (Latin, numbers, emoji) pass through unchanged alongside Thai tokens:
//!
//! ```rust
//! use kham_core::{Tokenizer, TokenKind};
//!
//! let tok = Tokenizer::new();
//! let tokens = tok.segment("ธนาคาร100แห่ง");
//! assert_eq!(tokens[1].text, "100");
//! assert_eq!(tokens[1].kind, TokenKind::Number);
//! // char_span is suitable for Python/JS string indexing
//! assert_eq!(tokens[0].char_span, 0..6); // ธนาคาร = 6 chars
//! assert_eq!(tokens[1].char_span, 6..9); // 100 = 3 chars
//! ```
//!
//! ### Custom dictionary
//!
//! Merge extra words with the built-in dictionary using the builder:
//!
//! ```rust
//! use kham_core::Tokenizer;
//!
//! let tok = Tokenizer::builder()
//!     .dict_words("ปัญญาประดิษฐ์\n")
//!     .build();
//! let tokens = tok.segment("ปัญญาประดิษฐ์คือ");
//! assert!(tokens.iter().any(|t| t.text == "ปัญญาประดิษฐ์"));
//! ```
//!
//! ### Normalize then segment
//!
//! [`Tokenizer::segment`] is zero-copy. For input with stacked tone marks or
//! สระลอย in wrong order, normalize into a new `String` first, then borrow it:
//!
//! ```rust
//! use kham_core::Tokenizer;
//!
//! let tok = Tokenizer::new();
//! let normalized = tok.normalize("กเินข้าว"); // reorder สระลอย
//! let tokens = tok.segment(&normalized);       // tokens borrow `normalized`
//! assert!(!tokens.is_empty());
//! ```
//!
//! ### Full-text search pipeline
//!
//! [`fts::FtsTokenizer`] wraps the segmenter with stopword tagging, synonym
//! expansion, POS tags, and named-entity recognition in one call:
//!
//! ```rust
//! use kham_core::fts::FtsTokenizer;
//!
//! let fts = FtsTokenizer::new();
//!
//! // All tokens with metadata (position, kind, stopword flag, synonyms, …)
//! let tokens = fts.segment_for_fts("กินข้าวกับปลา");
//! assert!(tokens.iter().any(|t| t.text == "กับ" && t.is_stop));
//!
//! // Only indexable tokens (stopwords excluded, positions preserved)
//! let indexed = fts.index_tokens("กินข้าวกับปลา");
//! assert!(indexed.iter().all(|t| !t.is_stop));
//!
//! // Flat list of lexeme strings ready for a tsvector
//! let lexemes = fts.lexemes("กินข้าวกับปลา");
//! assert!(lexemes.iter().any(|l| l == "กิน" || l == "ปลา"));
//! ```
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod abbrev;
pub mod date;
pub mod dict;
pub mod error;
pub mod freq;
pub mod fts;
pub mod ne;
pub mod ngram;
pub mod normalizer;
pub mod number;
pub mod pos;
pub mod pre_tokenizer;
pub mod romanizer;
pub mod segmenter;
pub mod sentence;
pub mod soundex;
pub mod stopwords;
pub mod synonym;
pub mod tcc;
pub mod token;

pub use error::KhamError;
pub use segmenter::{Tokenizer, TokenizerBuilder};
pub use token::{NamedEntityKind, Token, TokenKind};

/// Decompress zlib-compressed built-in data produced by the build script.
pub(crate) fn decompress_builtin(data: &[u8]) -> alloc::string::String {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(data)
        .expect("built-in data decompression failed");
    alloc::string::String::from_utf8(bytes).expect("built-in data is valid UTF-8")
}

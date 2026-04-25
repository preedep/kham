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

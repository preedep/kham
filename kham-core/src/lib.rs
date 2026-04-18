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

pub mod dict;
pub mod error;
pub(crate) mod freq;
pub mod normalizer;
pub mod pre_tokenizer;
pub mod segmenter;
pub mod tcc;
pub mod token;

pub use error::KhamError;
pub use segmenter::{Tokenizer, TokenizerBuilder};
pub use token::{Token, TokenKind};

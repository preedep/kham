//! WebAssembly bindings for kham-core via wasm-bindgen.
//!
//! Build with:
//! ```bash
//! wasm-pack build kham-wasm --target web
//! ```
//!
//! Then from JavaScript / TypeScript:
//! ```js
//! import init, { segment } from "./pkg/kham_wasm.js";
//! await init();
//! const tokens = segment("กินข้าวกับปลา");
//! console.log(tokens); // ["กิน", "ข้าว", "กับ", "ปลา"]
//! ```

use kham_core::Tokenizer;
use wasm_bindgen::prelude::*;

/// Segment Thai text and return an array of token strings.
///
/// Mixed-script input (Thai + Latin + numbers) is handled correctly.
///
/// # Arguments
///
/// * `text` — Input string. Must be valid UTF-8 (JavaScript strings are always UTF-16 → UTF-8).
///
/// # Returns
///
/// A `js_sys::Array` of `JsString` token values.
#[wasm_bindgen]
pub fn segment(text: &str) -> Vec<JsValue> {
    let tok = Tokenizer::new();
    tok.segment(text)
        .into_iter()
        .map(|t| JsValue::from_str(t.text))
        .collect()
}

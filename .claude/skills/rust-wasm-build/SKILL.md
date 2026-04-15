---
name: rust-wasm-build
description: Build kham-wasm for WebAssembly targets. Use when building WASM, debugging wasm-pack issues, optimizing WASM bundle size, or setting up the npm package for kham-wasm.
---

# kham-wasm Build Guide

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## Build Commands

```bash
# For browser (ESM)
wasm-pack build kham-wasm --target web --release

# For Node.js
wasm-pack build kham-wasm --target nodejs --release

# For bundlers (webpack/vite)
wasm-pack build kham-wasm --target bundler --release
```

Output goes to `kham-wasm/pkg/`.

## Cargo.toml for kham-wasm

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
kham-core = { path = "../kham-core", default-features = false }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"

[profile.release]
opt-level = "s"       # optimize for size
lto = true
codegen-units = 1
strip = true
```

## wasm-bindgen API Pattern

```rust
use wasm_bindgen::prelude::*;
use kham_core::Tokenizer;

#[wasm_bindgen]
pub struct WasmTokenizer {
    inner: Tokenizer,
}

#[wasm_bindgen]
impl WasmTokenizer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { inner: Tokenizer::default() }
    }

    // Return JSON string — avoid complex types across WASM boundary
    pub fn segment(&self, text: &str) -> String {
        let tokens = self.inner.segment(text);
        serde_json::to_string(&tokens).unwrap_or_default()
    }

    // Simple string array for basic usage
    pub fn segment_words(&self, text: &str) -> Vec<String> {
        self.inner.segment(text)
            .iter()
            .map(|t| t.text.to_string())
            .collect()
    }
}
```

## Common Mistakes

- Do NOT use `std::fs` in WASM — dictionary must be `include_bytes!`
- Do NOT use `println!` — use `web_sys::console::log_1` or `#[wasm_bindgen]` logging
- Do NOT pass `&str` slices back to JS — they reference WASM linear memory. Return owned `String`
- `Vec<String>` works across boundary but `Vec<Token>` does not — serialize to JSON

## Bundle Size Target

Built-in dictionary adds ~500KB-1MB. Target total WASM < 2MB gzipped.
Use `twiggy` to analyze: `twiggy top kham_wasm_bg.wasm`

# kham.rs — Thai Word Segmentation Engine

Batteries-included Thai word segmentation library in Rust. Multi-target: Rust crate, WASM, Python (PyO3), C FFI, CLI.

## Architecture

| Crate | Purpose |
|-------|---------|
| `kham-core/` | Pure Rust, `no_std`. Segmentation, FTS, NLP modules. → [kham-core/CLAUDE.md](kham-core/CLAUDE.md) |
| `kham-python/` | PyO3 bindings → `segment()` / `segment_tokens()` |
| `kham-wasm/` | wasm-bindgen bindings → `segment()` / `segment_tokens()` |
| `kham-capi/` | C FFI via cbindgen |
| `kham-cli/` | CLI binary using clap. → [kham-cli/CLAUDE.md](kham-cli/CLAUDE.md) |
| `kham-pg/` | PostgreSQL text-search parser extension (`cdylib`). → [kham-pg/CLAUDE.md](kham-pg/CLAUDE.md) |

## Commands

```bash
cargo fmt --all                      # format (run before every commit)
cargo fmt --all -- --check           # verify formatting (CI gate)
cargo clippy --workspace --exclude kham-python --exclude kham-wasm --exclude kham-pg --all-targets -- -D warnings
cargo build                          # build all crates
cargo test                           # run all tests
cargo test -p kham-core              # test core only
cargo bench                          # run benchmarks (criterion)
cargo run -p kham-cli -- "ข้อความ"    # run CLI

# Bindings
wasm-pack build kham-wasm --target web
maturin develop -m kham-python/Cargo.toml
source .venv/bin/activate && pytest kham-python/tests/ -v
cbindgen --config kham-capi/cbindgen.toml --crate kham-capi --output kham-capi/include/kham.h
cargo build -p kham-capi --release

# kham-pg (requires pg_config in PATH or PG_CONFIG env var)
cargo build -p kham-pg --release
make -C kham-pg install
make -C kham-pg regress              # pg_regress in Docker (PG 17)
```

## Code Style

- Rust 2021 edition, MSRV 1.85+
- `#![no_std]` in `kham-core` — use `alloc`, no `std`
- All public APIs must have doc comments with Thai+English examples
- Error handling: `Result<T, KhamError>` — no `.unwrap()` in library code
- Zero-copy where possible — return `&str` slices into input text
- Follow the `rust-engineer` skill for general Rust conventions
- Follow the `rust-wasm-build` skill for WASM builds

**Always run `cargo fmt --all` before pushing.** Common CI failures: long signatures not wrapped at 100 chars, struct literals with 3+ fields on one line, `assert_eq!` with message not on its own line.

Common clippy pitfalls: `map_or(false, …)` → `is_some_and(…)`; `map_or(true, …)` → `is_none_or(…)`; literal tabs in doc-comment code blocks.

## Token Output Contract

```rust
pub struct Token<'a> {
    pub text: &'a str,            // zero-copy reference to input
    pub span: Range<usize>,       // byte offsets in original text
    pub char_span: Range<usize>,  // Unicode scalar-value (char) offsets
    pub kind: TokenKind,          // Thai | Latin | Number | Punctuation | Emoji | Whitespace | Unknown | Named(_)
}
```

Byte spans must be valid UTF-8 boundaries. `char_span` is suitable for Python/JS string indexing. **Adding a field to `Token` requires updating all three bindings** (Python, WASM, C FFI).

In bindings, `char_span: Range<usize>` is flattened to `char_start` / `char_end` integer fields; same for `span` → `byte_start` / `byte_end`.

## Binding Rules

- All three bindings expose `segment(text)` (strings only) and `segment_tokens(text)` (rich objects).
- **C FFI legacy API** — `KhamTokens` / `kham_segment()` / `kham_tokens_free()` exist for backward compatibility. Do not remove them. New callers use `kham_segment_tokens()`.
- `unsafe` is confined to `kham-capi/src/lib.rs` and `kham-pg/src/lib.rs`. Do not add `unsafe` to any other crate.
- Regenerate the C header after any `#[repr(C)]` struct change: `cbindgen --config kham-capi/cbindgen.toml --crate kham-capi --output kham-capi/include/kham.h`

## Testing

- Unit tests co-located in each module
- Integration tests in `kham-core/tests/`; test data in `kham-core/testdata/` (format: `input|tok1|tok2|…`)
- Python binding tests: `kham-python/tests/test_kham.py` — run after every `maturin develop`
- kham-pg regress: `make -C kham-pg regress` — Docker (PG 17); expected output in `kham-pg/regress/expected/`

## Important

- **Library-first** — `kham-core` must never depend on `std`
- **Performance matters** — benchmark every PR touching segmenter or dict (`cargo bench`)
- **No BEST corpus or non-CC0 data** in the repo
- Algorithm reference: nlpO3 (Apache-2.0) and PyThaiNLP newmm — clean-room implementation
- All Thai text in tests must be valid UTF-8, never raw bytes

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-18

### Added

**Core (`kham-core`)**
- DAG-based newmm Thai word segmentation algorithm
- Double-Array Trie (DARTS) dictionary with pre-compiled binary blob for fast startup
- Built-in word list (`words_th.txt`, CC0) embedded at compile time
- Thai Character Cluster (TCC) boundary detection (Theeramunkong et al. 2000)
- Unicode script pre-tokenizer: Thai / Latin / Number / Punctuation / Emoji / Whitespace / Unknown
- Thai text normalizer: สระลอย reordering, วรรณยุกต์ deduplication, NFC
- `Token` struct with zero-copy `text: &str`, `span` (byte offsets), `char_span` (Unicode scalar offsets), and `TokenKind`
- `FreqMap` — Thai National Corpus (TNC) frequency table for DP tie-breaking (CC0)
- 4-field lexicographic DP score (`DpScore`): minimise unknowns → maximise dict matches → maximise TNC frequency → minimise token count
- `no_std` compatible (uses `alloc`); optional `std` feature for stdlib integration
- Runtime custom dictionary via `TokenizerBuilder::dict_file()`

**CLI (`kham-cli`)**
- `kham <text>` — segment Thai text, one token per line
- `--dict <FILE>` — load custom word list
- `--sep <STR>` — custom token separator
- `--whitespace` — include whitespace tokens
- `--normalize` — print normalized form before segmenting
- `--kind` — append token kind (`กิน:Thai`)
- `--spans` — append Unicode char span (`กิน:0-3`)

**Python bindings (`kham-python`, PyO3)**
- `kham.segment(text) -> list[str]`
- `kham.segment_tokens(text) -> list[Token]`
- `Token` with `text`, `byte_start`, `byte_end`, `char_start`, `char_end`, `kind`

**WASM bindings (`kham-wasm`, wasm-bindgen)**
- `segment(text: string): string[]`
- `segment_tokens(text: string): Token[]`
- `Token` with matching fields

**C FFI (`kham-capi`, cbindgen)**
- `kham_segment()` / `kham_tokens_free()` — legacy string-only API
- `kham_segment_tokens()` / `kham_token_list_free()` — span-aware `KhamToken` API

**Testing & Benchmarks**
- Integration tests auto-discovering `kham-core/testdata/*.txt` (format: `input|tok1|tok2|…`)
- 30-case pytest suite for Python bindings covering `char_span` round-trip, UTF-8 byte spans, kind labels, and contiguity
- Criterion benchmark suite: dict construction, trie lookup, prefix matching, FreqMap, end-to-end segmentation (short/medium/long), mixed-script scenarios

[0.1.0]: https://github.com/preedep/kham/releases/tag/v0.1.0

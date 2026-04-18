# kham

Thai word segmentation engine written in Rust. Fast, `no_std`-compatible core library with bindings for Python, WebAssembly, C, and a command-line interface.

[![CI](https://github.com/preedep/kham/actions/workflows/ci.yml/badge.svg)](https://github.com/preedep/kham/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/kham-core.svg)](https://crates.io/crates/kham-core)
[![PyPI](https://img.shields.io/pypi/v/kham.svg)](https://pypi.org/project/kham/)
[![npm](https://img.shields.io/npm/v/kham-wasm.svg)](https://www.npmjs.com/package/kham-wasm)

## Features

- **newmm algorithm** — DAG-based maximal matching constrained to Thai Character Cluster (TCC) boundaries
- **Multi-target** — single core library ships as a Rust crate, Python wheel, WASM module, C shared library, and CLI binary
- **Zero-copy API** — `segment()` returns `&str` slices into the original input; no heap allocation per token
- **`no_std` core** — `kham-core` compiles for bare-metal targets (`alloc` only, no `std` dependency)
- **Built-in dictionary** — 62,102-word CC0-licensed Thai word list embedded at compile time; custom dictionaries loaded at runtime
- **TNC frequency scoring** — Thai National Corpus (CC0) raw counts guide the DP scorer to prefer statistically common segmentations when multiple dictionary paths tie
- **Pre-compiled DARTS** — Double-Array Trie is built once at compile time (`build.rs`) and loaded from a binary blob at runtime (~64 µs vs ~960 ms construction from text)
- **Text normalization** — วรรณยุกต์ dedup and Sara Am composition before segmentation
- **Structured CLI logging** — `RUST_LOG`-controlled output with coloured log levels via `env_logger` + `colored`

## Packages

| Crate | Registry | Description |
|---|---|---|
| `kham-core` | [crates.io](https://crates.io/crates/kham-core) | Pure Rust engine, `no_std` compatible |
| `kham-cli` | [crates.io](https://crates.io/crates/kham-cli) | `kham` binary (clap) |
| `kham-python` | [PyPI](https://pypi.org/project/kham/) | Python bindings via PyO3 / maturin |
| `kham-wasm` | [npm](https://www.npmjs.com/package/kham-wasm) | WebAssembly bindings via wasm-bindgen |
| `kham-capi` | — | C FFI with cbindgen-generated header |

## Quick start

### Rust

```toml
[dependencies]
kham-core = "0.1"
```

```rust
use kham_core::Tokenizer;

let tok = Tokenizer::new();
let tokens = tok.segment("กินข้าวกับปลา");
for t in &tokens {
    println!("{} ({:?})", t.text, t.kind);
}
// กิน (Thai)
// ข้าว (Thai)
// ...
```

Mixed script works out of the box:

```rust
let tokens = tok.segment("ธนาคาร100แห่ง");
assert_eq!(tokens[0].text, "ธนาคาร"); // Thai
assert_eq!(tokens[1].text, "100");     // Number
assert_eq!(tokens[2].text, "แห่ง");   // Thai
```

For input that may contain stacked tone marks or decomposed Sara Am, normalize first:

```rust
let normalized = tok.normalize(raw_input); // tone dedup + Sara Am composition
let tokens = tok.segment(&normalized);     // tokens borrow `normalized`
```

### Python

```bash
pip install kham
```

```python
import kham

# Simple — list of token strings
tokens = kham.segment("กินข้าวกับปลา")
print(tokens)  # ['กิน', 'ข้าว', 'กับ', 'ปลา']

# Rich — Token objects with span information
tokens = kham.segment_tokens("ธนาคาร100แห่ง")
for t in tokens:
    print(t.text, t.char_start, t.char_end, t.kind)
# ธนาคาร  0  6  Thai
# 100     6  9  Number
# แห่ง    9  13 Thai
```

`Token` attributes: `text`, `byte_start`, `byte_end`, `char_start`, `char_end`, `kind`.

### JavaScript / TypeScript (WASM)

```bash
npm install kham-wasm
```

```js
import init, { segment, segment_tokens } from "kham-wasm";
await init();

// Simple — array of token strings
const words = segment("กินข้าวกับปลา");
console.log(words); // ["กิน", "ข้าว", "กับ", "ปลา"]

// Rich — Token objects with span information
const tokens = segment_tokens("ธนาคาร100แห่ง");
for (const t of tokens) {
    console.log(t.text, t.char_start, t.char_end, t.kind);
}
// ธนาคาร  0  6  Thai
// 100     6  9  Number
// แห่ง    9  13 Thai
```

`Token` properties: `text`, `byte_start`, `byte_end`, `char_start`, `char_end`, `kind`.

> **Note on JS string offsets:** `char_start`/`char_end` are Unicode scalar-value counts.
> For BMP text these equal JavaScript's `string.slice()` indices. For surrogate-pair
> emoji, use `byte_start`/`byte_end` with `TextEncoder` for precise byte-level slicing.

### CLI

```bash
cargo install kham-cli
```

```bash
# Positional argument
kham "กินข้าวกับปลา"
# กิน|ข้าว|กับ|ปลา

# Custom separator
kham --sep " / " "สวัสดีชาวโลก"
# สวัสดี / ชาว / โลก

# Show token kinds
kham --kind "ธนาคาร100แห่ง"
# ธนาคาร:Thai|100:Number|แห่ง:Thai

# Show Unicode char spans
kham --spans "กินข้าวกับปลา"
# กิน:0-3|ข้าว:3-7|กับ:7-10|ปลา:10-13

# Combine kind and spans
kham --kind --spans "กินข้าว"
# กิน:Thai:0-3|ข้าว:Thai:3-7

# Normalize before segmenting
kham --normalize "กิน\u{0E02}\u{0E49}\u{0E49}าว"

# Custom dictionary
kham --dict my_words.txt "มะม่วงหิมพานต์"

# Pipeline / stdin
echo "กินข้าว" | kham
cat corpus.txt | kham --sep " "
```

Full options:

```
Usage: kham [OPTIONS] [TEXT]

Arguments:
  [TEXT]  Thai text to segment. Reads from stdin line-by-line if omitted.

Options:
  -d, --dict <FILE>   Path to a custom word-list file (newline-separated)
  -s, --sep <SEP>     Output separator between tokens [default: |]
  -w, --whitespace    Include whitespace tokens in output
  -n, --normalize     Run normalize() before segmenting
  -k, --kind          Append token kind after each token (e.g. กิน:Thai)
      --spans         Append Unicode char span after each token (e.g. กิน:0-3)
  -h, --help          Print help
  -V, --version       Print version
```

Debug and timing output is controlled by the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug kham "กินข้าวกับปลา"   # full per-token trace + timing
RUST_LOG=info  kham --dict w.txt "..."  # dict-load confirmation only
```

### C

Generate the header and link `libkham_capi`:

```bash
cbindgen --config kham-capi/cbindgen.toml --crate kham-capi --output kham.h
cargo build -p kham-capi --release
```

```c
#include "kham.h"

// Simple — array of token strings
KhamTokens *tokens = kham_segment("กินข้าวกับปลา");
for (size_t i = 0; i < tokens->len; i++) {
    printf("%s\n", tokens->words[i]);
}
kham_tokens_free(tokens);

// Rich — KhamToken structs with full span information
KhamTokenList *list = kham_segment_tokens("ธนาคาร100แห่ง");
for (size_t i = 0; i < list->len; i++) {
    KhamToken t = list->tokens[i];
    printf("%s  char %zu..%zu  %s\n", t.text, t.char_start, t.char_end, t.kind);
}
// ธนาคาร  char 0..6   Thai
// 100     char 6..9   Number
// แห่ง    char 9..13  Thai
kham_token_list_free(list);
```

`KhamToken` fields: `text`, `byte_start`, `byte_end`, `char_start`, `char_end`, `kind` (all null-terminated UTF-8 strings or `size_t`).

## Token contract

Every `segment()` call returns `Vec<Token>`:

```rust
pub struct Token<'a> {
    pub text: &'a str,            // zero-copy slice of the input string
    pub span: Range<usize>,       // byte offsets in the original string
    pub char_span: Range<usize>,  // Unicode scalar-value (char) offsets
    pub kind: TokenKind,          // Thai | Latin | Number | Punctuation | Emoji | Whitespace | Unknown
}
```

- `span` — byte offsets; use to slice `&str` directly (`&input[token.span.clone()]`)
- `char_span` — Unicode scalar-value offsets; use for Python/JavaScript string indexing where strings are char- or code-unit-indexed
- Both spans are always valid UTF-8 boundaries
- Joining all `token.text` values (with whitespace kept) reconstructs the original input exactly

```rust
use kham_core::Tokenizer;

let tok = Tokenizer::new();
let input = "ธนาคาร100แห่ง";
let tokens = tok.segment(input);

// ธนาคาร: 6 chars, 18 bytes
assert_eq!(tokens[0].span,      0..18);
assert_eq!(tokens[0].char_span, 0..6);

// 100: 3 chars, 3 bytes
assert_eq!(tokens[1].span,      18..21);
assert_eq!(tokens[1].char_span, 6..9);
```

## Custom dictionary

```rust
// From a string
let tok = Tokenizer::builder()
    .dict_words("มะม่วงหิมพานต์\nกระทะ\n")
    .build();

// From a file (requires the `std` feature)
let tok = Tokenizer::builder()
    .dict_file("my_words.txt")?
    .build();

// Keep whitespace tokens
let tok = Tokenizer::builder()
    .keep_whitespace(true)
    .build();
```

## Architecture

### Workspace crate graph

```mermaid
graph LR
    core["<b>kham-core</b><br/><i>no_std · alloc only</i><br/>segmentation engine"]

    cli["<b>kham-cli</b><br/>kham binary<br/>(clap)"]
    python["<b>kham-python</b><br/>Python wheel<br/>(PyO3 · maturin)"]
    wasm["<b>kham-wasm</b><br/>WASM module<br/>(wasm-bindgen)"]
    capi["<b>kham-capi</b><br/>C shared library<br/>(cbindgen)"]

    core --> cli
    core --> python
    core --> wasm
    core --> capi
```

### Core module responsibilities

```mermaid
classDiagram
    direction LR

    class normalizer {
        +normalize(text) String
        --
        วรรณยุกต์ dedup
        Sara Am composition
    }

    class pre_tokenizer {
        +pre_tokenize(text) Vec~Token~
        +classify_char(c) TokenKind
        --
        Unicode script split
        Thai · Latin · Number
        Emoji · Punct · WS
    }

    class tcc {
        +tcc_boundaries(text) Vec~usize~
        +tcc_iter(text) Iterator
        --
        Thai Character Cluster
        boundary detection
        Theeramunkong 2000
    }

    class dict {
        +builtin_dict() Dict
        +from_word_list(text) Dict
        +from_bytes(data) Dict
        +contains(word) bool
        +prefixes(text) Vec~str~
        --
        Double-Array Trie
        O(k) byte-level lookup
        pre-compiled binary blob
        built-in CC0 word list
    }

    class freq {
        +FreqMap::builtin() FreqMap
        +from_tsv(data) FreqMap
        +get(word) u32
        --
        TNC raw occurrence counts
        CC0 · 106k entries
        DP tie-breaking scorer
    }

    class segmenter {
        +segment(text) Vec~Token~
        +normalize(text) String
        --
        newmm DAG algorithm
        DP over TCC boundaries
        min unknowns · max dict words
        TNC freq · min token count
    }

    class token {
        +text : and str
        +span : Range~usize~
        +char_span : Range~usize~
        +kind : TokenKind
        --
        Thai · Latin · Number
        Punctuation · Emoji
        Whitespace · Unknown
    }

    segmenter ..> normalizer : calls
    segmenter ..> pre_tokenizer : calls
    segmenter ..> tcc : calls
    segmenter ..> dict : queries
    segmenter ..> freq : scores
    segmenter ..> token : emits
    pre_tokenizer ..> token : emits
```

### Segmentation pipeline

```mermaid
flowchart TD
    INPUT(["<b>raw &amp;str</b>"])

    subgraph OPTIONAL["optional — call before segment()"]
        NORM["<b>normalizer::normalize()</b>\nวรรณยุกต์ dedup\nSara Am อํ+อา → อำ"]
    end

    PRE["<b>pre_tokenizer::pre_tokenize()</b>\nUnicode script classification\nsplit into homogeneous spans"]

    SPLIT{span kind?}

    PASS["pass through\nas-is"]

    subgraph THAI_PATH["Thai span processing"]
        TCC["<b>tcc::tcc_boundaries()</b>\nTCC boundary positions\n= legal word-break points"]
        DICT["<b>dict::prefixes()</b>\nDATS prefix search\nat each boundary"]
        DAG["<b>DP over boundary graph</b>\nminimise unknown tokens\nmaximise dict-word count\nTNC frequency score · fewest tokens"]
    end

    MERGE(["<b>Vec&lt;Token&lt;'_&gt;&gt;</b>\nzero-copy &amp;str slices"])

    INPUT --> OPTIONAL
    OPTIONAL --> PRE
    PRE --> SPLIT
    SPLIT -->|"Thai"| TCC
    SPLIT -->|"Latin · Number\nEmoji · Punct · WS"| PASS
    TCC --> DICT
    DICT --> DAG
    DAG --> MERGE
    PASS --> MERGE
```

### DAG segmentation detail

```mermaid
flowchart LR
    subgraph INPUT["Thai span: &quot;กินข้าว&quot;"]
        direction LR
        C0(["pos 0"])
        C1(["pos 3\n กิ"])
        C2(["pos 6\n น"])
        C3(["pos 9\n ข้"])
        C4(["pos 15\n าว"])
        C5(["pos 21\n end"])
    end

    C0 -->|"กิน ✓ dict"| C2
    C0 -.->|"กิ  unknown"| C1
    C1 -.->|"น   unknown"| C2
    C2 -->|"ข้าว ✓ dict"| C5
    C2 -.->|"ข้  unknown"| C3
    C3 -.->|"าว  unknown"| C4

    BEST["DP picks bold path:\nกิน · ข้าว\n= 2 dict words"]
    C5 --- BEST
```

## Building

```bash
cargo build                                  # all default members (also runs build.rs → dict.bin)
cargo test --release                         # run all tests
cargo test -p kham-core --release            # core only
cargo bench -p kham-core                     # criterion benchmarks
cargo run -p kham-cli -- "ข้อความ"           # run CLI
```

The `kham-core` build script (`build.rs`) pre-compiles the built-in dictionary into a binary DARTS blob (`$OUT_DIR/dict.bin`) on every `cargo build`. It only reruns when `build.rs` or `data/words_th.txt` change.

Binding targets require additional tooling:

```bash
wasm-pack build kham-wasm --target web           # WASM
maturin develop -m kham-python/Cargo.toml        # Python wheel
```

## CI / CD

Two GitHub Actions workflows run automatically:

### CI (`ci.yml`) — every push and pull request to `main` / `develop`

| Job | What it checks |
|---|---|
| `fmt` | `cargo fmt --check` |
| `clippy` | `cargo clippy -D warnings` |
| `test` | Unit + integration + doc tests on stable and MSRV 1.78, Linux and macOS |
| `no_std` | `kham-core` compiles for `thumbv7em-none-eabihf` (bare metal) |
| `wasm` | `wasm-pack build --target web` succeeds |
| `python` | `maturin develop` on Python 3.8 and 3.12 |
| `bench_compile` | Benchmark suite compiles without errors |

### Release (`release.yml`) — on `v*.*.*` tag push

Publishes to all registries after the CI gate passes:

```mermaid
flowchart LR
    TAG(["git tag v0.1.0\ngit push --tags"])
    CI["CI gate\n(full test matrix)"]
    CRATES["crates.io\nkham-core + kham-cli"]
    PYPI["PyPI\nkham wheels\n(manylinux · macOS · Windows)"]
    NPM["npm\nkham-wasm"]
    GH["GitHub Release\nauto release notes\n+ wheel artifacts"]

    TAG --> CI
    CI --> CRATES
    CI --> PYPI
    CI --> NPM
    CRATES --> GH
    PYPI --> GH
    NPM --> GH
```

#### Required secrets

| Secret | Used for |
|---|---|
| `CARGO_REGISTRY_TOKEN` | crates.io publish |
| `NPM_TOKEN` | npm publish |
| PyPI — no secret needed | OIDC trusted publishing; configure via pypi.org Trusted Publisher |

To cut a release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## Benchmarks

Measured on Apple M-series (release build, LTO, built-in 62k-word dictionary):

| Benchmark | Time | Throughput |
|---|---|---|
| `segment` — short (~37 B) | ~1.0 µs | ~37 MiB/s |
| `segment` — medium (~182 B) | ~4.0 µs | ~42 MiB/s |
| `segment` — long (~546 B) | ~11.6 µs | ~44 MiB/s |
| `dict::contains` (hit) | ~13–32 ns | ~520–690 MiB/s |
| `dict::contains` (miss) | ~1.3 ns | ~4 GiB/s |
| `dict::prefixes` | ~65–112 ns | ~275–860 MiB/s |
| `builtin_dict()` — binary blob load | ~64 µs | — |
| `Dict::from_word_list` — 62k words (custom merge) | ~960 ms | ~65k words/s |
| Dict construction (small custom list) | ~4 µs | — |

> `Tokenizer::new()` and `TokenizerBuilder::build()` (no custom dict) use `builtin_dict()` which
> loads the pre-compiled DARTS binary produced by `build.rs` at compile time — ~15,000× faster
> than constructing from text. `Dict::from_word_list` is only called when a custom dictionary is
> merged with the built-in word list.

Run locally:

```bash
cargo bench -p kham-core
# HTML report: target/criterion/report/index.html
```

## Dictionary

The built-in word list (`kham-core/data/words_th.txt`) is CC0-licensed and contains 62,102 Thai words. Custom dictionaries are newline-separated plain text files; lines beginning with `#` are treated as comments.

A separate frequency table (`kham-core/data/tnc_freq.txt`, CC0) provides raw occurrence counts from the Thai National Corpus (106,125 entries). It is embedded at compile time and loaded into a `FreqMap` at runtime. The newmm DP scorer uses it as the third tiebreaker — after minimising unknown tokens and maximising dictionary matches — so statistically more common segmentations are preferred when multiple paths are otherwise equal. Frequency data is kept separate from `dict.bin`; do not merge them.

**Constraint:** Never ship BEST corpus data or any non-CC0 material in this repository.

### Pre-compiled DARTS binary (`dict.bin`)

`build.rs` compiles the built-in word list into a binary Double-Array Trie blob (`$OUT_DIR/dict.bin`) once at build time. At runtime, `builtin_dict()` loads this blob via `Dict::from_bytes`, which is ~15,000× faster than reconstructing the trie from the text word list (~64 µs vs ~960 ms).

#### File format

All multi-byte integers are **little-endian**. The file begins with a fixed 16-byte header followed immediately by the two DARTS arrays.

| Offset | Size (bytes) | Field       | Type    | Description                                     |
|-------:|-------------:|-------------|---------|------------------------------------------------|
|      0 |            4 | `magic`     | `[u8;4]`| `b"KDAM"` — file-type identifier               |
|      4 |            1 | `version`   | `u8`    | Format version; currently `0x01`               |
|      5 |            3 | `reserved`  | `[u8;3]`| Zero-filled; reserved for future flags         |
|      8 |            4 | `base_len`  | `u32`   | Number of `i32` elements in the `base` array   |
|     12 |            4 | `check_len` | `u32`   | Number of `i32` elements in the `check` array  |
|     16 |  `base_len×4`| `base[]`    | `i32[]` | DARTS base offsets, little-endian              |
| `16 + base_len×4` | `check_len×4` | `check[]` | `i32[]` | DARTS parent-state indices, little-endian (`-1` = unused slot) |

#### Lifecycle

```mermaid
flowchart LR
    WL(["words_th.txt\n62k words · CC0"])
    BS["build.rs\nbuild_trie() → from_trie()\nBFS base-allocation\nFreeBitmap O(n/64)"]
    BIN(["$OUT_DIR/dict.bin\n16-byte header\n+ base[] + check[]"])
    IB["include_bytes!\nembedded in binary"]
    RT["Dict::from_bytes()\none-pass LE decode\nO(S) — ~64 µs"]
    BD(["builtin_dict()\nready Dict"])

    WL --> BS --> BIN --> IB --> RT --> BD

    FQ(["tnc_freq.txt\n106k entries · CC0"])
    FM["include_str!\nembedded at compile time"]
    FP["FreqMap::builtin()\nparse TSV → BTreeMap"]
    FS(["FreqMap\nDP tie-breaking scorer"])

    FQ --> FM --> FP --> FS
```

#### Validity guarantees

`Dict::from_bytes` panics on malformed input rather than returning an error, because failures always indicate a stale or corrupted build artifact — not a recoverable runtime condition. A clean `cargo build` regenerates a valid blob automatically.

| Condition checked           | Panic message                      |
|-----------------------------|------------------------------------|
| `data.len() < 16`           | `"dict.bin too short"`             |
| Bytes 0–3 ≠ `b"KDAM"`      | `"dict.bin: bad magic"`            |
| Byte 4 ≠ `0x01`             | `"dict.bin: unsupported version"`  |

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE)

at your option.

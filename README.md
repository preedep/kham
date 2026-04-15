# kham

Thai word segmentation engine written in Rust. Fast, `no_std`-compatible core library with bindings for Python, WebAssembly, C, and a command-line interface.

## Features

- **newmm algorithm** — DAG-based maximal matching constrained to Thai Character Cluster (TCC) boundaries
- **Multi-target** — single core library ships as a Rust crate, Python wheel, WASM module, C shared library, and CLI binary
- **Zero-copy API** — `segment()` returns `&str` slices into the original input; no heap allocation per token
- **`no_std` core** — `kham-core` compiles for bare-metal targets (`alloc` only, no `std` dependency)
- **Built-in dictionary** — CC0-licensed Thai word list embedded at compile time; custom dictionaries loaded at runtime
- **Text normalization** — วรรณยุกต์ dedup and Sara Am composition before segmentation

## Packages

| Crate | Description |
|---|---|
| `kham-core` | Pure Rust engine, `no_std` compatible |
| `kham-cli` | `kham` binary (clap) |
| `kham-python` | Python bindings via PyO3 / maturin |
| `kham-wasm` | WebAssembly bindings via wasm-bindgen |
| `kham-capi` | C FFI with cbindgen-generated header |

## Quick start

### Rust

```toml
[dependencies]
kham-core = { git = "https://github.com/preedee/kham" }
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

### CLI

```bash
cargo install --path kham-cli
```

```bash
# Positional argument
kham "กินข้าวกับปลา"
# กิน|ข้าว|กั|บ|ปลา

# Custom separator
kham --sep " / " "สวัสดีชาวโลก"
# สวัสดี / ชาวโลก

# Show token kinds
kham --kind "ธนาคาร100แห่ง"
# ธนาคาร:Thai|100:Number|แห่ง:Thai

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
  -h, --help          Print help
  -V, --version       Print version
```

## Token contract

Every `segment()` call returns `Vec<Token>`:

```rust
pub struct Token<'a> {
    pub text: &'a str,       // zero-copy slice of the input string
    pub span: Range<usize>,  // byte offsets in the original string
    pub kind: TokenKind,     // Thai | Latin | Number | Punctuation | Emoji | Whitespace | Unknown
}
```

Byte spans are always valid UTF-8 boundaries. Joining all `token.text` values (with whitespace kept) reconstructs the original input exactly.

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

```
kham-core/src/
  normalizer.rs    — วรรณยุกต์ dedup, Sara Am composition
  pre_tokenizer.rs — Unicode script classification (Thai/Latin/Number/Emoji/…)
  tcc.rs           — Thai Character Cluster boundary detection (Theeramunkong 2000)
  dict.rs          — Double-Array Trie (DARTS), O(k) lookup, built-in word list
  segmenter.rs     — newmm DAG: DP over TCC boundaries, maximises dict matches
  token.rs         — Token struct and TokenKind enum
```

**Pipeline for Thai spans:**

```
raw text
  │
  ▼  pre_tokenize()        split by script (Thai / Latin / Number / …)
  │
  ▼  tcc_boundaries()      find legal word-break positions within Thai spans
  │
  ▼  dict.prefixes()       enumerate dictionary matches at each boundary
  │
  ▼  DP shortest path      maximise dict words, minimise total tokens
  │
  ▼  Vec<Token<'_>>
```

Non-Thai spans (Latin, numbers, emoji, punctuation) pass through the pre-tokenizer unchanged; only Thai spans go through the DAG.

## Building

```bash
cargo build                          # all default members
cargo test                           # run all tests
cargo test -p kham-core              # core only
cargo bench -p kham-core             # criterion benchmarks
cargo run -p kham-cli -- "ข้อความ"   # run CLI
```

Binding targets require additional tooling:

```bash
wasm-pack build kham-wasm --target web           # WASM
maturin develop -m kham-python/Cargo.toml        # Python wheel
```

## Benchmarks

Measured on Apple M-series (release build, LTO):

| Benchmark | Time | Throughput |
|---|---|---|
| `segment` — short (~37 B) | ~1.0 µs | ~37 MiB/s |
| `segment` — medium (~182 B) | ~4.0 µs | ~42 MiB/s |
| `segment` — long (~546 B) | ~11.6 µs | ~44 MiB/s |
| `dict::contains` (hit) | ~19–44 ns | — |
| `dict::contains` (miss) | ~2 ns | — |
| `dict::prefixes` | ~57–100 ns | — |
| Dict construction (built-in) | ~15 µs | — |

Run locally:

```bash
cargo bench -p kham-core
# HTML report: target/criterion/report/index.html
```

## Dictionary

The built-in word list (`kham-core/data/words_th.txt`) is CC0-licensed. Custom dictionaries are newline-separated plain text files; lines beginning with `#` are treated as comments.

**Constraint:** Never ship BEST corpus data or any non-CC0 material in this repository.

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE)

at your option.

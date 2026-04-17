---
name: perf-optimize
description: Optimize Rust code for performance — profiling, algorithm tuning, memory layout, and benchmarking. Use when code is slow, dictionary loading takes too long, segmentation throughput needs improvement, or when reviewing hot paths in kham-core.
allowed-tools: Bash(cargo *), Bash(perf *), Bash(flamegraph *), Bash(valgrind *)
---

# Performance Optimization for kham.rs

## Workflow: Profile → Hypothesize → Change → Measure

NEVER optimize without measuring first. Always:
1. Benchmark the current state (baseline)
2. Profile to find the actual bottleneck
3. Make ONE change
4. Benchmark again to prove improvement
5. If no improvement, revert

## Step 1: Profiling Tools

### Quick timing

```bash
cargo bench -p kham-core -- --save-baseline before
# ... make changes ...
cargo bench -p kham-core -- --baseline before
```

### Flamegraph (find hot functions)

```bash
cargo install flamegraph
cargo flamegraph -p kham-cli -- "$(cat testdata/article_10k.txt)"
# Opens flamegraph.svg — look for widest bars
```

### perf stat (CPU counters)

```bash
cargo build -p kham-cli --release
perf stat ./target/release/kham "$(cat testdata/article_10k.txt)"
# Look at: cache-misses, branch-misses, instructions-per-cycle
```

### Heap profiling (memory allocations)

```bash
cargo install dhat
# Add to code temporarily:
# #[global_allocator] static ALLOC: dhat::Alloc = dhat::Alloc;
# let _profiler = dhat::Profiler::new_heap();
cargo run -p kham-cli --release -- "$(cat testdata/article_10k.txt)"
# Opens dhat-heap.json — look for hot allocation sites
```

## Step 2: Common Bottlenecks & Fixes

### Dictionary Loading (large file)

Problem: Loading 80K+ words from text file into Trie is slow.

Optimization ladder (try in order):

```
1. Pre-compiled binary Trie (build.rs)
   - Serialize Trie at compile time → include_bytes!
   - Load = memmap/deserialize, NOT parse text line by line
   - Expected: 100ms → < 1ms

2. Memory-mapped file for custom dict
   - mmap the file, parse lazily
   - Use memchr crate for fast newline scanning

3. Double-Array Trie (DARTS) layout
   - Two arrays: base[] and check[]
   - Linear memory = cache-friendly
   - Lookup = array index arithmetic, no pointer chasing

4. Compile-time perfect hash (if dict is static)
   - phf crate for compile-time perfect hash map
   - O(1) lookup but no prefix search — use alongside Trie
```

Implementation pattern for pre-compiled Trie:

```rust
// build.rs
fn main() {
    let words = std::fs::read_to_string("data/words_th.txt").unwrap();
    let trie = DartsTrie::build(words.lines());
    let bytes = trie.serialize();  // custom binary format
    std::fs::write(
        format!("{}/dict.bin", std::env::var("OUT_DIR").unwrap()),
        bytes
    ).unwrap();
}

// src/dict.rs
static DICT_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/dict.bin"));

pub fn default_dict() -> DartsTrie {
    // Zero-copy deserialize — just cast bytes to struct
    DartsTrie::from_bytes(DICT_BYTES)
}
```

### Segmentation (DAG construction)

Problem: Building DAG scans Trie at every position → O(n × max_word_len).

Optimizations:

```
1. Aho-Corasick automaton instead of per-position Trie lookup
   - Build automaton from dictionary once
   - Single pass over input finds ALL matches
   - Use `aho-corasick` crate or implement on DARTS
   - Expected: 2-5x faster for long text

2. Skip positions inside known matches
   - If word "ประเทศไทย" matched at pos 0..27
   - Still need DAG edges at 0,9,18 (internal positions)
   - But can skip non-TCC-boundary positions

3. Reuse TCC position set
   - Compute TCC boundaries once → BitVec
   - DAG only adds edges at TCC boundary positions
   - Reduces DAG edges significantly
```

### String Operations

Problem: UTF-8 Thai = 3 bytes per char, iteration is slow.

```
1. Work with byte offsets, not char indices
   - NEVER use .chars().nth(n) — O(n) each time
   - Use byte slicing: &text[start..end]
   - Pre-compute char-to-byte offset table if needed

2. Avoid String allocation in hot path
   - Return &str references into input (zero-copy)
   - Token { text: &'a str } not Token { text: String }
   - Use SmallVec for short token lists (< 32 tokens)

3. SIMD-friendly patterns
   - memchr for fast byte scanning
   - Batch UTF-8 validation instead of per-char
```

### Memory Layout

```
1. Struct of Arrays (SoA) for token output
   - Instead of Vec<Token> (AoS)
   - Use separate Vec<Range<usize>> + Vec<TokenKind>
   - Better cache utilization when iterating one field

2. Arena allocation for temporary DAG
   - bumpalo crate for arena allocator
   - Allocate all DAG nodes in one contiguous block
   - Drop entire arena at once after segmentation

3. Pre-allocate output Vec
   - Estimate token count: input.len() / avg_word_len (≈5 bytes for Thai)
   - Vec::with_capacity(estimated_count)
```

## Step 3: Micro-Benchmark Template

For testing a specific optimization:

```rust
// benches/dict_load.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_dict_from_text(c: &mut Criterion) {
    let text = include_str!("../data/words_th.txt");
    c.bench_function("dict_from_text", |b| {
        b.iter(|| DartsTrie::build(text.lines()))
    });
}

fn bench_dict_from_binary(c: &mut Criterion) {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/dict.bin"));
    c.bench_function("dict_from_binary", |b| {
        b.iter(|| DartsTrie::from_bytes(bytes))
    });
}

fn bench_trie_lookup(c: &mut Criterion) {
    let dict = default_dict();
    let text = "ประเทศไทย";
    c.bench_function("trie_prefix_search", |b| {
        b.iter(|| dict.prefix_search(text))
    });
}

criterion_group!(benches, bench_dict_from_text, bench_dict_from_binary, bench_trie_lookup);
criterion_main!(benches);
```

## Optimization Checklist (before PR)

- [ ] Baseline benchmark saved (`--save-baseline before`)
- [ ] Profiler identifies THIS code as bottleneck (not guessing)
- [ ] Only ONE optimization per commit
- [ ] After-benchmark shows measurable improvement
- [ ] No regression in other benchmarks
- [ ] `cargo clippy` clean
- [ ] `cargo test` passes
- [ ] Comment in code explaining WHY this optimization exists

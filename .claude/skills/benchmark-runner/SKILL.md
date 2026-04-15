---
name: benchmark-runner
description: Run and compare segmentation benchmarks. Use when measuring performance, comparing kham against nlpO3 or PyThaiNLP, profiling bottlenecks, or validating that a PR does not regress performance.
---

# Benchmark Guide

## Rust Benchmarks (criterion)

```bash
# Run all benchmarks
cargo bench -p kham-core

# Run specific benchmark
cargo bench -p kham-core -- segment_short
cargo bench -p kham-core -- segment_long

# Generate HTML report
# → opens target/criterion/report/index.html
```

## Benchmark Structure

Place benchmarks in `kham-core/benches/`:

```rust
// benches/segmentation.rs
use criterion::{criterion_group, criterion_main, Criterion, black_box};
use kham_core::Tokenizer;

fn bench_segment_short(c: &mut Criterion) {
    let tok = Tokenizer::default();
    let text = "ฉันรักประเทศไทย";
    c.bench_function("segment_short", |b| {
        b.iter(|| tok.segment(black_box(text)))
    });
}

fn bench_segment_long(c: &mut Criterion) {
    let tok = Tokenizer::default();
    let text = std::fs::read_to_string("testdata/article_1k.txt").unwrap();
    c.bench_function("segment_1k_words", |b| {
        b.iter(|| tok.segment(black_box(&text)))
    });
}

fn bench_dict_load(c: &mut Criterion) {
    c.bench_function("dict_load", |b| {
        b.iter(|| Tokenizer::default())
    });
}

criterion_group!(benches, bench_segment_short, bench_segment_long, bench_dict_load);
criterion_main!(benches);
```

## Comparison Against nlpO3

Use `scripts/compare.py` to run both kham-cli and nlpO3 on the same input:

```bash
python3 .claude/skills/benchmark-runner/scripts/compare.py \
  --input testdata/article_1k.txt \
  --iterations 100
```

## Key Metrics

- **Throughput**: MB/sec of input text processed
- **Latency p50/p99**: per-call latency distribution
- **Dict load time**: one-time startup cost
- **Memory**: peak RSS during segmentation (use `/usr/bin/time -v`)

## Performance Targets

| Metric | Target | Rationale |
|---|---|---|
| Short text (< 100 chars) | < 10 µs | Interactive use |
| Long text (1000 words) | < 1 ms | Batch processing |
| Dict load | < 50 ms | Startup cost |
| WASM segment short | < 50 µs | Browser responsiveness |
| vs nlpO3 | ≥ parity | Must not be slower |

## Regression Check

Before merging any PR that touches segmenter or dict:

```bash
# On main branch
cargo bench -p kham-core -- --save-baseline main

# On PR branch
cargo bench -p kham-core -- --baseline main
```

Criterion will report % change and flag regressions.

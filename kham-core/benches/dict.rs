//! Criterion benchmarks for the Double-Array Trie dictionary.
//!
//! Run with:
//!   cargo bench -p kham-core --bench dict

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kham_core::dict::{Dict, BUILTIN_WORDS};

// ---------------------------------------------------------------------------
// Benchmark inputs
// ---------------------------------------------------------------------------

const WORDS_IN_DICT: &[&str] = &["กิน", "ข้าว", "สวัสดี", "ธนาคาร", "แห่ง"];
const WORDS_NOT_IN_DICT: &[&str] = &["xxxxxx", "zzzzzz", "unknown", "missing"];

// Anchor texts for prefix search — longer strings surface more candidates.
const PREFIX_TEXTS: &[(&str, &str)] = &[
    ("short", "กินข้าว"),
    ("medium", "สวัสดีชาวโลกคนที่นี่"),
    ("long", "ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ"),
];

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Dict construction from the built-in word list.
fn bench_dict_from_builtin(c: &mut Criterion) {
    c.bench_function("dict/from_builtin_word_list", |b| {
        b.iter(|| {
            let d = Dict::from_word_list(BUILTIN_WORDS);
            criterion::black_box(d);
        });
    });
}

/// Dict construction from a small custom word list (simulates user-supplied dict).
fn bench_dict_from_small_list(c: &mut Criterion) {
    let small = "กิน\nข้าว\nปลา\nน้ำ\nสวัสดี\nธนาคาร\nแห่ง\nชาวโลก\n";
    c.bench_function("dict/from_small_word_list", |b| {
        b.iter(|| {
            let d = Dict::from_word_list(small);
            criterion::black_box(d);
        });
    });
}

/// `contains()` — positive lookups (words known to be in the dict).
fn bench_contains_hit(c: &mut Criterion) {
    let dict = Dict::from_word_list(BUILTIN_WORDS);
    let mut group = c.benchmark_group("dict/contains/hit");
    for &word in WORDS_IN_DICT {
        group.throughput(Throughput::Bytes(word.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(word), word, |b, w| {
            b.iter(|| criterion::black_box(dict.contains(w)));
        });
    }
    group.finish();
}

/// `contains()` — negative lookups (words not in the dict).
fn bench_contains_miss(c: &mut Criterion) {
    let dict = Dict::from_word_list(BUILTIN_WORDS);
    let mut group = c.benchmark_group("dict/contains/miss");
    for &word in WORDS_NOT_IN_DICT {
        group.throughput(Throughput::Bytes(word.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(word), word, |b, w| {
            b.iter(|| criterion::black_box(dict.contains(w)));
        });
    }
    group.finish();
}

/// `prefixes()` — anchored prefix search, the hot path in the segmenter DAG.
fn bench_prefixes(c: &mut Criterion) {
    let dict = Dict::from_word_list(BUILTIN_WORDS);
    let mut group = c.benchmark_group("dict/prefixes");
    for &(label, text) in PREFIX_TEXTS {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("text", label), text, |b, t| {
            b.iter(|| criterion::black_box(dict.prefixes(t)));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_dict_from_builtin,
    bench_dict_from_small_list,
    bench_contains_hit,
    bench_contains_miss,
    bench_prefixes,
);
criterion_main!(benches);

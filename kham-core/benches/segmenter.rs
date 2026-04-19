//! Criterion benchmarks for the full segmentation pipeline.
//!
//! Run with:
//!   cargo bench -p kham-core --bench segmenter
//!
//! HTML reports are written to target/criterion/.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kham_core::fts::FtsTokenizer;
use kham_core::Tokenizer;

// ---------------------------------------------------------------------------
// Benchmark inputs
// ---------------------------------------------------------------------------

/// Short sentence — common case, all words in built-in dict.
const SHORT: &str = "กินข้าวกับปลา";

/// Medium sentence — realistic Thai prose (~50 chars).
const MEDIUM: &str = "สวัสดีชาวโลกคนที่นี่ไปมาที่ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ";

/// Long paragraph — stress test (~200 chars, repeated medium sentence).
const LONG: &str = concat!(
    "สวัสดีชาวโลกคนที่นี่ไปมาที่ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ",
    "สวัสดีชาวโลกคนที่นี่ไปมาที่ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ",
    "สวัสดีชาวโลกคนที่นี่ไปมาที่ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ",
);

/// Mixed Thai + Latin + Number — tests the pre-tokenizer split overhead.
const MIXED: &str = "ธนาคาร100แห่งสวัสดีhello123คน42ไปมา";

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Measure `Tokenizer::new()` — dict construction from the built-in word list.
/// This is pay-once-at-startup cost; not in the hot path.
fn bench_tokenizer_new(c: &mut Criterion) {
    c.bench_function("tokenizer_new", |b| {
        b.iter(|| {
            let tok = Tokenizer::new();
            criterion::black_box(tok);
        });
    });
}

/// Measure `segment()` throughput for pure-Thai inputs at three sizes.
///
/// Group ID: `segment/by_length` — criterion reports MB/s via `Throughput::Bytes`.
fn bench_segment_by_length(c: &mut Criterion) {
    let tok = Tokenizer::new();
    let inputs = [("short", SHORT), ("medium", MEDIUM), ("long", LONG)];

    let mut group = c.benchmark_group("segment/by_length");
    for (label, text) in inputs {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), text, |b, text| {
            b.iter(|| criterion::black_box(tok.segment(text)));
        });
    }
    group.finish();
}

/// Measure `segment()` throughput for mixed-script inputs.
///
/// Group ID: `segment/mixed` — exercises the pre-tokenizer split overhead
/// (Thai→Latin→Number boundary detection) in addition to DP cost.
fn bench_segment_mixed(c: &mut Criterion) {
    let tok = Tokenizer::new();

    // Three inputs at increasing density of script-boundary crossings.
    let inputs: &[(&str, &str)] = &[
        ("sparse", "ธนาคาร100แห่ง"),
        ("medium", MIXED),
        ("dense", "a1ก2b3ข4c5ค6d7ง8e9จ10"),
    ];

    let mut group = c.benchmark_group("segment/mixed");
    for &(label, text) in inputs {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), text, |b, text| {
            b.iter(|| criterion::black_box(tok.segment(text)));
        });
    }
    group.finish();
}

/// Measure `normalize()` — tone dedup + Sara Am composition pass.
fn bench_normalize(c: &mut Criterion) {
    let tok = Tokenizer::new();
    let inputs = [
        ("clean_short", SHORT),
        ("clean_medium", MEDIUM),
        ("clean_long", LONG),
    ];

    let mut group = c.benchmark_group("normalize");
    for (label, text) in inputs {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("thai", label), text, |b, text| {
            b.iter(|| criterion::black_box(tok.normalize(text)));
        });
    }
    group.finish();
}

/// Measure `normalize()` + `segment()` together — the recommended two-step API.
fn bench_normalize_then_segment(c: &mut Criterion) {
    let tok = Tokenizer::new();
    c.bench_function("normalize_then_segment/medium", |b| {
        b.iter(|| {
            let normalized = tok.normalize(MEDIUM);
            let tokens = tok.segment(&normalized);
            criterion::black_box(tokens.len())
        });
    });
}

// ---------------------------------------------------------------------------
// FTS pipeline benchmarks
// ---------------------------------------------------------------------------

/// Measure `FtsTokenizer::new()` — construction cost of the full FTS stack.
///
/// Loads stopwords, POS table, NE gazetteer, and synonym map on top of the
/// base `Tokenizer`. Compare against `tokenizer_new` to quantify the overhead
/// of the extra tables.
fn bench_fts_new(c: &mut Criterion) {
    c.bench_function("fts/new", |b| {
        b.iter(|| {
            let fts = FtsTokenizer::new();
            criterion::black_box(fts);
        });
    });
}

/// Measure `segment_for_fts()` throughput for pure-Thai inputs.
///
/// Group ID: `fts/segment_for_fts` — compare against `segment/by_length` to
/// quantify the overhead of NE tagging + POS tagging + stopword lookup added
/// by the FTS pipeline on every call.
fn bench_fts_segment_for_fts(c: &mut Criterion) {
    let fts = FtsTokenizer::new();
    let inputs = [("short", SHORT), ("medium", MEDIUM), ("long", LONG)];

    let mut group = c.benchmark_group("fts/segment_for_fts");
    for (label, text) in inputs {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), text, |b, text| {
            b.iter(|| criterion::black_box(fts.segment_for_fts(text)));
        });
    }
    group.finish();
}

/// Measure `index_tokens()` — `segment_for_fts` + stopword filter.
///
/// The difference between this and `fts/segment_for_fts` isolates the cost
/// of the filter pass (expected to be negligible).
fn bench_fts_index_tokens(c: &mut Criterion) {
    let fts = FtsTokenizer::new();

    let mut group = c.benchmark_group("fts/index_tokens");
    group.throughput(Throughput::Bytes(MEDIUM.len() as u64));
    group.bench_with_input(BenchmarkId::from_parameter("medium"), MEDIUM, |b, text| {
        b.iter(|| criterion::black_box(fts.index_tokens(text)));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_tokenizer_new,
    bench_segment_by_length,
    bench_segment_mixed,
    bench_normalize,
    bench_normalize_then_segment,
    bench_fts_new,
    bench_fts_segment_for_fts,
    bench_fts_index_tokens,
);
criterion_main!(benches);

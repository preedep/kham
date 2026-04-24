//! Criterion benchmarks for the kham SQLite FTS5 tokenizer extension.
//!
//! Run with:
//!   cargo build -p kham-sqlite --release
//!   cargo bench -p kham-sqlite --bench fts
//!
//! Measures:
//! - `index/*`  — INSERT throughput (drives xTokenize for document indexing)
//! - `query/*`  — SELECT … MATCH latency for single-word and multi-word queries
//! - `snippet`  — snippet() cost (requires byte-accurate iStart/iEnd offsets)

use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rusqlite::{Connection, LoadExtensionGuard};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Path to the kham-sqlite release dylib.
/// `cargo bench` runs from the workspace root so target/release is reachable.
fn ext_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // workspace root
    #[cfg(target_os = "macos")]
    p.push("target/release/libkham_sqlite.dylib");
    #[cfg(target_os = "linux")]
    p.push("target/release/libkham_sqlite.so");
    #[cfg(target_os = "windows")]
    p.push("target/release/kham_sqlite.dll");
    p
}

/// Open an in-memory SQLite database with the kham extension loaded and an
/// FTS5 virtual table ready for use.
fn open_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open :memory:");
    unsafe {
        let _guard = LoadExtensionGuard::new(&conn).expect("load_extension guard");
        conn.load_extension(ext_path(), Some("sqlite3_kham_init"))
            .expect("load kham extension");
    }
    conn.execute_batch("CREATE VIRTUAL TABLE bench_docs USING fts5(body, tokenize='kham');")
        .expect("create fts5 table");
    conn
}

// ---------------------------------------------------------------------------
// Benchmark inputs
// ---------------------------------------------------------------------------

const SHORT: &str = "กินข้าวกับปลา";

const MEDIUM: &str = "สวัสดีชาวโลกคนที่นี่ไปมาที่ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ";

const LONG: &str = concat!(
    "สวัสดีชาวโลกคนที่นี่ไปมาที่ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ",
    "สวัสดีชาวโลกคนที่นี่ไปมาที่ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ",
    "สวัสดีชาวโลกคนที่นี่ไปมาที่ธนาคารแห่งนั้นกินข้าวกับปลาและน้ำ",
);

const MIXED: &str = "ธนาคาร100แห่งสวัสดีhello123คน42ไปมา";

// Pre-populate database for query benchmarks (1 000 rows of MEDIUM).
const QUERY_ROWS: usize = 1_000;

// ---------------------------------------------------------------------------
// INDEX benchmarks — measures INSERT + xTokenize throughput
// ---------------------------------------------------------------------------

/// INSERT a single document; measures end-to-end xTokenize cost.
fn bench_index_single(c: &mut Criterion) {
    let conn = open_db();

    let inputs = [
        ("short", SHORT),
        ("medium", MEDIUM),
        ("long", LONG),
        ("mixed", MIXED),
    ];
    let mut group = c.benchmark_group("index/single");
    for (label, text) in inputs {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), text, |b, text| {
            b.iter(|| {
                conn.execute("INSERT INTO bench_docs(body) VALUES (?1)", [text])
                    .unwrap();
            });
        });
    }
    group.finish();
}

/// INSERT 100 rows in a transaction; isolates bulk-indexing throughput.
fn bench_index_batch(c: &mut Criterion) {
    let conn = open_db();
    const BATCH: usize = 100;

    let inputs = [("short", SHORT), ("medium", MEDIUM), ("long", LONG)];
    let mut group = c.benchmark_group("index/batch_100");
    for (label, text) in inputs {
        group.throughput(Throughput::Bytes((text.len() * BATCH) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), text, |b, text| {
            b.iter(|| {
                conn.execute_batch("BEGIN;").unwrap();
                for _ in 0..BATCH {
                    conn.execute("INSERT INTO bench_docs(body) VALUES (?1)", [text])
                        .unwrap();
                }
                conn.execute_batch("COMMIT;").unwrap();
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// QUERY benchmarks — measures FTS5 match + xTokenize of the query term
// ---------------------------------------------------------------------------

/// Set up a fresh in-memory DB with QUERY_ROWS pre-inserted rows.
fn open_populated_db() -> Connection {
    let conn = open_db();
    conn.execute_batch("BEGIN;").unwrap();
    for _ in 0..QUERY_ROWS {
        conn.execute("INSERT INTO bench_docs(body) VALUES (?1)", [MEDIUM])
            .unwrap();
    }
    conn.execute_batch("COMMIT;").unwrap();
    conn
}

/// Single-word query: `SELECT rowid FROM bench_docs WHERE bench_docs MATCH ?`.
fn bench_query_single_word(c: &mut Criterion) {
    let conn = open_populated_db();

    let queries = [
        ("thai_common", "ข้าว"),
        ("thai_rare", "ปลา"),
        ("number", "100"),
        ("latin", "hello"),
    ];

    let mut group = c.benchmark_group("query/single_word");
    for (label, q) in queries {
        group.bench_with_input(BenchmarkId::from_parameter(label), q, |b, q| {
            b.iter(|| {
                let mut stmt = conn
                    .prepare_cached("SELECT rowid FROM bench_docs WHERE bench_docs MATCH ?1")
                    .unwrap();
                let count: usize = stmt.query_map([q], |_| Ok(())).unwrap().count();
                criterion::black_box(count)
            });
        });
    }
    group.finish();
}

/// snippet() call — exercises highlight reconstruction using iStart/iEnd.
fn bench_snippet(c: &mut Criterion) {
    let conn = open_populated_db();

    c.bench_function("query/snippet", |b| {
        b.iter(|| {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT snippet(bench_docs, 0, '<b>', '</b>', '...', 10) \
                     FROM bench_docs WHERE bench_docs MATCH 'ข้าว' LIMIT 10",
                )
                .unwrap();
            let results: Vec<String> = stmt
                .query_map([], |row| row.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            criterion::black_box(results)
        });
    });
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_index_single,
    bench_index_batch,
    bench_query_single_word,
    bench_snippet,
);
criterion_main!(benches);

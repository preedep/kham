# Benchmarks

Run locally:

```bash
cargo bench -p kham-core             # core segmentation + FTS pipeline
cargo build -p kham-sqlite --release # must build dylib before benchmarking
cargo bench -p kham-sqlite           # SQLite FTS5 extension
# HTML report: target/criterion/report/index.html
```

## Environment

| Field | Value |
|---|---|
| CPU | Apple M-series (arm64) |
| OS | macOS 26.4.1 |
| Rust | 1.94.1 (stable) |
| Profile | release (LTO enabled) |
| Built-in dictionary | 62,102 words · 670,460 DARTS states · 5.1 MiB |
| TNC frequency table | 106,125 entries |

## Segmentation throughput (`segment/by_length`)

Pure Thai input, built-in dictionary, no custom dict.

| Input | Size | Time (median) | Throughput |
|---|---|---|---|
| short | 39 B | 1.12 µs | 33.2 MiB/s |
| medium | 180 B | 5.09 µs | 33.7 MiB/s |
| long | 540 B | 14.95 µs | 34.4 MiB/s |

## Mixed-script throughput (`segment/mixed`)

Thai + Latin + Number in the same input, measuring pre-tokenizer boundary overhead.

| Input | Size | Time (median) | Throughput |
|---|---|---|---|
| sparse (`ธนาคาร100แห่ง`) | 33 B | 948 ns | 33.2 MiB/s |
| medium (multi-boundary) | 79 B | 2.23 µs | 33.8 MiB/s |
| dense (alternating script) | 31 B | 770 ns | 38.4 MiB/s |

## Normalization throughput (`normalize/thai`)

| Input | Size | Time (median) | Throughput |
|---|---|---|---|
| short | 39 B | 107.6 ns | 345 MiB/s |
| medium | 180 B | 285.9 ns | 600 MiB/s |
| long | 540 B | 743.7 ns | 692 MiB/s |

## Dictionary construction (`dict/construction`)

| Operation | Time (median) | Notes |
|---|---|---|
| `builtin_dict()` — binary blob load | 92.9 µs | pay-once startup cost |
| `Tokenizer::new()` | 33.4 ms | includes freq table parse |
| `FtsTokenizer::new()` | 46.7 ms | includes NE/POS/synonym tables |
| `Dict::from_word_list` — 62k words | 1.63 s | only when merging a custom dict |

> `builtin_dict()` is **~17,500×** faster than `Dict::from_word_list` because the DARTS trie is
> pre-compiled by `build.rs` at compile time; runtime cost is a single O(S) binary decode pass.
> `Dict::from_word_list` runs only when a user-supplied custom dictionary is merged with the built-in list.

## Dictionary lookup (`dict/contains`, `dict/prefixes`)

| Operation | Time (median) | Throughput |
|---|---|---|
| `contains` — hit (3-byte word `กิน`) | 11.1 ns | 770 MiB/s |
| `contains` — miss (non-word) | 1.22 ns | 4.5 GiB/s |
| `prefixes` — short anchor | 63.8 ns | — |
| `prefixes` — medium anchor | 80.4 ns | — |
| `prefixes` — long anchor | 100 ns | — |

## Accuracy

| Metric | Value |
|---|---|
| F1 (word boundary) | 1.000 on 228 curated test cases |
| Sentence-level agreement with PyThaiNLP newmm | 94.9% |
| Dictionary states | 670,460 DARTS states |
| Speedup vs `Dict::from_word_list` | ~17,500× |

## SQLite FTS5 extension (`kham-sqlite`)

Criterion benchmarks via `rusqlite` with bundled SQLite (FTS5 enabled), in-memory database.
Run with `cargo build -p kham-sqlite --release && cargo bench -p kham-sqlite`.

Pipeline per `xTokenize` call: normalize → NE tag → stopword → POS → synonym expand → RTGS romanization.
Synonyms and RTGS forms are emitted via `FTS5_TOKEN_COLOCATED` at the same position as the primary token.

### Indexing — INSERT throughput (`index/*`)

| Benchmark | Input | Size | Time (median) | Throughput |
|---|---|---|---|---|
| `index/single/short` | `กินข้าวกับปลา` | 21 B | 20.2 µs | 1.84 MiB/s |

### Query latency (`query/*`)

Table pre-populated with 1,000 rows of medium Thai prose.

| Benchmark | Query | Time (median) |
|---|---|---|
| `query/single_word/thai_common` | `ข้าว` | 4.28 µs |
| `query/single_word/thai_rare` | `ปลา` | 139.7 µs |
| `query/snippet` | `ข้าว` (top 10 snippets) | 4.24 µs |

## PostgreSQL extension (`kham-pg`)

Benchmarked at the SQL level using `pgbench` inside the Docker test container.

### 1 · Latency — psql `\timing`

```sql
\timing on
SELECT to_tsvector('kham', 'กินข้าวกับปลา Python 3 สำหรับนักพัฒนา');

EXPLAIN (ANALYZE, BUFFERS)
SELECT to_tsvector('kham', body) FROM documents LIMIT 1000;
```

### 2 · Throughput — `pgbench`

Create `bench_fts.sql`:

```sql
SELECT to_tsvector('kham', 'กินข้าวกับปลา Python 3 สำหรับนักพัฒนา');
```

Run via Docker:

```bash
# Terminal 1 — watch CPU/memory
docker stats docker-regress-1

# Terminal 2 — throughput bench (4 clients, 30 seconds)
docker exec docker-regress-1 pgbench \
  -n -c 4 -j 4 -T 30 \
  -f /bench_fts.sql \
  -h /var/run/postgresql -p 15432 kham_test
```

### 3 · Index build time

```sql
CREATE TABLE docs (id serial, body text);
INSERT INTO docs (body)
  SELECT 'กินข้าวกับปลา Python ' || g
  FROM generate_series(1, 100000) g;

\timing on
CREATE INDEX ON docs USING gin(to_tsvector('kham', body));

SELECT count(*) FROM docs
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'ปลา');
```

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
| Built-in dictionary | 62,102 words · 669,387 DARTS states · 5.1 MiB |
| TNC frequency table | 106,125 entries |

## Segmentation throughput (`segment/by_length`)

Pure Thai input, built-in dictionary, no custom dict.

| Input | Size | Time (median) | Throughput |
|---|---|---|---|
| short | 37 B | 879 ns | 42.3 MiB/s |
| medium | 182 B | 3.80 µs | 45.1 MiB/s |
| long | 546 B | 10.9 µs | 47.1 MiB/s |

## Mixed-script throughput (`segment/mixed`)

Thai + Latin + Number in the same input, measuring pre-tokenizer boundary overhead.

| Input | Size | Time (median) | Throughput |
|---|---|---|---|
| sparse (`ธนาคาร100แห่ง`) | 26 B | 744 ns | 42.3 MiB/s |
| medium (multi-boundary) | 74 B | 1.73 µs | 43.5 MiB/s |
| dense (alternating script) | 29 B | 535 ns | 55.3 MiB/s |

## Normalize + segment (`normalize_then_segment/medium`)

| Operation | Time (median) |
|---|---|
| `normalize()` then `segment()` on medium input | 4.09 µs |

## Normalization throughput (`normalize/thai`)

| Input | Size | Time (median) | Throughput |
|---|---|---|---|
| short | 37 B | 79.9 ns | 465 MiB/s |
| medium | 182 B | 199 ns | 864 MiB/s |
| long | 546 B | 507 ns | 1.0 GiB/s |

## Dictionary construction (`dict/construction`)

| Operation | Time (median) | Notes |
|---|---|---|
| `builtin_dict()` — binary blob load | 78 µs | pay-once startup cost |
| `Dict::from_word_list` — 62k words | 980 ms | only when merging a custom dict |
| `Dict::from_word_list` — 8-word list | 3.72 µs | small custom dict |
| `dict/file/read_and_build` — disk + build | 1.01 s | `kham --dict <file>` startup |
| `Tokenizer::builder().dict_file().build()` | 1.04 s | full CLI code path with custom dict |

> `builtin_dict()` is **~12,500×** faster than `Dict::from_word_list` because the DARTS trie is
> pre-compiled by `build.rs` at compile time; runtime cost is a single O(S) binary decode pass.
> `Dict::from_word_list` runs only when a user-supplied custom dictionary is merged with the built-in list.

## Dictionary lookup (`dict/contains`, `dict/prefixes`)

| Operation | Time (median) | Throughput |
|---|---|---|
| `contains` — hit (3-byte word `กิน`) | 7.1 ns | 1.18 GiB/s |
| `contains` — hit (12-byte word `สวัสดี`) | 18.3 ns | 940 MiB/s |
| `contains` — miss (ASCII non-word) | 744 ps | 7.5–8.8 GiB/s |
| `prefixes` — short anchor (7 B) | 42.3 ns | 473 MiB/s |
| `prefixes` — medium anchor (60 B) | 36.7 ns | 1.52 GiB/s |
| `prefixes` — long anchor (97 B) | 74.5 ns | 1.24 GiB/s |

## TNC frequency table (`freq/construction`, `freq/get`)

| Operation | Time (median) | Notes |
|---|---|---|
| `FreqMap::builtin()` — parse 106k TSV entries | 22.1 ms | pay-once startup cost |
| `FreqMap::get` — common word hit (`กิน`) | 67.8 ns | O(log n) BTreeMap |
| `FreqMap::get` — rare word hit | 48.6 ns | |
| `FreqMap::get` — miss | 56.5 ns | |

> `FreqMap::builtin()` startup cost (~22 ms) is the dominant component of `Tokenizer::new()` (~20 ms total).
> It is paid once per tokenizer instance; the returned `FreqMap` is reused across all `segment()` calls.

## SQLite FTS5 extension (`kham-sqlite`)

Criterion benchmarks via `rusqlite` with bundled SQLite (FTS5 enabled), in-memory database.
Run with `cargo build -p kham-sqlite --release && cargo bench -p kham-sqlite`.

Pipeline per `xTokenize` call: normalize → NE tag → stopword → POS → synonym expand → RTGS romanization.
Synonyms and RTGS forms are emitted via `FTS5_TOKEN_COLOCATED` at the same position as the primary token.

### Indexing — INSERT throughput (`index/*`)

`index/single` measures one INSERT per autocommit transaction (includes SQLite journal overhead).
`index/batch_100` wraps 100 INSERTs in a single transaction — reflects real bulk-indexing throughput.

| Benchmark | Input | Size | Time (median) | Throughput |
|---|---|---|---|---|
| `index/single/short` | `กินข้าวกับปลา` | 21 B | 15.5 µs | 2.47 MiB/s |
| `index/single/medium` | ~63 B Thai prose | 63 B | 41.8 µs | 4.14 MiB/s |
| `index/single/long` | 3× medium | 189 B | 94.3 µs | 5.46 MiB/s |
| `index/single/mixed` | Thai + Latin + Number | 37 B | 32.4 µs | 2.32 MiB/s |
| `index/batch_100/short` | 100 × short | 2.1 KB | 640 µs (**6.4 µs/doc**) | 6.0 MiB/s |
| `index/batch_100/medium` | 100 × medium | 6.3 KB | 2.54 ms (**25.4 µs/doc**) | 7.1 MiB/s |
| `index/batch_100/long` | 100 × long | 18.9 KB | 6.75 ms (**67.5 µs/doc**) | 7.6 MiB/s |

> Per-document cost includes: normalization + NE tagging + POS + stopword + synonym expand + RTGS.
> SQLite transaction overhead still dominates single-INSERT latency; batch mode reflects true
> tokenizer throughput (~6–68 µs/doc depending on input size).

### Query latency (`query/*`)

Table pre-populated with 1 000 rows of the medium input.

| Benchmark | Query | Result rows | Time (median) |
|---|---|---|---|
| `query/single_word/thai_common` | `ข้าว` | 1 000 | 88.3 µs |
| `query/single_word/thai_rare` | `ปลา` | 1 000 | 88.9 µs |
| `query/single_word/number` | `100` | 0 | 1.4 µs |
| `query/single_word/latin` | `hello` | 0 | 1.5 µs |
| `query/snippet` | `ข้าว` (top 10 snippets) | 10 | 417 µs |

> Query latency covers: full FTS pipeline on the query term + FTS5 index lookup + iterating
> 1 000 matching rowids. No-match queries (number / latin) cost only ~1.4 µs (FTS5 index miss
> path; NE/POS pipeline still runs on the query term but is fast for short inputs).

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

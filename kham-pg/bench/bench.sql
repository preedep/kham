-- kham-pg benchmark suite
-- Measures throughput of the key FTS operations provided by the kham extension.
--
-- Metrics:
--   ops/s  — operations per second (higher is better)
--   µs/op  — mean latency per operation in microseconds (lower is better)
--   ms/op  — mean latency in milliseconds (for slower operations)
--
-- Run via:  make -C kham-pg bench
--
-- Section 1: cache-warm batch throughput
--   generate_series cycles only 3 unique inputs; PG caches STABLE function results
--   within a single ExprContext after the first 3 calls. These numbers reflect
--   cache-warm amortised throughput — useful for bulk indexing estimates.
--
-- Section 2: true per-call latency (pgbench)
--   pgbench runs each transaction in a fresh ExprContext; no cross-transaction
--   result cache. Numbers reflect actual single-call tokenizer latency.

CREATE EXTENSION IF NOT EXISTS kham_pg;

-- ── Corpus constants ──────────────────────────────────────────────────────────

DO $$
DECLARE
    n_s    int := 50000;
    n_m    int :=  5000;
    n_l    int :=   500;

    t0     timestamptz;
    e      float8;
    dummy  bigint;
BEGIN
    RAISE NOTICE '';
    RAISE NOTICE '=== kham-pg benchmark (cache-warm batch throughput) ===';
    RAISE NOTICE '%', format('%-42s %12s %10s', 'operation', 'ops/s', 'µs/op');
    RAISE NOTICE '%', repeat('-', 67);

    -- ── to_tsvector ───────────────────────────────────────────────────────────
    -- Cycle through 3 equivalent-length documents so PostgreSQL cannot
    -- constant-fold the STABLE to_tsvector call across generate_series rows.

    -- warmup
    SELECT count(*) INTO dummy FROM (
        SELECT to_tsvector('kham',
            CASE i % 3
                WHEN 0 THEN 'กินข้าวกับปลาและผักสด'
                WHEN 1 THEN 'ปลาสดกับข้าวและผักกินดี'
                ELSE        'ผักสดและปลากินข้าวกับ'
            END)
        FROM generate_series(1, 1000) i
    ) t;

    -- small (~63 bytes)
    t0 := clock_timestamp();
    SELECT count(*) INTO dummy FROM (
        SELECT to_tsvector('kham',
            CASE i % 3
                WHEN 0 THEN 'กินข้าวกับปลาและผักสด'
                WHEN 1 THEN 'ปลาสดกับข้าวและผักกินดี'
                ELSE        'ผักสดและปลากินข้าวกับ'
            END)
        FROM generate_series(1, n_s) i
    ) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'to_tsvector small  (~63 B)',
        to_char((n_s / e)::numeric,        'FM9,999,999,990'),
        to_char((e * 1e6 / n_s)::numeric,  'FM9990.000'));

    -- medium (~630 bytes)
    t0 := clock_timestamp();
    SELECT count(*) INTO dummy FROM (
        SELECT to_tsvector('kham',
            CASE i % 3
                WHEN 0 THEN repeat('กินข้าวกับปลาและผักสด ', 10)
                WHEN 1 THEN repeat('ปลาสดกับข้าวและผักกินดี ', 10)
                ELSE        repeat('ผักสดและปลากินข้าวกับ ', 10)
            END)
        FROM generate_series(1, n_m) i
    ) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'to_tsvector medium (~630 B)',
        to_char((n_m / e)::numeric,        'FM9,999,999,990'),
        to_char((e * 1e6 / n_m)::numeric,  'FM9990.000'));

    -- large (~6.3 KB)
    t0 := clock_timestamp();
    SELECT count(*) INTO dummy FROM (
        SELECT to_tsvector('kham',
            CASE i % 3
                WHEN 0 THEN repeat('กินข้าวกับปลาและผักสด ', 100)
                WHEN 1 THEN repeat('ปลาสดกับข้าวและผักกินดี ', 100)
                ELSE        repeat('ผักสดและปลากินข้าวกับ ', 100)
            END)
        FROM generate_series(1, n_l) i
    ) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'to_tsvector large  (~6.3 KB)',
        to_char((n_l / e)::numeric,        'FM9,999,999,990'),
        to_char((e * 1e6 / n_l)::numeric,  'FM9990.000'));

    -- ── plainto_tsquery ───────────────────────────────────────────────────────

    RAISE NOTICE '%', repeat('-', 67);

    t0 := clock_timestamp();
    SELECT count(*) INTO dummy FROM (
        SELECT plainto_tsquery('kham',
            CASE i % 3
                WHEN 0 THEN 'ปลาทะเล'
                WHEN 1 THEN 'ทะเลปลา'
                ELSE        'ปลาสด'
            END)
        FROM generate_series(1, n_s) i
    ) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'plainto_tsquery (single word)',
        to_char((n_s / e)::numeric,        'FM9,999,999,990'),
        to_char((e * 1e6 / n_s)::numeric,  'FM9990.000'));

    t0 := clock_timestamp();
    SELECT count(*) INTO dummy FROM (
        SELECT plainto_tsquery('kham',
            CASE i % 3
                WHEN 0 THEN 'กินข้าว ปลาทะเล ผักสด'
                WHEN 1 THEN 'ข้าวสวย ปลาทะเล กินดี'
                ELSE        'ผักสด ข้าว ปลา'
            END)
        FROM generate_series(1, n_s) i
    ) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'plainto_tsquery (3 words)',
        to_char((n_s / e)::numeric,        'FM9,999,999,990'),
        to_char((e * 1e6 / n_s)::numeric,  'FM9990.000'));

    RAISE NOTICE '';
END $$;

DO $$
BEGIN
    RAISE NOTICE '=== done ===';
END $$;

-- GIN-indexed scan and ts_rank benchmarks are excluded from the Docker suite:
-- building a stored tsvector column or populating a large table triggers thousands
-- of real to_tsvector calls that make the bench hang inside Docker.
-- To benchmark GIN queries, run against a real PostgreSQL instance with a
-- pre-built index and measure via EXPLAIN ANALYZE or pgbench.

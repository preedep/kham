-- kham-pg benchmark suite
-- Measures throughput of the key FTS operations provided by the kham extension.
--
-- Metrics:
--   ops/s  — operations per second (higher is better)
--   µs/op  — mean latency per operation in microseconds (lower is better)
--   ms/op  — mean latency in milliseconds (for slower operations)
--
-- Run via:  make -C kham-pg bench

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
    RAISE NOTICE '=== kham-pg benchmark ===';
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

-- ── Sequential scan on 10k rows ───────────────────────────────────────────────

DROP TABLE IF EXISTS kham_bench_docs;
CREATE TABLE kham_bench_docs (id serial, body text);

INSERT INTO kham_bench_docs (body)
SELECT
    CASE (i % 5)
        WHEN 0 THEN 'กินข้าวกับปลาทะเลสดและผักรวม'
        WHEN 1 THEN 'Python สำหรับนักพัฒนาซอฟต์แวร์'
        WHEN 2 THEN 'ราคา ๑๕๐ บาทต่อกิโลกรัม'
        WHEN 3 THEN 'ท่องเที่ยวทะเลอ่าวไทยช่วงหน้าร้อน'
        ELSE        'นักพัฒนาเขียนโปรแกรมด้วยภาษา Rust'
    END
FROM generate_series(1, 10000) i;

-- Sequential scan (to_tsvector on every row) is intentionally omitted:
-- at ~50–200 µs/call × 10k rows it takes 30+ seconds per scan in Docker,
-- and is not a realistic production pattern (use a GIN index instead).

-- ── GIN-indexed scan on 100k rows ─────────────────────────────────────────────

INSERT INTO kham_bench_docs (body)
SELECT
    CASE (i % 5)
        WHEN 0 THEN 'กินข้าวกับปลาทะเลสดและผักรวม'
        WHEN 1 THEN 'Python สำหรับนักพัฒนาซอฟต์แวร์'
        WHEN 2 THEN 'ราคา ๑๕๐ บาทต่อกิโลกรัม'
        WHEN 3 THEN 'ท่องเที่ยวทะเลอ่าวไทยช่วงหน้าร้อน'
        ELSE        'นักพัฒนาเขียนโปรแกรมด้วยภาษา Rust'
    END
FROM generate_series(1, 90000) i;

ALTER TABLE kham_bench_docs ADD COLUMN fts tsvector
    GENERATED ALWAYS AS (to_tsvector('kham', body)) STORED;

CREATE INDEX kham_bench_gin ON kham_bench_docs USING GIN (fts);

ANALYZE kham_bench_docs;

DO $$
DECLARE
    t0    timestamptz;
    e     float8;
    dummy bigint;
    n_idx int := 200;
BEGIN
    -- warmup — force index pages into OS cache
    SELECT count(*) INTO dummy
    FROM kham_bench_docs WHERE fts @@ plainto_tsquery('kham', 'ปลา');

    RAISE NOTICE '%', format('%-42s %12s %10s', 'operation', 'queries/s', 'ms/query');
    RAISE NOTICE '%', repeat('-', 67);

    t0 := clock_timestamp();
    FOR i IN 1..n_idx LOOP
        SELECT count(*) INTO dummy
        FROM kham_bench_docs WHERE fts @@ plainto_tsquery('kham', 'ปลา');
    END LOOP;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'GIN scan 100k rows (ปลา)',
        to_char((n_idx / e)::numeric,        'FM9,999,990.00'),
        to_char((e * 1000 / n_idx)::numeric,  'FM9990.000'));

    t0 := clock_timestamp();
    FOR i IN 1..n_idx LOOP
        SELECT count(*) INTO dummy
        FROM kham_bench_docs WHERE fts @@ plainto_tsquery('kham', 'Python นักพัฒนา');
    END LOOP;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'GIN scan 100k rows (mixed script)',
        to_char((n_idx / e)::numeric,        'FM9,999,990.00'),
        to_char((e * 1000 / n_idx)::numeric,  'FM9990.000'));

    -- ── ts_rank ───────────────────────────────────────────────────────────────

    RAISE NOTICE '%', repeat('-', 67);

    n_idx := 100;
    t0 := clock_timestamp();
    FOR i IN 1..n_idx LOOP
        SELECT count(*) INTO dummy FROM (
            SELECT ts_rank(fts, plainto_tsquery('kham', 'ปลา'))
            FROM kham_bench_docs
            WHERE fts @@ plainto_tsquery('kham', 'ปลา')
            ORDER BY 1 DESC
            LIMIT 10
        ) t;
    END LOOP;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'ts_rank top-10 (GIN + rank)',
        to_char((n_idx / e)::numeric,        'FM9,999,990.00'),
        to_char((e * 1000 / n_idx)::numeric,  'FM9990.000'));

    t0 := clock_timestamp();
    FOR i IN 1..n_idx LOOP
        SELECT count(*) INTO dummy FROM (
            SELECT ts_rank(
                setweight(fts, 'A'),
                plainto_tsquery('kham', 'ปลา')
            )
            FROM kham_bench_docs
            WHERE fts @@ plainto_tsquery('kham', 'ปลา')
            ORDER BY 1 DESC
            LIMIT 10
        ) t;
    END LOOP;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%', format('%-42s %12s %10s',
        'ts_rank setweight A top-10',
        to_char((n_idx / e)::numeric,        'FM9,999,990.00'),
        to_char((e * 1000 / n_idx)::numeric,  'FM9990.000'));

    RAISE NOTICE '';
    RAISE NOTICE '=== done ===';
END $$;

DROP TABLE IF EXISTS kham_bench_docs;

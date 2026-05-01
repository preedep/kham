-- kham-pg benchmark suite
-- Measures throughput of the key FTS operations provided by the kham extension.
--
-- Metrics reported for each operation:
--   ops/s  — operations per second (higher is better)
--   ms/op  — mean latency per operation (lower is better)
--
-- Run via:  make -C kham-pg bench

CREATE EXTENSION IF NOT EXISTS kham_pg;

-- ── Corpus constants ──────────────────────────────────────────────────────────

DO $$
DECLARE
    -- Three document sizes exercising different tokenisation depths
    doc_s  text := 'กินข้าวกับปลาและผักสด';
    doc_m  text := repeat('กินข้าวกับปลาและผักสด ', 10);
    doc_l  text := repeat('กินข้าวกับปลาและผักสด ', 100);

    n_s    int := 50000;
    n_m    int :=  5000;
    n_l    int :=   500;

    t0     timestamptz;
    e      float8;
    dummy  bigint;
BEGIN
    RAISE NOTICE '';
    RAISE NOTICE '=== kham-pg benchmark ===';
    RAISE NOTICE '%-42s %10s %10s', 'operation', 'ops/s', 'ms/op';
    RAISE NOTICE '%s', repeat('-', 65);

    -- ── to_tsvector ───────────────────────────────────────────────────────────

    -- warmup
    SELECT count(*) INTO dummy
    FROM (SELECT to_tsvector('kham', doc_s) FROM generate_series(1, 1000)) t;

    -- small (~63 bytes)
    t0 := clock_timestamp();
    SELECT count(*) INTO dummy
    FROM (SELECT to_tsvector('kham', doc_s) FROM generate_series(1, n_s)) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'to_tsvector small  (~63 B)', n_s / e, e * 1000 / n_s;

    -- medium (~630 bytes)
    t0 := clock_timestamp();
    SELECT count(*) INTO dummy
    FROM (SELECT to_tsvector('kham', doc_m) FROM generate_series(1, n_m)) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'to_tsvector medium (~630 B)', n_m / e, e * 1000 / n_m;

    -- large (~6.3 KB)
    t0 := clock_timestamp();
    SELECT count(*) INTO dummy
    FROM (SELECT to_tsvector('kham', doc_l) FROM generate_series(1, n_l)) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'to_tsvector large  (~6.3 KB)', n_l / e, e * 1000 / n_l;

    -- ── plainto_tsquery ───────────────────────────────────────────────────────

    RAISE NOTICE '%s', repeat('-', 65);

    t0 := clock_timestamp();
    SELECT count(*) INTO dummy
    FROM (SELECT plainto_tsquery('kham', 'ปลาทะเล') FROM generate_series(1, n_s)) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'plainto_tsquery (single word)', n_s / e, e * 1000 / n_s;

    t0 := clock_timestamp();
    SELECT count(*) INTO dummy
    FROM (SELECT plainto_tsquery('kham', 'กินข้าว ปลาทะเล ผักสด') FROM generate_series(1, n_s)) t;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'plainto_tsquery (3 words)', n_s / e, e * 1000 / n_s;

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

DO $$
DECLARE
    t0    timestamptz;
    e     float8;
    dummy bigint;
    n_seq int := 100;   -- how many times to repeat the full-table scan
BEGIN
    -- warmup
    SELECT count(*) INTO dummy
    FROM kham_bench_docs
    WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'ปลา');

    RAISE NOTICE '%-42s %10s %10s', 'operation', 'ops/s', 'ms/op';
    RAISE NOTICE '%s', repeat('-', 65);

    t0 := clock_timestamp();
    FOR i IN 1..n_seq LOOP
        SELECT count(*) INTO dummy
        FROM kham_bench_docs
        WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'ปลา');
    END LOOP;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.1f',
        '@@ seq scan 10k rows (ปลา)', n_seq / e, e * 1000 / n_seq;

    t0 := clock_timestamp();
    FOR i IN 1..n_seq LOOP
        SELECT count(*) INTO dummy
        FROM kham_bench_docs
        WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'Python นักพัฒนา');
    END LOOP;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.1f',
        '@@ seq scan 10k rows (mixed script)', n_seq / e, e * 1000 / n_seq;

    RAISE NOTICE '';
END $$;

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
    n_idx int := 500;
BEGIN
    -- warmup — force index build into OS cache
    SELECT count(*) INTO dummy
    FROM kham_bench_docs WHERE fts @@ plainto_tsquery('kham', 'ปลา');

    RAISE NOTICE '%-42s %10s %10s', 'operation', 'ops/s', 'ms/op';
    RAISE NOTICE '%s', repeat('-', 65);

    t0 := clock_timestamp();
    FOR i IN 1..n_idx LOOP
        SELECT count(*) INTO dummy
        FROM kham_bench_docs WHERE fts @@ plainto_tsquery('kham', 'ปลา');
    END LOOP;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'GIN scan 100k rows (ปลา)', n_idx / e, e * 1000 / n_idx;

    t0 := clock_timestamp();
    FOR i IN 1..n_idx LOOP
        SELECT count(*) INTO dummy
        FROM kham_bench_docs WHERE fts @@ plainto_tsquery('kham', 'Python นักพัฒนา');
    END LOOP;
    e := extract(epoch from (clock_timestamp() - t0));
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'GIN scan 100k rows (mixed script)', n_idx / e, e * 1000 / n_idx;

    -- ── ts_rank ───────────────────────────────────────────────────────────────

    RAISE NOTICE '%s', repeat('-', 65);

    n_idx := 200;
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
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'ts_rank top-10 (GIN + rank)', n_idx / e, e * 1000 / n_idx;

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
    RAISE NOTICE '%-42s %10.0f %10.3f',
        'ts_rank setweight A top-10', n_idx / e, e * 1000 / n_idx;

    RAISE NOTICE '';
    RAISE NOTICE '=== done ===';
END $$;

DROP TABLE IF EXISTS kham_bench_docs;

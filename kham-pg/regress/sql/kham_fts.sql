-- kham_fts — pg_regress integration tests for the kham-pg extension.
--
-- Run via:  make -C kham-pg regress   (uses Docker PostgreSQL 17)

-- ── 1. Load extension ────────────────────────────────────────────────────────

CREATE EXTENSION kham_pg;

-- ── 2. Verify lextypes ───────────────────────────────────────────────────────

SELECT tokid, alias, description
FROM ts_token_type('kham')
ORDER BY tokid;

-- ── 3. ts_parse — pure Thai ──────────────────────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'กินข้าว')
ORDER BY tokid, token;

-- ── 4. ts_parse — Thai sentence with stopword ────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'กินข้าวกับปลา')
ORDER BY tokid, token;

-- ── 5. ts_parse — mixed Thai + Latin + number ────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'Python 3 สำหรับ AI')
ORDER BY tokid, token;

-- ── 6. ts_parse — empty string returns no rows ───────────────────────────────

SELECT count(*) FROM ts_parse('kham', '');

-- ── 7. to_tsvector — non-null result ────────────────────────────────────────

SELECT to_tsvector('kham', 'กินข้าวกับปลา') IS NOT NULL AS ok;

-- ── 8. Phrase search — token present in document ────────────────────────────

SELECT to_tsvector('kham', 'กินข้าวกับปลา') @@ plainto_tsquery('kham', 'ปลา') AS found;

-- ── 9. Phrase search — token NOT in document ────────────────────────────────

SELECT to_tsvector('kham', 'กินข้าวกับปลา') @@ plainto_tsquery('kham', 'หมา') AS found;

-- ── 10. to_tsvector — mixed script ───────────────────────────────────────────

SELECT to_tsvector('kham', 'โปรแกรม Python') IS NOT NULL AS ok;

-- ── 11. tsvector lexeme content — pure Thai (exact positions) ────────────────

SELECT to_tsvector('kham', 'กินข้าว')::text;

-- ── 12. tsvector lexeme content — sentence with stopword ────────────────────

SELECT to_tsvector('kham', 'กินข้าวกับปลา')::text;

-- ── 13. to_tsquery — single word match ───────────────────────────────────────

SELECT to_tsvector('kham', 'กินข้าวกับปลา') @@ to_tsquery('kham', 'กิน') AS found;

-- ── 14. to_tsquery — no match ────────────────────────────────────────────────

SELECT to_tsvector('kham', 'กินข้าวกับปลา') @@ to_tsquery('kham', 'หมู') AS found;

-- ── 15. to_tsquery — Latin token match (simple dict lowercases) ──────────────

SELECT to_tsvector('kham', 'Python สำหรับ AI') @@ to_tsquery('kham', 'python') AS found;

-- ── 16. Table FTS with GIN index ─────────────────────────────────────────────

CREATE TABLE kham_docs (
    id   integer,
    body text
);

INSERT INTO kham_docs VALUES
    (1, 'กินข้าวที่บ้าน'),
    (2, 'ดื่มน้ำสะอาด'),
    (3, 'Python สำหรับนักพัฒนา');

CREATE INDEX kham_docs_fts_idx ON kham_docs
    USING GIN (to_tsvector('kham', body));

-- search Thai token — must return only row 1
SELECT id
FROM kham_docs
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'กิน')
ORDER BY id;

-- ── 17. GIN index — Latin token search ───────────────────────────────────────

-- search Latin token — must return only row 3
SELECT id
FROM kham_docs
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'Python')
ORDER BY id;

-- ── 18. GIN index — no match ─────────────────────────────────────────────────

SELECT id
FROM kham_docs
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'หมู')
ORDER BY id;

-- ── 19. ts_rank — non-zero for matching document ─────────────────────────────

SELECT ts_rank(
    to_tsvector('kham', 'กินข้าวกับปลา'),
    plainto_tsquery('kham', 'ปลา')
) > 0 AS ranked;

-- ── 20. Named entity — ts_parse returns tokid=7 ──────────────────────────────
-- จีน is in the NE gazetteer as PLACE; single syllable → always one token

SELECT tokid, token
FROM ts_parse('kham', 'จีน')
ORDER BY tokid, token;

-- ── 21. Named entity — to_tsvector indexes the token ────────────────────────

SELECT to_tsvector('kham', 'ไปจีน') @@ to_tsquery('kham', 'จีน') AS found;

-- ── 22. lextypes — parser exposes 7 token types ──────────────────────────────

SELECT count(*) AS lextype_count
FROM ts_token_type('kham');

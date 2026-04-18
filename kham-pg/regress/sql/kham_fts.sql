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

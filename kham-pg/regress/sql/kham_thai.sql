-- kham_thai — Thai language edge cases for the kham-pg extension.
--
-- Run via:  make -C kham-pg regress   (uses Docker PostgreSQL 17)
--
-- Covers: single char, Thai numerals, OOV/Unknown tokens, compound words,
-- stopword indexing, mixed scripts, punctuation, whitespace filtering.

CREATE EXTENSION IF NOT EXISTS kham_pg;

-- ── 1. Single Thai character → tokid=6 (Unknown, below TCC threshold) ────────

SELECT tokid, token
FROM ts_parse('kham', 'ก')
ORDER BY tokid, token;

-- ── 2. Thai numeral string → tokid=3 (Number) ────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', '๑๒๓')
ORDER BY tokid, token;

-- ── 3. Thai numeral mixed with Thai words ─────────────────────────────────────
-- ราคา segments as รา+คา (dict behaviour); ๑๕๐ is Number; บาท is Thai

SELECT tokid, token
FROM ts_parse('kham', 'ราคา๑๕๐บาท')
ORDER BY tokid, token;

-- ── 4. OOV brand name → Unknown tokens ───────────────────────────────────────
-- เปปซี่ is not in the built-in dictionary → เป+ป are Unknown; ซี่ is Thai

SELECT tokid, token
FROM ts_parse('kham', 'เปปซี่')
ORDER BY tokid, token;

-- ── 5. OOV tokens appear in tsvector (Unknown type is mapped in kham config) ──

SELECT to_tsvector('kham', 'เปปซี่') @@ plainto_tsquery('kham', 'ซี่') AS found;

-- ── 6. Punctuation: % sign → tokid=4 ─────────────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'ลด10%')
ORDER BY tokid, token;

-- ── 7. 3-word Thai sentence — correct segmentation ───────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'แมวกินปลา')
ORDER BY tokid, token;

-- ── 8. Compound word: โรงพยาบาล → โรง + พยาบาล ───────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'โรงพยาบาล')
ORDER BY tokid, token;

-- ── 9. Common greeting: สวัสดีครับ → สวัสดี + ครับ ──────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'สวัสดีครับ')
ORDER BY tokid, token;

-- ── 10. Developer compound: นักพัฒนา → นัก + พัฒนา ──────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'นักพัฒนา')
ORDER BY tokid, token;

-- ── 11. Whitespace is filtered — never appears in ts_parse output ─────────────

SELECT count(*)
FROM ts_parse('kham', 'กิน ปลา')
WHERE tokid = 0;

-- ── 12. Stopword กับ: appears in ts_parse (parser does not filter stopwords) ──

SELECT tokid, token
FROM ts_parse('kham', 'กับ')
ORDER BY tokid, token;

-- ── 13. Stopword ที่: appears in ts_parse ────────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'ที่')
ORDER BY tokid, token;

-- ── 14. Stopword ของ: appears in ts_parse ────────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'ของ')
ORDER BY tokid, token;

-- ── 15. Stopwords appear in tsvector (kham_dict=simple, no PG-level filtering)

SELECT to_tsvector('kham', 'กินข้าวกับปลา') @@ plainto_tsquery('kham', 'กับ') AS stopword_indexed;

-- ── 16. Thai + Arabic number: กินข้าว 3 มื้อ ────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'กินข้าว 3 มื้อ')
ORDER BY tokid, token;

-- ── 17. Thai + Latin: Python สำหรับ AI ───────────────────────────────────────

SELECT tokid, token
FROM ts_parse('kham', 'Python สำหรับ AI')
ORDER BY tokid, token;

-- ── 18. Mixed Thai + number + percent ────────────────────────────────────────

SELECT count(*) FILTER (WHERE tokid = 1) AS thai_count,
       count(*) FILTER (WHERE tokid = 3) AS number_count,
       count(*) FILTER (WHERE tokid = 4) AS punct_count
FROM ts_parse('kham', 'ราคา 500 บาท ลด 10%');

-- ── 19. FTS search on compound word ──────────────────────────────────────────
-- โรงพยาบาล segments as โรง+พยาบาล; search for พยาบาล must match

SELECT to_tsvector('kham', 'โรงพยาบาลใกล้บ้าน') @@ plainto_tsquery('kham', 'พยาบาล') AS found;

-- ── 20. Normalisation: to_tsvector is deterministic for same input ────────────

SELECT to_tsvector('kham', 'กินข้าว')::text = to_tsvector('kham', 'กินข้าว')::text AS stable;

-- ── 21. Named entity: จีน → tokid=7, ไป → tokid=1 ──────────────────────────
-- จีน is single-syllable → not split by segmenter

SELECT tokid, token
FROM ts_parse('kham', 'ไปจีน')
ORDER BY tokid, token;

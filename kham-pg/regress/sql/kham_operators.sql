-- kham_operators — FTS operator coverage for the kham-pg extension.
--
-- Run via:  make -C kham-pg regress   (uses Docker PostgreSQL 17)
--
-- Covers: AND, OR, NOT, phrase (phraseto_tsquery), websearch_to_tsquery,
-- ts_debug configuration verification.
--
-- Reference document: 'กินข้าวกับปลา' tokenises as:
--   กิน:1  ข้าว:2  กับ:3  ปลา:4  (all tokid=1, Thai)

CREATE EXTENSION IF NOT EXISTS kham_pg;

-- ── AND operator ──────────────────────────────────────────────────────────────

-- 1. AND — both tokens present → true
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ to_tsquery('kham', 'กิน & ปลา') AS and_both_present;

-- 2. AND — one token missing → false
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ to_tsquery('kham', 'กิน & หมู') AS and_one_missing;

-- 3. AND — neither token present → false
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ to_tsquery('kham', 'หมู & วัว') AS and_neither_present;

-- ── OR operator ───────────────────────────────────────────────────────────────

-- 4. OR — one token present → true
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ to_tsquery('kham', 'กิน | หมู') AS or_one_present;

-- 5. OR — both tokens present → true
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ to_tsquery('kham', 'กิน | ปลา') AS or_both_present;

-- 6. OR — neither token present → false
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ to_tsquery('kham', 'หมู | วัว') AS or_neither_present;

-- ── NOT operator ──────────────────────────────────────────────────────────────

-- 7. NOT — required token present, excluded token absent → true
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ to_tsquery('kham', 'กิน & !หมู') AS not_excluded_absent;

-- 8. NOT — required token present, excluded token also present → false
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ to_tsquery('kham', 'กิน & !ปลา') AS not_excluded_present;

-- ── Phrase operator (phraseto_tsquery) ───────────────────────────────────────
-- กินข้าวกับปลา: กิน=pos1, ข้าว=pos2, กับ=pos3, ปลา=pos4

-- 9. Phrase — adjacent tokens (pos 1 & 2) → true
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ phraseto_tsquery('kham', 'กิน ข้าว') AS phrase_adjacent;

-- 10. Phrase — non-adjacent tokens (pos 1 & 4, gap of 3) → false
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ phraseto_tsquery('kham', 'กิน ปลา') AS phrase_non_adjacent;

-- 11. Phrase — second pair adjacent (pos 3 & 4) → true
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ phraseto_tsquery('kham', 'กับ ปลา') AS phrase_second_pair;

-- ── websearch_to_tsquery ──────────────────────────────────────────────────────

-- 12. websearch — space-separated terms → AND match
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ websearch_to_tsquery('kham', 'กิน ปลา') AS websearch_and_match;

-- 13. websearch — term present but excluded with minus → false
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ websearch_to_tsquery('kham', 'กิน -ปลา') AS websearch_exclusion;

-- 14. websearch — excluded term absent → true
SELECT to_tsvector('kham', 'กินข้าวกับปลา')
    @@ websearch_to_tsquery('kham', 'กิน -หมู') AS websearch_exclusion_absent;

-- ── ts_debug — verify kham configuration ─────────────────────────────────────

-- 15. ts_debug: alias and lexemes for pure Thai input
SELECT alias, lexemes
FROM ts_debug('kham', 'กินข้าว')
WHERE alias IS NOT NULL
ORDER BY alias;

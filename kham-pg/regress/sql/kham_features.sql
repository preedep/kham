-- kham_features — regression tests for features added in 0.7.0.
--
-- Tests: stopword suppression, Thai number normalization,
--        udom83/MetaSound soundex dict variants, POS lexemes.
--
-- Assumes kham_pg is already loaded (kham_fts.sql runs first).
--
-- Run via:  make -C kham-pg regress   (uses Docker PostgreSQL 17)

-- ── 1. Stopword suppression — กับ must be absent from tsvector ────────────────
-- กับ is in the built-in Thai stopword list.  kham_fts_dict returns NULL
-- for it so PostgreSQL does not include it in the tsvector.

SELECT 'กับ' IN (SELECT lexeme FROM unnest(to_tsvector('kham', 'กินข้าวกับปลา')))
    AS stopword_present;

-- ── 2. Content word retained after stopword ───────────────────────────────────

SELECT 'ปลา' IN (SELECT lexeme FROM unnest(to_tsvector('kham', 'กินข้าวกับปลา')))
    AS content_present;

-- ── 3. Thai number normalization — Thai digit token ───────────────────────────
-- ๑๒๓ (Thai digit string, token type: number) goes through kham_fts_dict.
-- kham_fts_dict expands it to [๑๒๓, 123], so querying 123 finds ๑๒๓.

SELECT '123' IN (SELECT lexeme FROM unnest(to_tsvector('kham', '๑๒๓')))
    AS ascii_indexed;

-- ── 4. Thai number normalization — ASCII query matches Thai digit document ────

SELECT to_tsvector('kham', '๑๒๓') @@ plainto_tsquery('kham', '123')
    AS found_by_ascii;

-- ── 5. POS lexeme — verb-tagged Thai token emits pos_verb ─────────────────────
-- กิน (eat) is tagged VERB in the built-in POS table.

SELECT 'pos_verb' IN (SELECT lexeme FROM unnest(to_tsvector('kham', 'กิน')))
    AS pos_verb_indexed;

-- ── 6. POS lexeme — noun-tagged Thai token emits pos_noun ─────────────────────
-- ปลา (fish) is tagged NOUN in the built-in POS table.

SELECT 'pos_noun' IN (SELECT lexeme FROM unnest(to_tsvector('kham', 'ปลา')))
    AS pos_noun_indexed;

-- ── 7. udom83 dict — registered in pg_ts_dict ─────────────────────────────────

SELECT count(*) AS udom83_dict_exists
FROM pg_ts_dict
WHERE dictname = 'kham_fts_dict_udom83';

-- ── 8. MetaSound dict — registered in pg_ts_dict ─────────────────────────────

SELECT count(*) AS metasound_dict_exists
FROM pg_ts_dict
WHERE dictname = 'kham_fts_dict_metasound';

-- ── 9. udom83 dict — works inside a custom FTS configuration ─────────────────
-- Build a temporary config using udom83 and verify it produces a non-NULL tsvector.

CREATE TEXT SEARCH CONFIGURATION kham_udom83 (PARSER = kham);
ALTER TEXT SEARCH CONFIGURATION kham_udom83
    ADD MAPPING FOR thai, named WITH kham_fts_dict_udom83;
ALTER TEXT SEARCH CONFIGURATION kham_udom83
    ADD MAPPING FOR latin, number, unknown WITH kham_dict;

SELECT to_tsvector('kham_udom83', 'ปลา') IS NOT NULL AS udom83_works;

-- ── 10. udom83 soundex — numeric code present in tsvector ─────────────────────

SELECT EXISTS (
    SELECT 1 FROM unnest(to_tsvector('kham_udom83', 'ปลา'))
    WHERE lexeme ~ '^[0-9]'
) AS udom83_soundex_indexed;

-- ── 11. MetaSound dict — works inside a custom FTS configuration ──────────────

CREATE TEXT SEARCH CONFIGURATION kham_metasound (PARSER = kham);
ALTER TEXT SEARCH CONFIGURATION kham_metasound
    ADD MAPPING FOR thai, named WITH kham_fts_dict_metasound;
ALTER TEXT SEARCH CONFIGURATION kham_metasound
    ADD MAPPING FOR latin, number, unknown WITH kham_dict;

SELECT to_tsvector('kham_metasound', 'ปลา') IS NOT NULL AS metasound_works;

-- ── 12. MetaSound soundex — soundex code present in tsvector ───────────────────

SELECT EXISTS (
    SELECT 1 FROM unnest(to_tsvector('kham_metasound', 'ปลา'))
    WHERE lexeme ~ '^[0-9A-J]{3}'
) AS metasound_soundex_indexed;

-- ── 13. POS filtering — query for pos_verb matches document with a verb ────────
-- Use the tsquery literal cast so the underscore is treated as part of the lexeme.

SELECT to_tsvector('kham', 'กิน') @@ 'pos_verb'::tsquery AS found_by_pos;

-- ── 14. udom83 and lk82 both match ปลา ──────────────────────────────────────────
-- The soundex codes are different between algorithms but both are present.

SELECT
    EXISTS (SELECT 1 FROM unnest(to_tsvector('kham', 'ปลา'))        WHERE lexeme ~ '^[0-9]') AS lk82_code,
    EXISTS (SELECT 1 FROM unnest(to_tsvector('kham_udom83', 'ปลา')) WHERE lexeme ~ '^[0-9]') AS udom83_code;

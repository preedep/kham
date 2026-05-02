-- kham_pg--0.1.0--0.2.0.sql
--
-- Upgrade from 0.1.0 to 0.2.0.
--
-- What changed:
--   + kham_dict_lexize C function (phonetic + RTGS expansion)
--   + kham_fts_template TEXT SEARCH TEMPLATE
--   + kham_fts_dict     TEXT SEARCH DICTIONARY (Thai/Named token expansion)
--   ~ kham config: thai and named mappings switched to kham_fts_dict
--     (was: kham_dict simple pass-through)
--
-- The kham parser already has HEADLINE registered (added in 0.1.0).
-- No destructive changes; existing GIN indexes remain valid.

-- ── New C function ────────────────────────────────────────────────────────────

CREATE FUNCTION kham_dict_lexize(internal, internal, internal, internal)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_dict_lexize'
    LANGUAGE c;  -- NOT STRICT: dict state (arg0) is NULL when no INIT is provided

-- ── New dictionary template + dictionary ──────────────────────────────────────

CREATE TEXT SEARCH TEMPLATE kham_fts_template (
    LEXIZE = kham_dict_lexize
);

CREATE TEXT SEARCH DICTIONARY kham_fts_dict (
    TEMPLATE = kham_fts_template
);

-- ── Remap thai and named tokens to phonetic-expanding dictionary ──────────────
-- Previously both used kham_dict (simple lowercase).  Now they go through
-- kham_fts_dict which emits [word, lk82_soundex, rtgs_romanization] at the
-- same tsvector position.
--
-- NOTE: existing tsvectors stored in columns are NOT automatically updated.
-- Run UPDATE … SET col = to_tsvector('kham', source_col) to rebuild them,
-- or REINDEX any GIN/GiST indexes built on to_tsvector('kham', …) expressions.

ALTER TEXT SEARCH CONFIGURATION kham
    ALTER MAPPING FOR thai  WITH kham_fts_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ALTER MAPPING FOR named WITH kham_fts_dict;

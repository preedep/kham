-- kham_pg--0.1.3--0.2.0.sql
--
-- Upgrade from 0.1.3 to 0.2.0.
--
-- What changed:
--   + kham_headline C function and HEADLINE callback on the parser
--   + kham_dict_lexize C function (phonetic + RTGS expansion)
--   + kham_fts_template TEXT SEARCH TEMPLATE
--   + kham_fts_dict     TEXT SEARCH DICTIONARY
--   ~ kham config: thai and named remapped to kham_fts_dict
--
-- PostgreSQL does not support ALTER TEXT SEARCH PARSER to add a HEADLINE
-- function after creation, so this upgrade drops the parser (and its
-- dependent configuration) and recreates everything.
--
-- ⚠ WARNING: any stored tsvector columns or GIN/GiST indexes that depend
-- on the kham configuration must be rebuilt after this upgrade:
--   UPDATE tbl SET fts_col = to_tsvector('kham', source_col);
--   REINDEX INDEX idx_name;

-- ── Drop existing objects (cascade drops kham config automatically) ───────────

DROP TEXT SEARCH CONFIGURATION IF EXISTS kham CASCADE;
DROP TEXT SEARCH PARSER         IF EXISTS kham CASCADE;
DROP TEXT SEARCH DICTIONARY     IF EXISTS kham_dict CASCADE;

-- ── Recreate C functions ──────────────────────────────────────────────────────

CREATE FUNCTION kham_start(internal, int4)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_start'
    LANGUAGE c STRICT;

CREATE FUNCTION kham_gettoken(internal, internal, internal)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_gettoken'
    LANGUAGE c STRICT;

CREATE FUNCTION kham_end(internal)
    RETURNS void
    AS 'MODULE_PATHNAME', 'kham_end'
    LANGUAGE c STRICT;

CREATE FUNCTION kham_lextypes(internal)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_lextypes'
    LANGUAGE c STRICT;

CREATE FUNCTION kham_headline(internal, internal, tsquery)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_headline'
    LANGUAGE c STRICT;

CREATE FUNCTION kham_dict_lexize(internal, internal, internal, internal)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_dict_lexize'
    LANGUAGE c;  -- NOT STRICT: dict state (arg0) is NULL when no INIT is provided

-- ── Recreate parser (now includes HEADLINE) ───────────────────────────────────

CREATE TEXT SEARCH PARSER kham (
    START    = kham_start,
    GETTOKEN = kham_gettoken,
    END      = kham_end,
    LEXTYPES = kham_lextypes,
    HEADLINE = kham_headline
);

-- ── Recreate dictionaries ──────────────────────────────────────────────────────

CREATE TEXT SEARCH TEMPLATE kham_fts_template (
    LEXIZE = kham_dict_lexize
);

CREATE TEXT SEARCH DICTIONARY kham_fts_dict (
    TEMPLATE = kham_fts_template
);

CREATE TEXT SEARCH DICTIONARY kham_dict (
    TEMPLATE = simple
);

-- ── Recreate configuration ────────────────────────────────────────────────────

CREATE TEXT SEARCH CONFIGURATION kham (
    PARSER = kham
);

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR thai    WITH kham_fts_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR named   WITH kham_fts_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR latin   WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR number  WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR unknown WITH kham_dict;

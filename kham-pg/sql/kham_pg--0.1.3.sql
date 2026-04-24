-- kham_pg--0.1.3.sql
--
-- Creates the kham text search parser, a pass-through dictionary, and a
-- ready-to-use text search configuration for Thai documents.
--
-- Token types produced by the parser:
--   1  thai     Thai word token
--   2  latin    Latin script token
--   3  number   Numeric token
--   4  punct    Punctuation
--   5  emoji    Emoji token
--   6  unknown  Unknown / OOV token
--   7  named    Named entity token (person, place, organisation)

-- Guard against accidental direct \i load
\echo Use "CREATE EXTENSION kham_pg" to load this file. \quit

-- ── C function registrations ─────────────────────────────────────────────────
-- PostgreSQL resolves these to symbols in MODULE_PATHNAME (= $libdir/kham_pg).
-- Signatures match what ts_parse.c passes:
--   startfunc  (internal, int4)               → internal
--   gettoken   (internal, internal, internal) → int4
--   endfunc    (internal)                     → void
--   lextypes   (internal)                     → internal

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

-- ── Parser ───────────────────────────────────────────────────────────────────

CREATE TEXT SEARCH PARSER kham (
    START    = kham_start,
    GETTOKEN = kham_gettoken,
    END      = kham_end,
    LEXTYPES = kham_lextypes
);

-- ── Dictionary ───────────────────────────────────────────────────────────────
-- Simple pass-through: lowercases Latin/Number tokens; Thai is returned as-is
-- (Thai script is not case-folded by the simple template).

CREATE TEXT SEARCH DICTIONARY kham_dict (
    TEMPLATE = simple
);

-- ── Configuration ────────────────────────────────────────────────────────────

CREATE TEXT SEARCH CONFIGURATION kham (
    PARSER = kham
);

-- Map substantive token types through kham_dict.
-- Punctuation and emoji are omitted intentionally — no MAPPING means PG
-- discards those token types during indexing.

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR thai    WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR latin   WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR number  WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR unknown WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR named   WITH kham_dict;

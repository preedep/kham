-- kham_pg--0.1.0.sql
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

-- Guard against accidental direct load
\echo Use "CREATE EXTENSION kham_pg" to load this file. \quit

-- ── Parser ──────────────────────────────────────────────────────────────────

CREATE TEXT SEARCH PARSER kham (
    START    = kham_start,
    GETTOKEN = kham_gettoken,
    END      = kham_end,
    LEXTYPES = kham_lextypes
);

-- ── Dictionary ──────────────────────────────────────────────────────────────
-- Simple pass-through: lowercases Latin/Number tokens; Thai tokens are
-- returned unchanged (Thai script is not case-folded by the simple template).

CREATE TEXT SEARCH DICTIONARY kham_dict (
    TEMPLATE = simple
);

-- ── Configuration ───────────────────────────────────────────────────────────

CREATE TEXT SEARCH CONFIGURATION kham (
    PARSER = kham
);

-- Map all parser token types through kham_dict.
-- Punctuation and emoji are intentionally omitted so they are discarded
-- during indexing (no MAPPING means PG drops those token types).

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR thai    WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR latin   WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR number  WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR unknown WITH kham_dict;

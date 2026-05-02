-- kham_pg--0.8.0.sql
--
-- Creates the kham text search parser, dictionaries, and a ready-to-use
-- text search configuration for Thai documents.
--
-- New in 0.8.0 (kham-core):
--   • Token::confidence — segmentation confidence score on every token
--   • SpellChecker::did_you_mean / correct_text — single-word and passage correction
--   • RomanizationMap::romanize_sentence — segment + RTGS-romanize a passage
--   • KeyExtractor::extract_phrases — bigram/trigram keyphrase extraction
--   • TokenStream — streaming iterator with confidence-threshold filtering
--
-- (No schema changes to kham_pg itself in 0.8.0.)
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
--   startfunc  (internal, int4)                        → internal
--   gettoken   (internal, internal, internal)          → int4
--   endfunc    (internal)                              → void
--   lextypes   (internal)                              → internal
--   headline   (internal, internal, tsquery)           → internal
--   dict_lexize (internal, internal, internal, internal) → internal

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

CREATE FUNCTION kham_dict_lexize_udom83(internal, internal, internal, internal)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_dict_lexize_udom83'
    LANGUAGE c;

CREATE FUNCTION kham_dict_lexize_metasound(internal, internal, internal, internal)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_dict_lexize_metasound'
    LANGUAGE c;

-- ── Parser ───────────────────────────────────────────────────────────────────

CREATE TEXT SEARCH PARSER kham (
    START    = kham_start,
    GETTOKEN = kham_gettoken,
    END      = kham_end,
    LEXTYPES = kham_lextypes,
    HEADLINE = kham_headline
);

-- ── Dictionary templates ─────────────────────────────────────────────────────
-- Each template wires one of the three soundex algorithms.
-- All three implement:
--   • Stopword suppression (returns NULL for stopwords)
--   • Thai number normalization (Thai digits ↔ ASCII, number words ↔ decimal)
--   • POS colocated lexeme ("pos_<tag>", e.g. "pos_verb")
--   • RTGS romanization (Latin-script search)
--
-- kham_fts_template    → lk82 soundex  (recommended default)
-- kham_fts_template_udom83   → udom83 soundex (finer sibilant/liquid distinctions)
-- kham_fts_template_metasound → MetaSound     (per-syllable encoding, variable length)

CREATE TEXT SEARCH TEMPLATE kham_fts_template (
    LEXIZE = kham_dict_lexize
);

CREATE TEXT SEARCH TEMPLATE kham_fts_template_udom83 (
    LEXIZE = kham_dict_lexize_udom83
);

CREATE TEXT SEARCH TEMPLATE kham_fts_template_metasound (
    LEXIZE = kham_dict_lexize_metasound
);

-- ── Dictionaries ─────────────────────────────────────────────────────────────

-- kham_fts_dict: lk82 soundex (default) — Thai/Named tokens expand to
--   [word, lk82_soundex, rtgs?, ascii_number?, pos_<tag>?]
CREATE TEXT SEARCH DICTIONARY kham_fts_dict (
    TEMPLATE = kham_fts_template
);

-- kham_fts_dict_udom83: udom83 soundex variant
--   Finer distinctions between sibilants (ส/ช/ซ) and liquids (ล/ร).
CREATE TEXT SEARCH DICTIONARY kham_fts_dict_udom83 (
    TEMPLATE = kham_fts_template_udom83
);

-- kham_fts_dict_metasound: MetaSound soundex variant
--   Per-syllable [initial][vowel][final] encoding; variable-length codes.
CREATE TEXT SEARCH DICTIONARY kham_fts_dict_metasound (
    TEMPLATE = kham_fts_template_metasound
);

-- ── Simple pass-through dictionary ───────────────────────────────────────────
-- Lowercases Latin tokens; Thai is returned as-is (no case folding needed).

CREATE TEXT SEARCH DICTIONARY kham_dict (
    TEMPLATE = simple
);

-- ── Configuration ────────────────────────────────────────────────────────────

CREATE TEXT SEARCH CONFIGURATION kham (
    PARSER = kham
);

-- Thai and Named tokens: phonetic expansion via kham_fts_dict.
--   Each token becomes [word, lk82_soundex, rtgs?, ascii_num?, pos_<tag>?]
ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR thai    WITH kham_fts_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR named   WITH kham_fts_dict;

-- Number tokens: also routed through kham_fts_dict so that Thai digit strings
-- (e.g. ๑๒๓) are indexed alongside their ASCII equivalents (e.g. 123).
ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR number  WITH kham_fts_dict;

-- Latin and Unknown: simple lowercase pass-through.
ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR latin   WITH kham_dict;

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR unknown WITH kham_dict;

-- Punctuation and emoji have no mapping — PG discards those token types at
-- index time, which is the intended behaviour.

-- ── Convenience SQL helpers ──────────────────────────────────────────────────

-- kham_tsvector(document) — build a tsvector using the kham configuration.
-- Equivalent to: to_tsvector('kham', document)
CREATE FUNCTION kham_tsvector(document text)
    RETURNS tsvector
    LANGUAGE sql STRICT STABLE
    AS $$ SELECT to_tsvector('kham', document) $$;

-- kham_tsquery(query) — build a tsquery using the kham configuration.
-- Equivalent to: plainto_tsquery('kham', query)
CREATE FUNCTION kham_tsquery(query text)
    RETURNS tsquery
    LANGUAGE sql STRICT STABLE
    AS $$ SELECT plainto_tsquery('kham', query) $$;

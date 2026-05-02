-- kham_pg--0.6.0--0.7.0.sql
--
-- Upgrade script from 0.6.0 → 0.7.0.
--
-- Changes in 0.7.0:
--   • Stopword suppression — kham_fts_dict now returns NULL for Thai stopwords
--   • Thai number normalization — number tokens routed through kham_fts_dict
--     so Thai digit strings (๑๒๓) are colocated with ASCII equivalents (123)
--   • kham_fts_dict_udom83 — new dict using udom83 phonetic soundex
--   • kham_fts_dict_metasound — new dict using MetaSound soundex
--   • POS lexemes — "pos_<tag>" colocated lexeme (e.g. pos_verb, pos_noun)
--
-- NOTE: kham_dict_lexize behavior changed — tokens that are Thai stopwords
-- now return NULL (not indexed).  Rebuild any GIN/GiST indexes after upgrade:
--   REINDEX INDEX <your_fts_index>;

-- Guard against accidental direct \i load
\echo Use "ALTER EXTENSION kham_pg UPDATE" to load this file. \quit

-- ── New C functions ───────────────────────────────────────────────────────────

CREATE FUNCTION kham_dict_lexize_udom83(internal, internal, internal, internal)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_dict_lexize_udom83'
    LANGUAGE c;

CREATE FUNCTION kham_dict_lexize_metasound(internal, internal, internal, internal)
    RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_dict_lexize_metasound'
    LANGUAGE c;

-- ── New dictionary templates ─────────────────────────────────────────────────

CREATE TEXT SEARCH TEMPLATE kham_fts_template_udom83 (
    LEXIZE = kham_dict_lexize_udom83
);

CREATE TEXT SEARCH TEMPLATE kham_fts_template_metasound (
    LEXIZE = kham_dict_lexize_metasound
);

-- ── New dictionaries ──────────────────────────────────────────────────────────

CREATE TEXT SEARCH DICTIONARY kham_fts_dict_udom83 (
    TEMPLATE = kham_fts_template_udom83
);

CREATE TEXT SEARCH DICTIONARY kham_fts_dict_metasound (
    TEMPLATE = kham_fts_template_metasound
);

-- ── Thai number normalization — route number tokens through kham_fts_dict ────
-- In 0.6.0 number tokens went through kham_dict (simple lowercase).
-- In 0.7.0 they go through kham_fts_dict so Thai digit strings are colocated
-- with their ASCII equivalents (e.g. ๑๒๓ → ['๑๒๓', '123']).

ALTER TEXT SEARCH CONFIGURATION kham
    ALTER MAPPING FOR number WITH kham_fts_dict;

-- ── Convenience SQL helpers ──────────────────────────────────────────────────

CREATE FUNCTION kham_tsvector(document text)
    RETURNS tsvector
    LANGUAGE sql STRICT STABLE
    AS $$ SELECT to_tsvector('kham', document) $$;

CREATE FUNCTION kham_tsquery(query text)
    RETURNS tsquery
    LANGUAGE sql STRICT STABLE
    AS $$ SELECT plainto_tsquery('kham', query) $$;

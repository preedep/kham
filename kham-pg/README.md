# kham-pg

PostgreSQL text-search extension for Thai. Registers a custom text-search parser (`kham`) and dictionary (`kham_fts_dict`) so Thai documents can be indexed and queried with `tsvector` / `tsquery`.

## Install

```bash
# Build + run pg_regress in Docker — no local PG needed
make -C kham-pg regress                  # default PG 17
make -C kham-pg regress PG_VERSION=16   # single-version override
make -C kham-pg regress-matrix          # PG 14–18 in sequence

# Install into a local PostgreSQL (requires pg_config in PATH)
make -C kham-pg install
psql -c "CREATE EXTENSION kham_pg;"
```

## Token types

```sql
SELECT * FROM ts_token_type('kham');
-- 1  thai    Thai word
-- 2  latin   Latin script token
-- 3  number  Numeric token
-- 4  punct   Punctuation
-- 5  emoji   Emoji token
-- 6  unknown Unknown / OOV token
-- 7  named   Named entity token (person, place, organisation)
```

## Basic usage

```sql
-- Inspect how the parser splits text
SELECT * FROM ts_parse('kham', 'ทักษิณเดินทางไปกรุงเทพ');
-- 7  ทักษิณ     ← Named: Person
-- 1  เดิน
-- 1  ทาง
-- 1  ไป
-- 7  กรุงเทพ    ← Named: Place (merged from กรุง+เทพ by multi-token NE)

-- Build a tsvector — Thai/Named tokens expand to [word, lk82_soundex, rtgs?]
-- at the same position; Latin/Number use simple lowercase (kham_dict).
SELECT to_tsvector('kham', 'กินข้าวกับปลา');
-- '1400':2 '1619':1 '4800':3 'kap':2 'pla':3 'กับ':2 'กินข้าว':1 'ปลา':3

-- Full-text search
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'ปลา');
```

## Phonetic (soundex) search

Thai/Named tokens are automatically expanded with their lk82 soundex code. This means near-homophones share a code and match each other without any extra schema work.

```sql
-- Find documents containing any word with the same lk82 code as ปลา (4800)
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ to_tsquery('kham', '4800');
```

## RTGS romanization search

Thai/Named tokens are also expanded with their RTGS romanized form (built-in map). Latin-script queries match Thai documents.

```sql
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'pla');
-- matches documents containing ปลา
```

## ts_headline

```sql
SELECT ts_headline('kham', body, plainto_tsquery('kham', 'ปลา'))
FROM articles;
-- …กิน<b>ปลา</b>กับข้าว…

-- Custom markers
SELECT ts_headline(
    'kham', body,
    plainto_tsquery('kham', 'ปลา'),
    'StartSel=<<<, StopSel=>>>'
) FROM articles;
```

## GIN index

```sql
CREATE INDEX articles_fts_idx ON articles
    USING GIN (to_tsvector('kham', body));
```

## Dictionary — kham_fts_dict

`kham_fts_dict` is a custom dictionary template that expands each Thai or Named token to up to 6 colocated lexemes at the **same tsvector position**:

1. The normalised word itself
2. Its lk82 Thai Soundex code (phonetic-fuzzy search)
3. Its RTGS romanization (if in the built-in map; Latin-script search)
4. Its ASCII form for Thai digit strings (number normalization)
5. A POS lexeme `pos_<tag>` if the word has a known part of speech

Thai stopwords are **suppressed**: the dictionary returns NULL for them so PostgreSQL omits those tokens from the tsvector entirely.

Number tokens also go through `kham_fts_dict` so Thai digit strings (๑๒๓) are stored alongside their ASCII equivalent (123).

Latin and Unknown tokens use `kham_dict` (simple lowercase pass-through).

## Stopword suppression

Common Thai particles (กับ, ใน, ของ, ที่, …) return NULL from `kham_fts_dict`, which tells PostgreSQL to drop them from the tsvector. This reduces index size and query noise without any schema configuration.

## Thai number normalization

Thai digit strings and the `number` token type are expanded to include their ASCII equivalent as a colocated lexeme:

```sql
SELECT to_tsvector('kham', '๑๒๓') @@ plainto_tsquery('kham', '123') AS found;
-- t
```

## POS lexeme filtering

Tokens with a known part of speech emit an extra `pos_<tag>` lexeme. Query using the `::tsquery` cast so the underscore is not split:

```sql
-- Filter by part of speech
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ 'pos_verb'::tsquery;
```

Available tags include `pos_noun`, `pos_verb`, `pos_adj`, `pos_adv`, and others from the 13-category POS scheme.

## kham_tsvector / kham_tsquery helpers

Two SQL convenience functions wrap the built-in `kham` configuration so you don't need to repeat the configuration name:

```sql
-- Equivalent to to_tsvector('kham', body)
SELECT kham_tsvector('กินข้าวกับปลา');

-- Equivalent to plainto_tsquery('kham', query)
SELECT kham_tsquery('ปลา');

-- Full-text search with helpers
SELECT title FROM articles
WHERE kham_tsvector(body) @@ kham_tsquery('ปลา');

-- GIN index with helper
CREATE INDEX articles_fts_idx ON articles
    USING GIN (kham_tsvector(body));
```

Both functions are declared `STABLE` so PostgreSQL can fold them correctly in expression indexes and query plans.

## Alternative soundex dictionaries

Two additional dictionary variants use different soundex algorithms:

| Dictionary | Algorithm |
|---|---|
| `kham_fts_dict` | lk82 (default) |
| `kham_fts_dict_udom83` | udom83 |
| `kham_fts_dict_metasound` | MetaSound |

```sql
CREATE TEXT SEARCH CONFIGURATION kham_udom83 (PARSER = kham);
ALTER TEXT SEARCH CONFIGURATION kham_udom83
    ADD MAPPING FOR thai, named WITH kham_fts_dict_udom83;
ALTER TEXT SEARCH CONFIGURATION kham_udom83
    ADD MAPPING FOR latin, number, unknown WITH kham_dict;
```

## Build requirements

| Requirement | Notes |
|---|---|
| `pg_config` in `PATH` or `PG_CONFIG` env var | Points to your PostgreSQL installation |
| C compiler (`clang` or `gcc`) | Compiles `src/shim.c` via `cc` crate |
| Docker with BuildKit | For regress tests only |
| `brew install gettext` (macOS) | PG headers include `libintl.h` |

## Architecture

```
PostgreSQL fmgr  ──▶  src/shim.c (C)  ──▶  kham_*_impl() (Rust, src/lib.rs)
                       PG_MODULE_MAGIC
                       PG_FUNCTION_INFO_V1
```

See [CLAUDE.md](CLAUDE.md) for the full developer reference: callback signatures, C header include order, PG16+ lexize calling convention, regress test workflow, and Docker environment details.

# kham-pg

PostgreSQL text-search extension for Thai. Registers a custom text-search parser (`kham`) and dictionary (`kham_fts_dict`) so Thai documents can be indexed and queried with `tsvector` / `tsquery`.

## Install

```bash
# Build + run pg_regress in Docker (PostgreSQL 17) — no local PG needed
make -C kham-pg regress

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

`kham_fts_dict` is a custom dictionary template that expands each Thai or Named token to up to 6 lexemes at the **same tsvector position**:

1. The normalised word itself
2. Its lk82 Thai Soundex code (phonetic-fuzzy search)
3. Its RTGS romanization (if in the built-in map; Latin-script search)

Latin, Number, and Unknown tokens use `kham_dict` (simple lowercase pass-through).

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

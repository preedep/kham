# kham_pg

PostgreSQL text-search extension for the Thai language. Provides a custom
parser, phonetic dictionary, and ready-to-use FTS configuration so Thai
documents can be indexed and queried with `tsvector` / `tsquery`.

Thai has no spaces between words. Standard PostgreSQL parsers treat an entire
Thai sentence as one token. kham_pg uses the kham newmm segmentation engine to
split Thai text into correct word boundaries, then expands each Thai or Named
token into up to six lexemes at the same `tsvector` position:

1. The normalised word itself
2. Its lk82 Thai Soundex code — enables phonetic-fuzzy search
3. Its RTGS romanization — enables Latin-script search
4. Its ASCII form (for Thai digit strings and number words)
5. A POS lexeme `pos_<tag>` — enables part-of-speech filtering

Thai stopwords (common grammatical particles like กับ, ใน, ของ) are suppressed
and excluded from the tsvector automatically.

Named entities (persons, places, organisations) are tagged automatically.

## Install

**Try Option 0 first.** The Docker Hub image is the fastest way to get started — no
compiler, no Rust toolchain, no installation steps.
Fall back to Option 1 for bare-metal PostgreSQL installs, and Option 2 only if a
pre-built binary is not available for your platform.

---

### Option 0 — Docker Hub (no install)

A ready-to-use Docker image is available on Docker Hub. Pull and run.

```bash
# PostgreSQL 17 with kham_pg pre-installed (multi-arch: amd64 + arm64)
docker run --rm -e POSTGRES_PASSWORD=secret \
  -p 5432:5432 nickmsft/kham-pg:latest

# Specific PostgreSQL version (14–18)
docker run --rm -e POSTGRES_PASSWORD=secret \
  -p 5432:5432 nickmsft/kham-pg:0.8.1-pg17

# Connect and use immediately
psql -h localhost -U postgres -c "
  CREATE EXTENSION kham_pg;
  SELECT to_tsvector('kham', 'กินข้าวกับปลา');
"
```

**Image tags**

| Tag | PostgreSQL | Architectures |
|-----|-----------|---------------|
| `latest` | 17 | linux/amd64, linux/arm64 |
| `0.8.1-pg17` | 17 | linux/amd64, linux/arm64 |
| `0.7.0-pg18` | 18 | linux/amd64, linux/arm64 |
| `0.7.0-pg16` | 16 | linux/amd64, linux/arm64 |
| `0.7.0-pg15` | 15 | linux/amd64, linux/arm64 |
| `0.7.0-pg14` | 14 | linux/amd64, linux/arm64 |

Source: <https://hub.docker.com/r/nickmsft/kham-pg>

---

### Option 1 — Pre-built binary (recommended, no Rust required)

Pre-compiled `.so` files are available for **Linux x86_64** and **Linux aarch64**
(AWS Graviton, Ampere) for PostgreSQL 14–18 on the
[GitHub Releases page](https://github.com/preedep/kham/releases/latest).

**Prerequisites**

| Requirement | Notes |
|-------------|-------|
| PostgreSQL 14–18 | Server must be installed; `pg_config` must be in `PATH` |
| Linux x86\_64 or aarch64 | Pre-built binaries are Linux-only |

**Steps**

```bash
# 1. Unzip the PGXN distribution (provides control + SQL files)
unzip kham_pg-0.7.0.zip
cd kham_pg-0.7.0

# 2. Download the pre-built .so for your PG version and architecture
#    Replace PG=17 and ARCH=x86_64 as needed (14–18, x86_64 or aarch64)
PG=17
ARCH=x86_64
VERSION=0.7.0
curl -fsSL \
  "https://github.com/preedep/kham/releases/download/v${VERSION}/kham-pg-v${VERSION}-pg${PG}-${ARCH}-unknown-linux-gnu.tar.gz" \
  | tar xz   # extracts libkham_pg.so

# 3. Install the .so and extension files (sudo required for system PG)
PG_CONFIG=/usr/lib/postgresql/${PG}/bin/pg_config
sudo install -m 755 libkham_pg.so          "$($PG_CONFIG --pkglibdir)/kham_pg.so"
sudo install -m 644 kham_pg.control        "$($PG_CONFIG --sharedir)/extension/"
sudo install -m 644 sql/kham_pg--${VERSION}.sql "$($PG_CONFIG --sharedir)/extension/"

# 4. Load the extension in psql
psql -c "CREATE EXTENSION kham_pg;"
```

---

### Option 2 — Build from source (fallback)

Use this path if no pre-built binary is available for your platform or
PostgreSQL version. Supports any platform where PostgreSQL and Rust are
available (Linux, macOS).

**Prerequisites**

| Requirement | Notes |
|-------------|-------|
| PostgreSQL 14–18 | `pg_config` must be in `PATH` or set via `PG_CONFIG` env var |
| Rust 1.85+ | Install via [rustup.rs](https://rustup.rs) |
| C compiler | `clang` or `gcc` |
| Linux system packages | `build-essential postgresql-server-dev-N` (replace N with your PG major version) |
| `brew install gettext` | **macOS only** — PostgreSQL headers require `libintl.h` |

**Steps**

**Linux (Debian / Ubuntu)**

```bash
# Replace 17 with your PostgreSQL major version (14–18)
PG=17
sudo apt-get install -y build-essential postgresql-server-dev-${PG}

unzip kham_pg-0.7.0.zip
cd kham_pg-0.7.0
PG_CONFIG=/usr/lib/postgresql/${PG}/bin/pg_config make install
psql -c "CREATE EXTENSION kham_pg;"
```

**macOS (Homebrew)**

```bash
# Replace 17 with your PostgreSQL major version (14–18)
PG=17
brew install postgresql@${PG} gettext

unzip kham_pg-0.7.0.zip
cd kham_pg-0.7.0
PG_CONFIG=$(brew --prefix postgresql@${PG})/bin/pg_config make install
psql -c "CREATE EXTENSION kham_pg;"
```

---

## Token types

```sql
SELECT * FROM ts_token_type('kham');
--  1  thai    Thai word
--  2  latin   Latin script token
--  3  number  Numeric token
--  4  punct   Punctuation
--  5  emoji   Emoji token
--  6  unknown Unknown / OOV token
--  7  named   Named entity token (person, place, organisation)
```

## Basic usage

```sql
-- Inspect how the parser splits Thai text
SELECT * FROM ts_parse('kham', 'กินข้าวกับปลา');
--  1  กินข้าว
--  1  กับ        ← stopword — will be suppressed in tsvector
--  1  ปลา

-- Build a tsvector — กับ (stopword) is suppressed;
-- ปลา expands to [word, lk82_soundex, rtgs, pos_noun]
SELECT to_tsvector('kham', 'กินข้าวกับปลา');
-- '1619':1 '4800':2 'pla':2 'pos_noun':2 'กินข้าว':1 'ปลา':2

-- Full-text search
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'ปลา');
```

## Stopword suppression

Thai grammatical particles (กับ, ใน, ของ, ที่, และ, …) are in the built-in
stopword list. The dictionary returns NULL for these tokens so PostgreSQL
excludes them from the tsvector entirely.

```sql
-- กับ is a stopword — it is NOT in the tsvector
SELECT 'กับ' IN (
    SELECT lexeme FROM unnest(to_tsvector('kham', 'กินข้าวกับปลา'))
) AS stopword_present;
-- f

-- ปลา is a content word — it IS indexed
SELECT 'ปลา' IN (
    SELECT lexeme FROM unnest(to_tsvector('kham', 'กินข้าวกับปลา'))
) AS content_present;
-- t
```

## Phonetic search (lk82 Soundex)

Thai/Named tokens are automatically expanded with their lk82 Soundex code.
Near-homophones share a code and match each other without any extra schema work.

```sql
-- Match any word with the same lk82 code as ปลา (4800)
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ to_tsquery('kham', '4800');

-- Find the code stored for a given word
SELECT lexeme FROM unnest(to_tsvector('kham', 'ปลา'))
WHERE lexeme ~ '^[0-9]';
-- 4800
```

## RTGS romanization search

Thai/Named tokens are also expanded with their RTGS romanized form.
Latin-script queries match Thai documents automatically.

```sql
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'pla');
-- matches documents containing ปลา
```

## Thai number normalization

Thai digit strings (Thai Unicode ๐–๙) and the `number` token type are expanded
to include their ASCII equivalent as a colocated lexeme, enabling cross-script
numeric queries.

```sql
-- ๑๒๓ is indexed as both ๑๒๓ and 123
SELECT to_tsvector('kham', '๑๒๓') @@ plainto_tsquery('kham', '123') AS found;
-- t

-- Inspect the expansion
SELECT lexeme FROM unnest(to_tsvector('kham', '๑๒๓'));
-- ๑๒๓
-- 123
```

## POS lexeme filtering

Each Thai token whose part of speech is known emits an additional colocated
lexeme of the form `pos_<tag>` (e.g. `pos_verb`, `pos_noun`, `pos_adj`).
Use the `::tsquery` cast so the underscore is treated as part of the lexeme.

```sql
-- Find documents that contain a verb
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ 'pos_verb'::tsquery;

-- Combine POS filter with content filter
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@
      ('pos_noun'::tsquery && plainto_tsquery('kham', 'กิน'));

-- Inspect POS lexemes for a word
SELECT lexeme FROM unnest(to_tsvector('kham', 'ปลา'))
WHERE lexeme LIKE 'pos_%';
-- pos_noun
```

## Alternative soundex dictionaries

Two additional dictionary variants are available for applications that need
finer phonetic discrimination:

| Dictionary | Algorithm | Characteristics |
|---|---|---|
| `kham_fts_dict` | lk82 | Default; broadest match; consonant-class-based |
| `kham_fts_dict_udom83` | udom83 | Finer sibilant and liquid distinctions |
| `kham_fts_dict_metasound` | MetaSound | Per-syllable encoding; most discriminating |

Use a custom configuration to swap in an alternative dictionary:

```sql
-- Build a configuration backed by udom83 soundex
CREATE TEXT SEARCH CONFIGURATION kham_udom83 (PARSER = kham);
ALTER TEXT SEARCH CONFIGURATION kham_udom83
    ADD MAPPING FOR thai, named WITH kham_fts_dict_udom83;
ALTER TEXT SEARCH CONFIGURATION kham_udom83
    ADD MAPPING FOR latin, number, unknown WITH kham_dict;

-- Index and search with udom83
SELECT to_tsvector('kham_udom83', 'ปลา') @@ plainto_tsquery('kham_udom83', 'ปลา');
-- t
```

## Named entity search

```sql
SELECT * FROM ts_parse('kham', 'ทักษิณเดินทางไปกรุงเทพ');
--  7  ทักษิณ    ← Named: Person
--  1  เดิน
--  1  ทาง
--  1  ไป
--  7  กรุงเทพ   ← Named: Place
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

## kham_tsvector / kham_tsquery helpers

Two SQL convenience functions wrap the built-in `kham` configuration so you don't
need to repeat the configuration name at every call site:

```sql
-- Equivalent to to_tsvector('kham', text)
SELECT kham_tsvector('กินข้าวกับปลา');

-- Equivalent to plainto_tsquery('kham', text)
SELECT kham_tsquery('ปลา');

-- Full-text search with helpers
SELECT title FROM articles
WHERE kham_tsvector(body) @@ kham_tsquery('ปลา');
```

Both functions are declared `STABLE` so PostgreSQL can use them correctly in
expression indexes and query plans.

## GIN index

```sql
-- Use the helper in an expression index
CREATE INDEX articles_fts_idx ON articles
    USING GIN (kham_tsvector(body));

-- Or the explicit form — both are equivalent
CREATE INDEX articles_fts_idx ON articles
    USING GIN (to_tsvector('kham', body));

-- Query uses the index automatically
SELECT title FROM articles
WHERE kham_tsvector(body) @@ kham_tsquery('ปลา')
ORDER BY ts_rank(kham_tsvector(body), kham_tsquery('ปลา')) DESC;
```

## Upgrade

If you are upgrading from a previous version:

```sql
ALTER EXTENSION kham_pg UPDATE;
```

## License

MIT OR Apache-2.0

## Links

- Source: <https://github.com/preedep/kham>
- Releases: <https://github.com/preedep/kham/releases>
- Issues: <https://github.com/preedep/kham/issues>
- PGXN: <https://pgxn.org/dist/kham_pg/>

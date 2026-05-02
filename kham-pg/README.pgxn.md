# kham_pg

PostgreSQL text-search extension for the Thai language. Provides a custom
parser, phonetic dictionary, and ready-to-use FTS configuration so Thai
documents can be indexed and queried with `tsvector` / `tsquery`.

Thai has no spaces between words. Standard PostgreSQL parsers treat an entire
Thai sentence as one token. kham_pg uses the kham newmm segmentation engine to
split Thai text into correct word boundaries, then expands each token into up
to three lexemes at the same `tsvector` position:

1. The normalised word itself
2. Its lk82 Thai Soundex code — enables phonetic-fuzzy search
3. Its RTGS romanization — enables Latin-script search

Named entities (persons, places, organisations) are tagged automatically.

## Requirements

| Requirement | Notes |
|-------------|-------|
| PostgreSQL 14–18 | `pg_config` must be in `PATH` or set via `PG_CONFIG` env var |
| Rust 1.85+ | Install via [rustup.rs](https://rustup.rs) |
| C compiler | `clang` or `gcc` — standard on Linux and macOS |
| `brew install gettext` | **macOS only** — PostgreSQL headers require `libintl.h` |

## Install

```bash
# 1. Unzip the distribution
unzip kham_pg-0.6.0.zip
cd kham_pg-0.6.0

# 2. Build and install into your PostgreSQL installation
make install

# 3. Load the extension in psql
psql -c "CREATE EXTENSION kham_pg;"
```

To target a specific PostgreSQL installation:

```bash
PG_CONFIG=/usr/lib/postgresql/17/bin/pg_config make install
```

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
--  1  กับ
--  1  ปลา

-- Build a tsvector — Thai tokens expand to [word, soundex, rtgs]
SELECT to_tsvector('kham', 'กินข้าวกับปลา');
-- '1400':2 '1619':1 '4800':3 'kap':2 'pla':3 'กับ':2 'กินข้าว':1 'ปลา':3

-- Full-text search
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'ปลา');
```

## Phonetic search

Thai/Named tokens are automatically expanded with their lk82 Soundex code.
Near-homophones share a code and match each other without any extra schema work.

```sql
-- Match any word with the same lk82 code as ปลา (4800)
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ to_tsquery('kham', '4800');
```

## RTGS romanization search

Thai/Named tokens are also expanded with their RTGS romanized form.
Latin-script queries match Thai documents automatically.

```sql
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'pla');
-- matches documents containing ปลา
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

## GIN index

```sql
CREATE INDEX articles_fts_idx ON articles
    USING GIN (to_tsvector('kham', body));

-- Query uses the index automatically
SELECT title FROM articles
WHERE to_tsvector('kham', body) @@ plainto_tsquery('kham', 'ปลา')
ORDER BY ts_rank(to_tsvector('kham', body), plainto_tsquery('kham', 'ปลา')) DESC;
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
- Issues: <https://github.com/preedep/kham/issues>
- PGXN: <https://pgxn.org/dist/kham_pg/>

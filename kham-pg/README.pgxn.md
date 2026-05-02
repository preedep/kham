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

## Install

Choose the path that fits your environment.

---

### Option 1 — Pre-built binary (no Rust required)

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
unzip kham_pg-0.6.0.zip
cd kham_pg-0.6.0

# 2. Download the pre-built .so for your PG version and architecture
#    Replace PG=17 and ARCH=x86_64 as needed (14–18, x86_64 or aarch64)
PG=17
ARCH=x86_64
VERSION=0.6.0
curl -fsSL \
  "https://github.com/preedep/kham/releases/download/v${VERSION}/kham-pg-v${VERSION}-pg${PG}-${ARCH}-unknown-linux-gnu.tar.gz" \
  | tar xz   # extracts libkham_pg.so

# 3. Install the .so and extension files
PG_CONFIG=/usr/lib/postgresql/${PG}/bin/pg_config
install -m 755 libkham_pg.so          "$($PG_CONFIG --pkglibdir)/kham_pg.so"
install -m 644 kham_pg.control        "$($PG_CONFIG --sharedir)/extension/"
install -m 644 sql/kham_pg--${VERSION}.sql "$($PG_CONFIG --sharedir)/extension/"

# 4. Load the extension in psql
psql -c "CREATE EXTENSION kham_pg;"
```

---

### Option 2 — Build from source

Builds the extension from source using your local `pg_config` and Rust toolchain.
Supports any platform where PostgreSQL and Rust are available (Linux, macOS).

**Prerequisites**

| Requirement | Notes |
|-------------|-------|
| PostgreSQL 14–18 | `pg_config` must be in `PATH` or set via `PG_CONFIG` env var |
| Rust 1.85+ | Install via [rustup.rs](https://rustup.rs) |
| C compiler | `clang` or `gcc` |
| Linux system packages | `build-essential postgresql-server-dev-N` (replace N with your PG major version) |
| `brew install gettext` | **macOS only** — PostgreSQL headers require `libintl.h` |

**Steps**

```bash
# Linux — install system packages first
sudo apt-get install -y build-essential postgresql-server-dev-17

# 1. Unzip the distribution
unzip kham_pg-0.6.0.zip
cd kham_pg-0.6.0

# 2. Build and install
make install

# 3. Load the extension in psql
psql -c "CREATE EXTENSION kham_pg;"
```

To target a specific PostgreSQL installation:

```bash
PG_CONFIG=/usr/lib/postgresql/17/bin/pg_config make install
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
- Releases: <https://github.com/preedep/kham/releases>
- Issues: <https://github.com/preedep/kham/issues>
- PGXN: <https://pgxn.org/dist/kham_pg/>

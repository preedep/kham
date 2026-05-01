# kham-sqlite

SQLite FTS5 tokenizer extension for Thai. Registers a `kham` tokenizer as a loadable extension with the full NLP pipeline: normalization, segmentation, NE tagging, synonym expansion, RTGS romanization, and Thai phonetic soundex via `FTS5_TOKEN_COLOCATED`. `highlight()` and `snippet()` work via byte-accurate offsets.

## Build

```bash
cargo build -p kham-sqlite --release
# → target/release/libkham_sqlite.dylib  (macOS)
# → target/release/libkham_sqlite.so     (Linux)
```

## Basic usage

```sql
-- Load the extension
SELECT load_extension('./target/release/libkham_sqlite', 'sqlite3_kham_init');

-- Create an FTS5 virtual table
CREATE VIRTUAL TABLE articles USING fts5(title, body, tokenize='kham');

-- Insert Thai documents
INSERT INTO articles VALUES ('อาหารไทย', 'กินข้าวกับปลาและน้ำพริก');
INSERT INTO articles VALUES ('สภาพอากาศ', 'วันนี้อากาศดีมากท้องฟ้าแจ่มใส');

-- Full-text search
SELECT title FROM articles WHERE articles MATCH 'ปลา';
-- อาหารไทย

SELECT title FROM articles WHERE articles MATCH 'อากาศ';
-- สภาพอากาศ
```

## RTGS romanization search

Thai tokens are indexed with their RTGS romanized form as a colocated synonym — no extra configuration needed.

```sql
SELECT title FROM articles WHERE articles MATCH 'kin';
-- อาหารไทย  (กิน is indexed as both "กิน" and "kin")
```

## Phonetic soundex search

lk82 soundex is enabled by default. Near-homophones share a code and match each other.

```sql
SELECT title FROM articles WHERE articles MATCH '1619';
-- อาหารไทย  (lk82 code for กินข้าว)
```

## tokenize arguments

Arguments are passed as space-separated key/value pairs in the `tokenize=` option:

| Argument | Values | Default | Description |
|---|---|---|---|
| `soundex` | `lk82`, `udom83`, `metasound`, `none` | `lk82` | Phonetic soundex algorithm |
| `stopwords` | `on`, `off` | `off` | Suppress stopword tokens at index time |
| `ngram_size` | integer ≥ 0 | `3` | N-gram size for Unknown/OOV tokens; 0 disables n-grams |
| `synonyms` | `'<path>'` | — | TSV synonym map: `canonical TAB syn1 TAB syn2 …` |
| `dict` | `'<path>'` | — | Newline-separated word list overlaid on the built-in dictionary |

> **Path quoting:** `/`, `.`, and `-` are not FTS5 bareword characters. File paths must be single-quoted inside the tokenize directive and SQL-escaped with `''`:
> `tokenize='kham synonyms ''/path/to/file.tsv'''`

```sql
-- udom83 soundex (finer sibilant/liquid distinctions)
CREATE VIRTUAL TABLE t1 USING fts5(body, tokenize='kham soundex udom83');

-- Disable soundex entirely
CREATE VIRTUAL TABLE t2 USING fts5(body, tokenize='kham soundex none');

-- Suppress stopwords + bigrams for OOV
CREATE VIRTUAL TABLE t3 USING fts5(body, tokenize='kham stopwords on ngram_size 2');

-- Custom synonym map
CREATE VIRTUAL TABLE t4 USING fts5(body,
    tokenize='kham synonyms ''/etc/kham/synonyms.tsv''');

-- Custom domain dictionary (overlaid on built-in words)
CREATE VIRTUAL TABLE t5 USING fts5(body,
    tokenize='kham dict ''/etc/kham/domain_words.txt''');

-- All options combined
CREATE VIRTUAL TABLE t6 USING fts5(body,
    tokenize='kham soundex lk82 stopwords on ngram_size 2 dict ''/words.txt'' synonyms ''/syns.tsv''');
```

## Snippet highlighting

Byte-accurate offsets from the tokenizer feed directly into FTS5's built-in highlight/snippet functions.

```sql
SELECT snippet(articles, 1, '>>>', '<<<', '...', 6)
FROM articles WHERE articles MATCH 'ข้าว';
-- กิน>>>ข้าว<<<กับปลาและน้ำพริก
```

## macOS note

The system `sqlite3` binary disables `load_extension`. Use Homebrew:

```bash
brew install sqlite
/opt/homebrew/opt/sqlite/bin/sqlite3 ':memory:' \
    "SELECT load_extension('./target/release/libkham_sqlite', 'sqlite3_kham_init');"
```

## Build requirements

| Platform | Requirement | Install |
|---|---|---|
| macOS | Xcode CLT or Homebrew sqlite | `xcode-select --install` or `brew install sqlite` |
| Linux | SQLite development headers | `apt install libsqlite3-dev` |

Override header path: `SQLITE_INCLUDE_DIR=/path/to/sqlite cargo build -p kham-sqlite`.

## Architecture

```
SQLite FTS5  ──▶  src/shim.c (C)  ──▶  kham_register_tokenizer() (Rust, src/lib.rs)
                  SQLITE_EXTENSION_INIT1/2            │
                  get_fts5_api() — bind-pointer trick  ▼
                  sqlite3_kham_init()        xCreate / xDelete / xTokenize
```

See [CLAUDE.md](CLAUDE.md) for the full developer reference: `fts5_api` struct layout, xToken byte offset semantics, build system details, and common issues.

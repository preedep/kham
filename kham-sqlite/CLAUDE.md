# kham-sqlite

SQLite FTS5 tokenizer extension (`cdylib`) wrapping `kham-core`'s `FtsTokenizer`.

## Architecture

```
SQLite FTS5  ──▶  src/shim.c (C helpers)  ──▶  lib.rs (Rust entry points + callbacks)
                  SQLITE_EXTENSION_INIT1/2           │
                  kham_sqlite_setup_api()             ▼
                  kham_sqlite_get_fts5api()  sqlite3_kham_init / sqlite3_khamsqlite_init
                                                      │
                                                      ▼
                                             xCreate → KhamInstance (cached FtsTokenizer)
                                                      │
                                                      ▼
                                             xTokenize → normalize → segment_for_fts
                                                               → xToken (primary)
                                                               → xToken FTS5_TOKEN_COLOCATED
                                                                        (synonyms + RTGS + soundex)
```

- `shim.c` provides C helpers (`kham_sqlite_setup_api`, `kham_sqlite_get_fts5api`) called from Rust
- `lib.rs` defines `#[no_mangle]` entry points (guaranteed in dylib symbol table) and FTS5 callbacks
- `FtsTokenizer` is built **once per FTS5 table** in `xCreate` and cached in `KhamInstance`
- `xTokenize` normalizes the input, segments, then emits primary tokens + colocated synonyms

## Key Files

```
kham-sqlite/
├── Cargo.toml             # crate-type = ["cdylib"], kham-core dep
├── build.rs               # find SQLite headers (xcrun/brew/pkg-config); compile shim.c
└── src/
    ├── lib.rs             # Rust FTS5 callbacks + registration
    └── shim.c             # C: SQLITE_EXTENSION_INIT1/2, fts5_api_from_db
```

## FTS5 Tokenizer Callbacks

| Callback      | Signature                                               | Purpose |
|---------------|---------------------------------------------------------|---------|
| `xCreate`     | `(userdata, azArg, nArg, **ppOut) → int`                | Allocate per-table tokenizer instance |
| `xDelete`     | `(*tokenizer)`                                          | Free tokenizer instance |
| `xTokenize`   | `(*tok, pCtx, flags, pText, nText, xToken) → int`       | Segment document / query text |

**`xTokenize` pipeline:**
1. Build `&str` from `(pText, nText)` — handles both counted and NUL-terminated forms
2. `normalizer::normalize(text)` → `normalized: String` (สระลอย, วรรณยุกต์ dedup, Sara Am)
3. `fts.segment_for_fts(text)` → `Vec<FtsToken>` (whitespace excluded; NE merged; synonyms populated)
4. For each `FtsToken`, locate its byte span in `normalized` via forward scan
5. `xToken(pCtx, 0, pToken, nToken, iStart, iEnd)` — primary token
6. `xToken(pCtx, FTS5_TOKEN_COLOCATED, pSyn, nSyn, iStart, iEnd)` — each synonym/RTGS form
7. Return immediately if any `xToken` call returns non-`SQLITE_OK`

## Struct layout — C inheritance pattern

```rust
#[repr(C)]
struct KhamInstance {
    vtable: KhamFts5Tokenizer,  // MUST be first field — pointer-aliases *mut KhamFts5Tokenizer
    fts: FtsTokenizer,          // cached; invisible to SQLite
}
```

SQLite stores and passes back `*mut KhamFts5Tokenizer` from `xCreate`.  `xTokenize`/`xDelete`
cast it to `*mut KhamInstance` to recover the `FtsTokenizer`.

## Rust Type Definitions

```rust
// Matches sqlite3.h struct fts5_tokenizer (v1, no locale)
#[repr(C)]
pub struct KhamFts5Tokenizer {
    x_create:   Option<unsafe extern "C" fn(...)> -> c_int>,
    x_delete:   Option<unsafe extern "C" fn(*mut KhamFts5Tokenizer)>,
    x_tokenize: Option<unsafe extern "C" fn(...)> -> c_int>,
}

// Truncated view of fts5_api — only iVersion + xCreateTokenizer accessed
#[repr(C)]
struct KhamFts5Api {
    i_version:          c_int,
    x_create_tokenizer: Option<unsafe extern "C" fn(...)> -> c_int>,
}
```

## Normalization and byte offsets

`xToken(iStart, iEnd)` reports byte offsets into the **normalized** form of the input text.
`snippet()` and `highlight()` are accurate when documents are stored in normalized form.
For documents stored with stacked tone marks or unresolved Sara Am (rare in practice), offsets
may shift by a few bytes in those spans.

## Synonym expansion and soundex (FTS5_TOKEN_COLOCATED)

For each non-stop token, synonyms, RTGS romanization forms, and soundex phonetic codes are
emitted as colocated tokens at the same `(iStart, iEnd)` position.

```sql
SELECT * FROM docs WHERE docs MATCH 'kin';   -- matches กิน via RTGS
SELECT * FROM docs WHERE docs MATCH '4800';  -- matches ปลา via lk82 soundex code
SELECT * FROM docs WHERE docs MATCH '1600';  -- matches กาน/ขาน/คาน (near-homophones, lk82)
```

RTGS romanization is enabled by default (`RomanizationMap::builtin()`).
Soundex defaults to **lk82** and can be overridden via the `soundex <algo>` `xCreate` argument.
Stopword suppression is off by default and enabled with `stopwords on`.

```sql
CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham');                          -- default: lk82, stopwords forwarded
CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham soundex udom83');
CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham soundex metasound');
CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham soundex none');             -- disable soundex
CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham stopwords on');             -- suppress stopwords
CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham soundex lk82 stopwords on'); -- both
```

Custom synonym maps are not yet exposed via `xCreate` arguments.

## Build Requirements

- **macOS:** Xcode Command Line Tools (provides `sqlite3.h` / `sqlite3ext.h` via `xcrun --show-sdk-path`)
  - Override with `SQLITE_INCLUDE_DIR=/path/to/sqlite/include`
  - Falls back to `$(brew --prefix sqlite)/include` if xcrun fails
- **Linux:** `libsqlite3-dev` (Ubuntu/Debian) or `sqlite-devel` (Fedora/RHEL)
  - Auto-detected via `pkg-config sqlite3 --cflags-only-I`
  - Falls back to `/usr/include`
- **macOS linker:** `build.rs` emits `-undefined dynamic_lookup` so SQLite symbols resolve at `dlopen` time

## Entry Points

Two `#[no_mangle]` symbols are exported (both call the same `do_init` function):

| Symbol | Used by |
|--------|---------|
| `sqlite3_kham_init` | Explicit: `load_extension('libkham_sqlite', 'sqlite3_kham_init')` |
| `sqlite3_khamsqlite_init` | Implicit: `load_extension('libkham_sqlite')` — SQLite derives this name from the filename by stripping underscores |

## Usage

```sql
-- Load the extension — implicit entry point (SQLite ≥3.44 recommended)
SELECT load_extension('./target/release/libkham_sqlite');

-- Or explicit entry point (works on all SQLite versions)
SELECT load_extension('./target/release/libkham_sqlite', 'sqlite3_kham_init');

-- Create an FTS5 table using the kham tokenizer
CREATE VIRTUAL TABLE docs USING fts5(body, tokenize='kham');

-- Insert Thai documents
INSERT INTO docs VALUES ('กินข้าวกับปลา');
INSERT INTO docs VALUES ('ทักษิณเดินทางไปกรุงเทพ');

-- Full-text search
SELECT * FROM docs WHERE docs MATCH 'ปลา';
SELECT * FROM docs WHERE docs MATCH 'กรุงเทพ';   -- NE merged from กรุง+เทพ

-- RTGS romanization search (built-in, no config needed)
SELECT * FROM docs WHERE docs MATCH 'kin';       -- matches กิน
SELECT * FROM docs WHERE docs MATCH 'krungthep'; -- matches กรุงเทพ (if in RTGS map)

-- Soundex phonetic fuzzy search (lk82 by default)
SELECT * FROM docs WHERE docs MATCH '4800';      -- matches ปลา (lk82 code)
SELECT * FROM docs WHERE docs MATCH '1600';      -- matches กาน/ขาน/คาน (near-homophones)

-- Override soundex algorithm
CREATE VIRTUAL TABLE docs2 USING fts5(body, tokenize='kham soundex udom83');
CREATE VIRTUAL TABLE docs3 USING fts5(body, tokenize='kham soundex none'); -- disable

-- Phrase search
SELECT * FROM docs WHERE docs MATCH '"กิน ข้าว"';

-- Snippet highlighting (uses byte offsets from xTokenize into normalized text)
SELECT snippet(docs, 0, '<b>', '</b>', '...', 5) FROM docs WHERE docs MATCH 'ปลา';
```

## Token Types

All non-whitespace token kinds are forwarded to SQLite FTS5 without stopword filtering.

| `TokenKind`              | Forwarded? | Notes |
|--------------------------|-----------|-------|
| `Thai`                   | ✓ | + synonyms / RTGS / soundex as colocated |
| `Latin`                  | ✓ | |
| `Number`                 | ✓ | |
| `Punctuation`            | ✓ | |
| `Emoji`                  | ✓ | |
| `Unknown`                | ✓ | trigrams emitted as colocated |
| `Named(_)`               | ✓ | merged by NE tagger; + RTGS / soundex |
| `Whitespace`             | — | excluded by `segment_for_fts` |

## Build Commands

```bash
# Build the shared library
cargo build -p kham-sqlite --release

# Verify exported symbols
nm -D target/release/libkham_sqlite.dylib | grep kham   # macOS
nm -D target/release/libkham_sqlite.so   | grep kham   # Linux

# Run criterion benchmarks
cargo bench -p kham-sqlite

# Quick smoke test
sqlite3 ':memory:' \
  ".load ./target/release/libkham_sqlite" \
  "CREATE VIRTUAL TABLE t USING fts5(body, tokenize='kham');" \
  "INSERT INTO t VALUES ('กินข้าวกับปลา');" \
  "SELECT * FROM t WHERE t MATCH 'ปลา';" \
  "SELECT * FROM t WHERE t MATCH 'kin';"
```

## v3 Roadmap

- Accept `synonyms=<path>` argument in `xCreate` to load a custom synonym TSV at table-creation time
- [x] **Stopword suppression** — `stopwords on` argument in `xCreate`; stopword tokens skipped in `xTokenize`
- Expose `ngram_size=N` for custom n-gram configuration on Unknown tokens

## unsafe policy

`unsafe` is confined to `src/lib.rs` (FFI boundary). `src/shim.c` is plain C. Do not add `unsafe` to any other crate.

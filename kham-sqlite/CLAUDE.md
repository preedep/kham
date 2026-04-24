# kham-sqlite

SQLite FTS5 tokenizer extension (`cdylib`) wrapping `kham-core`'s `Tokenizer`.

## Architecture

```
SQLite FTS5  ──▶  src/shim.c (C helpers)  ──▶  lib.rs (Rust entry points + callbacks)
                  SQLITE_EXTENSION_INIT1/2           │
                  kham_sqlite_setup_api()             ▼
                  kham_sqlite_get_fts5api()  sqlite3_kham_init / sqlite3_khamsqlite_init
                                                      │
                                                      ▼
                                             xCreate / xDelete / xTokenize
                                                      │
                                                      ▼
                                             kham_core::Tokenizer::segment()
```

- `shim.c` provides C helpers (`kham_sqlite_setup_api`, `kham_sqlite_get_fts5api`) called from Rust
- `lib.rs` defines `#[no_mangle]` entry points (guaranteed in dylib symbol table) and FTS5 callbacks
- `Tokenizer::segment()` provides zero-copy `Token<'_>` with byte spans for accurate `iStart`/`iEnd`

## Key Files

```
kham-sqlite/
├── Cargo.toml             # crate-type = ["cdylib"], kham-core dep
├── build.rs               # find SQLite headers (xcrun/brew/pkg-config); compile shim.c
└── src/
    ├── lib.rs             # Rust FTS5 callbacks + kham_register_tokenizer
    └── shim.c             # C: SQLITE_EXTENSION_INIT1/2, fts5_api_from_db, entry point
```

## FTS5 Tokenizer Callbacks

| Callback      | Signature                                               | Purpose |
|---------------|---------------------------------------------------------|---------|
| `xCreate`     | `(userdata, azArg, nArg, **ppOut) → int`                | Allocate per-table tokenizer instance |
| `xDelete`     | `(*tokenizer)`                                          | Free tokenizer instance |
| `xTokenize`   | `(*tok, pCtx, flags, pText, nText, xToken) → int`       | Segment document / query text |

**`xTokenize` flow:**
1. Build `&str` from `(pText, nText)` — handles both counted and NUL-terminated forms
2. Call `Tokenizer::new().segment(text)` → `Vec<Token<'_>>` (zero-copy, with `span`)
3. Skip `TokenKind::Whitespace` (Tokenizer default already drops whitespace)
4. Call `xToken(pCtx, 0, pToken, nToken, iStart, iEnd)` for each remaining token
5. Return immediately if any `xToken` call returns non-`SQLITE_OK`

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

`#[repr(C)]` field offsets match the C struct exactly: `c_int` (4 bytes) + implicit 4-byte alignment padding + function pointer (8 bytes on 64-bit). Accessing only the first two fields of `fts5_api` is safe because we receive a pointer to the full struct.

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
INSERT INTO docs VALUES ('วันนี้อากาศดีมาก');

-- Full-text search
SELECT * FROM docs WHERE docs MATCH 'ปลา';
SELECT * FROM docs WHERE docs MATCH 'อากาศ';

-- Phrase search
SELECT * FROM docs WHERE docs MATCH '"กิน ข้าว"';

-- Snippet highlighting (uses iStart/iEnd byte offsets from xTokenize)
SELECT snippet(docs, 0, '<b>', '</b>', '...', 5) FROM docs WHERE docs MATCH 'ปลา';
```

## Token Types

All non-whitespace token kinds are forwarded to SQLite FTS5 without filtering.
SQLite handles punctuation/emoji suppression at the FTS5 level if needed.

| `TokenKind`              | Forwarded? |
|--------------------------|-----------|
| `Thai`                   | ✓ |
| `Latin`                  | ✓ |
| `Number`                 | ✓ |
| `Punctuation`            | ✓ |
| `Emoji`                  | ✓ |
| `Unknown`                | ✓ |
| `Named(_)`               | ✓ |
| `Whitespace`             | — (dropped by `Tokenizer::new()`) |

## v1 Limitations

- **No normalization:** Text is segmented as-is. For Thai text with สระลอย reordering, normalize before insertion: `Tokenizer::new().normalize(text)`.
- **No synonym expansion:** Synonyms from `FtsTokenizer` are not expanded. Add `FTS5_TOKEN_COLOCATED` calls in v2.
- **No stopword filtering:** All tokens are indexed. Use SQLite FTS5 `content=` tables or application-level filtering for stopwords.

## Build Commands

```bash
# Build the shared library
cargo build -p kham-sqlite --release

# Verify exported symbols
nm -D target/release/libkham_sqlite.dylib | grep kham   # macOS
nm -D target/release/libkham_sqlite.so   | grep kham   # Linux

# Quick smoke test with sqlite3 CLI
sqlite3 ':memory:' \
  ".load ./target/release/libkham_sqlite" \
  "CREATE VIRTUAL TABLE t USING fts5(body, tokenize='kham');" \
  "INSERT INTO t VALUES ('กินข้าวกับปลา');" \
  "SELECT * FROM t WHERE t MATCH 'ปลา';"
```

## unsafe policy

`unsafe` is confined to `src/lib.rs` (FFI boundary). `src/shim.c` is plain C. Do not add `unsafe` to any other crate.

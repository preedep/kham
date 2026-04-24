---
name: sqlite-fts
description: Build and test the kham-sqlite SQLite FTS5 tokenizer extension. Use when scaffolding kham-sqlite/, implementing xCreate/xDelete/xTokenize callbacks, debugging SQLite header discovery, verifying fts5_api struct layout, or testing Thai FTS queries in SQLite.
metadata:
  domain: database
  triggers: kham-sqlite, SQLite FTS5, fts5_tokenizer, xTokenize, sqlite3_kham_init, load_extension, fts5_api, SQLite full-text search, Thai SQLite
  role: specialist
---

# sqlite-fts — kham-sqlite FTS5 Extension Builder

Specialist for building the `kham-sqlite` SQLite FTS5 tokenizer extension that wraps `kham-core`'s `Tokenizer`.

## Architecture

```
SQLite FTS5  ──▶  src/shim.c (C)  ──▶  kham_register_tokenizer() (Rust, src/lib.rs)
                  SQLITE_EXTENSION_INIT1/2            │
                  get_fts5_api() — bind-pointer trick  ▼
                  sqlite3_kham_init()        xCreate / xDelete / xTokenize
                                                       │
                                                       ▼
                                              kham_core::Tokenizer::segment()
```

- `shim.c` handles all SQLite extension boilerplate and delegates to Rust via `kham_register_tokenizer()`
- `lib.rs` defines `#[repr(C)]` FTS5 types, implements callbacks, exports `kham_register_tokenizer`
- Uses `Tokenizer::segment()` (not `FtsTokenizer`) for zero-copy `Token<'_>` with byte spans

## Key Files

```
kham-sqlite/
├── Cargo.toml             # crate-type = ["cdylib"], kham-core dep
├── build.rs               # xcrun/brew/pkg-config for SQLite headers; compile shim.c
├── CLAUDE.md              # full reference doc
└── src/
    ├── lib.rs             # Rust FTS5 types + callbacks + kham_register_tokenizer
    └── shim.c             # SQLITE_EXTENSION_INIT1/2, get_fts5_api, sqlite3_kham_init
```

## FTS5 Callback Signatures (Rust)

```rust
// Allocate per-table tokenizer instance
unsafe extern "C" fn kham_fts5_create(
    _p_ctx: *mut c_void,
    _az_arg: *const *const c_char,
    _n_arg: c_int,
    pp_out: *mut *mut KhamFts5Tokenizer,
) -> c_int;

// Free a tokenizer instance
unsafe extern "C" fn kham_fts5_delete(p: *mut KhamFts5Tokenizer);

// Tokenize text and call xToken for each token
unsafe extern "C" fn kham_fts5_tokenize(
    _p: *mut KhamFts5Tokenizer,
    p_ctx: *mut c_void,
    _flags: c_int,
    p_text: *const c_char,
    n_text: c_int,   // -1 = NUL-terminated
    x_token: XTokenFn,
) -> c_int;
```

## Critical: fts5_api Struct Layout

`fts5_api` in `sqlite3.h` (SQLite 3.9+):
```c
struct fts5_api {
  int iVersion;              // 4 bytes @ offset 0; currently 3
                             // 4-byte alignment padding on 64-bit
  int (*xCreateTokenizer)(...); // 8-byte pointer @ offset 8
  int (*xFindTokenizer)(...);
  int (*xCreateFunction)(...);
  // v2 fields (iVersion >= 3)...
};
```

Rust `KhamFts5Api` defines only the first two fields — this is safe because we receive a *pointer*, and `#[repr(C)]` matches the C alignment.

## xToken Byte Offsets

`Token<'a>.span` from `Tokenizer::segment()` gives `Range<usize>` byte offsets into the ORIGINAL input:
```rust
let i_start = token.span.start as c_int;  // → xToken iStart
let i_end   = token.span.end   as c_int;  // → xToken iEnd
```
These are the exact byte offsets SQLite FTS5 needs for `highlight()` and `snippet()`.

## fts5_api_from_db Trick (in shim.c)

Standard SQLite-endorsed approach to retrieve the `fts5_api*`:
```c
static void *get_fts5_api(sqlite3 *db) {
    void *pRet = NULL;
    sqlite3_stmt *pStmt = NULL;
    if (SQLITE_OK == sqlite3_prepare(db, "SELECT fts5(?1)", -1, &pStmt, NULL)) {
        sqlite3_bind_pointer(pStmt, 1, &pRet, "fts5_api_ptr", NULL);
        sqlite3_step(pStmt);
    }
    sqlite3_finalize(pStmt);
    return pRet;
}
```

Returns NULL if FTS5 is not compiled into the SQLite build.

## Build Requirements

- **macOS:** Xcode CLT (`xcrun --show-sdk-path` → `…/usr/include/sqlite3ext.h`) or `brew install sqlite`
  - Override: `SQLITE_INCLUDE_DIR=/path`
- **Linux:** `libsqlite3-dev` (Debian/Ubuntu) or `sqlite-devel` (RHEL)
  - Auto-detected via `pkg-config sqlite3 --cflags-only-I`
- **macOS linker:** `build.rs` emits `-undefined dynamic_lookup`; no equivalent needed on Linux (ELF allows undefined refs in shared libs, and we use vtable calls anyway)

## Build & Test Commands

```bash
# Build
cargo build -p kham-sqlite --release

# Verify symbol exports
nm -D target/release/libkham_sqlite.dylib | grep -E 'kham_register|sqlite3_kham'  # macOS
nm -D target/release/libkham_sqlite.so   | grep -E 'kham_register|sqlite3_kham'  # Linux

# Quick smoke test (requires sqlite3 CLI with FTS5 support)
sqlite3 ':memory:' \
  ".load ./target/release/libkham_sqlite" \
  "CREATE VIRTUAL TABLE t USING fts5(body, tokenize='kham');" \
  "INSERT INTO t VALUES ('กินข้าวกับปลา');" \
  "SELECT * FROM t WHERE t MATCH 'ปลา';"

# Check FTS5 is available in your sqlite3 build
sqlite3 --version && sqlite3 ':memory:' "SELECT fts5_version();"
```

## Common Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| `sqlite3ext.h: No such file` | Headers not found | Set `SQLITE_INCLUDE_DIR` |
| `FTS5 not available` | SQLite built without FTS5 | Install Homebrew sqlite or use `libsqlite3-dev` |
| `no such tokenizer: kham` | Extension not loaded | Verify `.load` path; check `nm` exports |
| Tokens missing byte offsets | Using `FtsTokenizer` (owned String) | Use `Tokenizer::segment()` for zero-copy spans |
| macOS linker error `undefined symbol` | Missing dynamic_lookup flag | Check `build.rs` emits `-undefined dynamic_lookup` |

## v1 Known Limitations

- No normalization (สระลอย). Pre-normalize with `Tokenizer::new().normalize(text)` before insertion.
- No synonym expansion (FTS5_TOKEN_COLOCATED). Planned for v2 using `FtsTokenizer`.
- No stopword filtering. Use application-level filtering or SQLite FTS5 `content=` tables.

## Constraints

- `unsafe` is confined to `src/lib.rs` (FFI boundary)
- Do NOT link against `libsqlite3` — all SQLite API calls go through the `sqlite3_api` vtable
- Do NOT use `rusqlite` crate in `kham-sqlite` — it would pull in a bundled SQLite and conflict
- `fts5_tokenizer` v1 (without locale) is sufficient; do not use `fts5_tokenizer_v2` unless locale support is needed
- `KHAM_TOKENIZER` static is used as a factory template only — SQLite copies the function pointers; per-instance state is allocated in `xCreate`

---
name: postgres-fts
description: Build and test the kham-pg C extension against a local PostgreSQL instance. Use when scaffolding kham-pg/, writing parser callbacks (startfunc/gettoken/endfunc), authoring kham_pg.control and SQL install scripts, debugging symbol export issues, or wiring FtsTokenizer lexemes into a tsvector.
metadata:
  domain: database
  triggers: kham-pg, PostgreSQL text search extension, tsvector, tsquery, text search parser, lexize, to_tsvector, kham_start, kham_gettoken, kham_end, kham_lextypes
  role: specialist
---

# postgres-fts — kham-pg Extension Builder

Specialist for building the `kham-pg` PostgreSQL text search extension that wraps `kham-core`'s `FtsTokenizer` pipeline.

## Architecture

```
PostgreSQL fmgr  ──▶  src/shim.c (C)  ──▶  kham_*_impl() (Rust, src/lib.rs)
                       PG_MODULE_MAGIC
                       PG_FUNCTION_INFO_V1
                       PG_GETARG_POINTER / PG_GETARG_INT32 / PG_RETURN_*
                       palloc / pfree / pstrdup / ereport
```

- `shim.c` handles all PostgreSQL fmgr macro boilerplate and delegates to `*_impl` Rust functions
- `lib.rs` exports `#[no_mangle] pub extern "C"` trampolines that are always in the dynamic symbol table
- `FtsTokenizer::segment_for_fts()` is the single entry point from Rust — normalise → segment → tag stopwords

## Key Files

```
kham-pg/
├── Cargo.toml             # crate-type = ["cdylib"], kham-core dep
├── build.rs               # pg_config --includedir-server, macOS gettext path
├── src/
│   ├── lib.rs             # Rust *_impl functions + #[no_mangle] PG trampolines
│   └── shim.c             # C: PG_MODULE_MAGIC, PG_FUNCTION_INFO_V1, fmgr glue
├── sql/
│   └── kham_pg--0.1.0.sql # CREATE TEXT SEARCH PARSER / DICTIONARY / CONFIGURATION
├── kham_pg.control        # default_version = '0.1.0', relocatable = false
├── Makefile               # build / install / regress / clean targets
└── docker/
    ├── Dockerfile.test    # two-stage: builder (Rust+pg headers) + runner (pg17 only)
    ├── docker-compose.yml
    └── entrypoint.sh      # initdb → pg_ctl start → pg_regress
```

## Parser Callback Signatures

**Critical:** `kham_start` receives a raw `char*` + `int4` length — NOT a varlena `text*`. Use `PG_GETARG_POINTER(0)` + `PG_GETARG_INT32(1)`, never `PG_GETARG_TEXT_PP`.

| Callback        | PG SQL signature                      | What PG passes                              |
|-----------------|---------------------------------------|---------------------------------------------|
| `kham_start`    | `(internal, int4) → internal`         | `char*` buffer + document byte length       |
| `kham_gettoken` | `(internal, internal, internal) → internal` | state ptr + `char**` output + `int*` output |
| `kham_end`      | `(internal) → void`                   | state pointer                               |
| `kham_lextypes` | `(internal) → internal`               | returns palloc'd `LexDescr[]`               |

## Rust impl function signatures (called from shim.c)

```rust
// Tokenise buf[0..len] and return a heap-allocated KhamState pointer.
// Returns NULL on panic — shim converts NULL to a PG ereport(ERROR).
#[no_mangle]
pub unsafe extern "C" fn kham_start_impl(text: *const c_char, len: c_int) -> *mut c_void;

// Write next token into *token / *tokenlen; return PG type int (0 = done).
#[no_mangle]
pub unsafe extern "C" fn kham_gettoken_impl(
    state: *mut c_void,
    token: *mut *const c_char,
    tokenlen: *mut c_int,
) -> c_int;

// Drop the KhamState; NULL is a safe no-op.
#[no_mangle]
pub unsafe extern "C" fn kham_end_impl(state: *mut c_void);
```

## Token Types

| PG type int | alias     | `TokenKind`              |
|-------------|-----------|--------------------------|
| 1           | `thai`    | `TokenKind::Thai`        |
| 2           | `latin`   | `TokenKind::Latin`       |
| 3           | `number`  | `TokenKind::Number`      |
| 4           | `punct`   | `TokenKind::Punctuation` |
| 5           | `emoji`   | `TokenKind::Emoji`       |
| 6           | `unknown` | `TokenKind::Unknown`     |

Whitespace (`TokenKind::Whitespace`) is filtered in `kham_start_impl` before tokens reach PG.

## Required C Headers (include order matters)

```c
#include "postgres.h"           // must be first
#include "fmgr.h"               // PG_FUNCTION_INFO_V1, PG_GETARG_*, PG_RETURN_*
#include "tsearch/ts_public.h"  // LexDescr
#include "utils/palloc.h"       // palloc, pfree, pstrdup
```

Do NOT include `varatt.h` — `kham_start` uses raw pointer args, not varlena.

## Symbol Export Rules

All PG-facing symbols must be `#[no_mangle] pub extern "C"` in `lib.rs`:

```rust
#[no_mangle] pub unsafe extern "C" fn Pg_magic_func() -> *const c_void { ... }
#[no_mangle] pub unsafe extern "C" fn kham_start(fcinfo: Fcinfo) -> Datum { ... }
#[no_mangle] pub unsafe extern "C" fn kham_gettoken(fcinfo: Fcinfo) -> Datum { ... }
#[no_mangle] pub unsafe extern "C" fn kham_end(fcinfo: Fcinfo) -> Datum { ... }
#[no_mangle] pub unsafe extern "C" fn kham_lextypes(fcinfo: Fcinfo) -> Datum { ... }
#[no_mangle] pub extern "C" fn pg_finfo_kham_start() -> *const PgFinfoRecord { ... }
// ... pg_finfo_* for each callback
```

Verify exports after build:

```bash
nm -D target/release/libkham_pg.so | grep -E 'Pg_magic_func|kham_start\b|kham_gettoken\b|kham_end\b|kham_lextypes\b'
```

## SQL Install Script Pattern

```sql
-- kham_pg--0.1.0.sql
CREATE FUNCTION kham_start(internal, int4) RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_start' LANGUAGE C STRICT;

CREATE FUNCTION kham_gettoken(internal, internal, internal) RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_gettoken' LANGUAGE C STRICT;

CREATE FUNCTION kham_end(internal) RETURNS void
    AS 'MODULE_PATHNAME', 'kham_end' LANGUAGE C STRICT;

CREATE FUNCTION kham_lextypes(internal) RETURNS internal
    AS 'MODULE_PATHNAME', 'kham_lextypes' LANGUAGE C STRICT;

CREATE TEXT SEARCH PARSER kham (
    START    = kham_start,
    GETTOKEN = kham_gettoken,
    END      = kham_end,
    LEXTYPES = kham_lextypes
);

CREATE TEXT SEARCH DICTIONARY kham_dict (TEMPLATE = simple);
CREATE TEXT SEARCH CONFIGURATION kham (PARSER = kham);
ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR thai, latin, number, unknown WITH kham_dict;
-- punct and emoji have no mapping — PG discards them at index time
```

## Build Commands

```bash
# Build .so (requires pg_config in PATH or PG_CONFIG env var)
make -C kham-pg build

# Install into host PostgreSQL
make -C kham-pg install

# Run regress tests in Docker (PostgreSQL 17) — preferred
make -C kham-pg regress

# Clean build artefacts
make -C kham-pg clean
```

## macOS Build Notes

- Requires `brew install gettext` — PG headers include `libintl.h` from GNU gettext
- `build.rs` auto-detects Homebrew prefix via `brew --prefix gettext`
- macOS linker: `build.rs` emits `-undefined dynamic_lookup` so PG server symbols (`palloc`, `ereport`) resolve at dlopen time

## PG_MODULE_MAGIC Portability

```c
// shim.c guards for PGDG PG17 (object-like) vs Homebrew/PG18+ (function-like):
#ifdef PG_MODULE_ABI_DATA
PG_MODULE_MAGIC_DATA;
#else
PG_MODULE_MAGIC;
#endif
```

## Docker Test Environment

Two-stage `Dockerfile.test`:
- **Stage 1 (builder):** `debian:bookworm-slim` + `postgresql-server-dev-17` + Rust → `libkham_pg.so`
- **Stage 2 (runner):** `debian:bookworm-slim` + `postgresql-17` only (~200 MB vs ~2 GB single-stage)

Do NOT use Alpine/musl — Rust musl targets are static-only and cannot produce `cdylib`.

Key `entrypoint.sh` constraints:
- `dynamic_shared_memory_type = mmap` must be set before `pg_ctl start` (PG17 removed `none`)
- Run `initdb` and `pg_ctl` as `postgres` via `gosu`
- pg_regress binary path: `$(pg_config --pgxs | xargs dirname | xargs dirname)/test/regress/pg_regress`
- Use `--outputdir=.` so results land in `regress/results/`

## Constraints

- Do NOT use pgrx macros — raw C-ABI extension only
- `unsafe` is confined to `src/lib.rs` FFI boundaries; `src/shim.c` is plain C
- Do NOT add `std`-only code to `kham-core` for PG support — it must remain `no_std`
- `kham_start` MUST use `PG_GETARG_POINTER(0)` + `PG_GETARG_INT32(1)` — never `PG_GETARG_TEXT_PP`
- All four callbacks must return `internal` in SQL (PG17 requirement)

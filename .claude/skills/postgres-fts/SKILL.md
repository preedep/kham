---
name: postgres-fts
description: Build and test the kham-pg C extension against a local PostgreSQL instance. Use when scaffolding kham-pg/, writing parser callbacks (startfunc/gettoken/endfunc), authoring kham_pg.control and SQL install scripts, debugging pgrx/pgx linkage, or wiring FtsTokenizer lexemes into a tsvector.
metadata:
  domain: database
  triggers: kham-pg, PostgreSQL text search extension, pgrx, pg_catalog, tsvector, tsquery, pg_* function, text search parser, lexize, to_tsvector
  role: specialist
---

# postgres-fts — kham-pg Extension Builder

Specialist for building the `kham-pg` PostgreSQL text search extension that wraps `kham-core`'s `FtsTokenizer` pipeline.

## Architecture

`kham-pg` is a PostgreSQL C extension (`cdylib`) that implements a custom text search parser:

- **Parser callbacks** — `startfunc`, `gettoken`, `endfunc` (and optionally `headline`)
- **SQL install script** — `kham_pg--<ver>.sql` registers the parser, dictionary, and text search config
- **Control file** — `kham_pg.control` declares version, relocatable flag, and schema
- **Cargo.toml** — `crate-type = ["cdylib"]`, links against PostgreSQL headers via `pg_config`

The extension calls `kham_core::fts::FtsTokenizer::new().lexemes(text)` to produce a flat
`Vec<String>` which maps directly to PostgreSQL lexeme tokens.

## Key Files

```
kham-pg/
├── Cargo.toml             # cdylib, kham-core dep, build.rs for pg_config
├── build.rs               # links libpq / server headers via pg_config --includedir-server
├── src/
│   └── lib.rs             # #[no_mangle] extern "C" parser callbacks
├── sql/
│   └── kham_pg--0.1.0.sql # CREATE TEXT SEARCH PARSER / DICTIONARY / CONFIGURATION
├── kham_pg.control        # default_version, relocatable = false
└── regress/               # pg_regress test files (see pg-regress skill)
```

## Parser Callback Signatures (PostgreSQL C API)

```c
// startfunc — called once per document; allocates parser state
Datum kham_start(PG_FUNCTION_ARGS);  // receives text *, returns internal state ptr

// gettoken — called repeatedly; returns next token type + text
Datum kham_gettoken(PG_FUNCTION_ARGS);  // (state, token *char, tokenlen *int) → int token_type

// endfunc — frees state
Datum kham_end(PG_FUNCTION_ARGS);

// lextypes — returns token type table (type id → name, description)
Datum kham_lextypes(PG_FUNCTION_ARGS);
```

## Token Types

Map `FtsToken.kind` (from `kham-core`) to PostgreSQL token type integers:

| PG type int | Name        | Description             |
|-------------|-------------|-------------------------|
| 1           | `thai`      | Thai word token         |
| 2           | `latin`     | Latin script token      |
| 3           | `number`    | Numeric token           |
| 4           | `punct`     | Punctuation             |
| 5           | `emoji`     | Emoji token             |
| 6           | `unknown`   | Unknown / OOV token     |

## SQL Install Script Pattern

```sql
CREATE TEXT SEARCH PARSER kham (
    START    = kham_start,
    GETTOKEN = kham_gettoken,
    END      = kham_end,
    LEXTYPES = kham_lextypes
);

CREATE TEXT SEARCH DICTIONARY kham_dict (
    TEMPLATE = simple
);

CREATE TEXT SEARCH CONFIGURATION kham (
    PARSER = kham
);

ALTER TEXT SEARCH CONFIGURATION kham
    ADD MAPPING FOR thai, latin, number WITH kham_dict;
```

## Build Commands

```bash
# Detect PG headers
pg_config --includedir-server

# Build the shared library
cargo build -p kham-pg --release

# Copy .so to PG lib dir (adjust path)
cp target/release/libkham_pg.so $(pg_config --pkglibdir)/kham_pg.so

# Install SQL
psql -c "CREATE EXTENSION kham_pg;"
```

## Constraints

- Do NOT use pgrx macros — this is a raw C-ABI extension linked via `build.rs`
- The only `unsafe` allowed is the `extern "C"` callback bodies (FFI boundary)
- `kham-core` must remain `no_std` — do not add `std` deps to it for PG support
- Stopword positions must be preserved (phrase distance scoring requires them)
- `lexemes()` is the single entry point; do not call `segment_for_fts` directly from PG code

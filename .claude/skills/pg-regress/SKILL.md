---
name: pg-regress
description: Run pg_regress integration tests for SQL correctness of the kham-pg extension. Use when writing .sql test files, expected output files, running make installcheck, debugging pg_regress failures, or setting up the regress/ directory structure.
metadata:
  domain: database
  triggers: pg_regress, installcheck, regress/, SQL test files, expected output, make installcheck, psql regression
  role: specialist
---

# pg-regress — kham-pg Integration Test Runner

Specialist for `pg_regress`-based SQL correctness tests for the `kham-pg` extension.

## Directory Layout

```
kham-pg/regress/
├── sql/                   # input SQL scripts — one file per test suite
│   ├── basic.sql
│   ├── fts_config.sql
│   └── lexemes.sql
└── expected/              # golden output files — exact match required
    ├── basic.out
    ├── fts_config.out
    └── lexemes.out
```

## Running Tests

```bash
# Run all regression tests (requires a running PG instance)
make installcheck

# Run a single test file
pg_regress --inputdir=regress --outputdir=regress/results basic

# Diff a failure
diff regress/expected/basic.out regress/results/basic.out
```

## Makefile Snippet

```makefile
EXTENSION    = kham_pg
DATA         = sql/kham_pg--0.1.0.sql
REGRESS      = basic fts_config lexemes
REGRESS_OPTS = --inputdir=regress

PG_CONFIG    ?= pg_config
PGXS         := $(shell $(PG_CONFIG) --pgxs)
include $(PGXS)
```

## Writing Test Files

Each `.sql` file in `regress/sql/` is run against a fresh database. Use `\set` and `SELECT` to produce deterministic output:

```sql
-- regress/sql/basic.sql
CREATE EXTENSION IF NOT EXISTS kham_pg;

SELECT to_tsvector('kham', 'กินข้าวกับปลา');
SELECT ts_parse('kham', 'กินข้าวกับปลา');
```

The corresponding `.out` file must contain the exact `psql` output including column headers and row counts.

## Updating Expected Output

When output changes intentionally:

```bash
# Run once to generate actual output, then copy to expected/
pg_regress --inputdir=regress --outputdir=regress/results basic
cp regress/results/basic.out regress/expected/basic.out
```

## Common Failure Patterns

| Symptom | Cause | Fix |
|---------|-------|-----|
| `FATAL: could not open file` | `.so` not installed | `make install` before `installcheck` |
| Output diff on whitespace | Column width change | Re-run and accept new expected |
| `ERROR: extension "kham_pg" does not exist` | Wrong `DATA` path in Makefile | Check `kham_pg.control` `default_version` matches SQL filename |
| Token type mismatch | `lextypes` out of sync | Update both `lextypes` callback and `ADD MAPPING FOR` in SQL |

## Constraints

- Each test file must be deterministic — no timestamps, no random data
- Thai segmentation output must match the built-in dictionary; do not depend on custom dict files in regress tests
- Test the `lexemes()` path end-to-end: `to_tsvector('kham', ...)` → assert specific lexemes present
- Never commit `regress/results/` — add to `.gitignore`

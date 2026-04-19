---
name: pg-regress
description: Run pg_regress integration tests for SQL correctness of the kham-pg extension. Use when writing .sql test files, expected output files, running make -C kham-pg regress, debugging pg_regress failures, or adding new test suites.
metadata:
  domain: database
  triggers: pg_regress, regress/, SQL test files, expected output, make regress, kham_fts.sql
  role: specialist
---

# pg-regress — kham-pg Integration Test Runner

Specialist for `pg_regress`-based SQL correctness tests for the `kham-pg` extension.

## Directory Layout

```
kham-pg/regress/
├── sql/                   # input SQL scripts — one file per test suite
│   └── kham_fts.sql       # current suite: parser, tsvector, FTS queries
└── expected/              # golden output files — exact match required
    └── kham_fts.out
```

> `regress/results/` is gitignored — never commit it.

## Running Tests (Docker — required)

All regress tests run inside Docker (PostgreSQL 17). There is no `make installcheck` target — always use:

```bash
# Run all regression tests
make -C kham-pg regress

# Rebuild image and run (after code changes)
make -C kham-pg regress   # --build is always passed by the Makefile
```

The Makefile target runs:

```bash
docker compose -f docker/docker-compose.yml up \
    --build \
    --exit-code-from regress \
    --abort-on-container-exit
```

## Updating Expected Output

When output changes intentionally (new tests, token output changes):

```bash
# 1. Run tests — they will fail with a diff
make -C kham-pg regress || true

# 2. Extract actual output from the container
docker compose -f kham-pg/docker/docker-compose.yml run regress \
    cat /kham/kham-pg/regress/results/output/kham_fts.out \
    > kham-pg/regress/expected/kham_fts.out

# 3. Review the diff, then commit if correct
git diff kham-pg/regress/expected/kham_fts.out
```

Or use the convenience target:

```bash
make -C kham-pg regress-update-expected
# then manually copy results/output/kham_fts.out → expected/kham_fts.out
```

## Adding a New Test Suite

1. Create `kham-pg/regress/sql/<suite>.sql`
2. Create a placeholder `kham-pg/regress/expected/<suite>.out`
3. Add `<suite>` to the `TESTS` list in `kham-pg/docker/entrypoint.sh`
4. Run `make -C kham-pg regress` to generate actual output
5. Copy actual → expected, review, commit

## Writing Test Files

Each `.sql` file runs against a fresh database. Always load the extension first:

```sql
CREATE EXTENSION kham_pg;

-- ts_parse: check token types and text
SELECT tokid, token
FROM ts_parse('kham', 'กินข้าวกับปลา')
ORDER BY tokid, token;

-- tsvector: assert specific lexemes present
SELECT to_tsvector('kham', 'กินข้าวกับปลา') @@ plainto_tsquery('kham', 'ปลา') AS found;

-- to_tsvector content assertion
SELECT to_tsvector('kham', 'กินข้าว')::text;
```

Expected output must include column headers, separator line, rows, and row count exactly as psql prints them:

```
 tokid | token 
-------+-------
     1 | กิน
     1 | ข้าว
(2 rows)
```

## Token Type Reference

| tokid | alias   | Meaning              |
|-------|---------|----------------------|
| 1     | thai    | Thai word            |
| 2     | latin   | Latin script token   |
| 3     | number  | Numeric token        |
| 4     | punct   | Punctuation          |
| 5     | emoji   | Emoji token          |
| 6     | unknown | Unknown / OOV token  |

Whitespace tokens (type 0) are filtered before reaching PG — they never appear in `ts_parse` output.

## Common Failure Patterns

| Symptom | Cause | Fix |
|---------|-------|-----|
| `ERROR: extension "kham_pg" does not exist` | `.so` not installed in Docker image | Rebuild image: `make -C kham-pg regress` |
| Output diff on column width | Thai character width changed | Re-capture expected output |
| `ok 1` but wrong content | Expected file is a placeholder | Capture actual output and replace expected |
| Blank lines in diff | Trailing newline mismatch | Ensure expected file ends with a single `\n` |
| `FATAL: could not open file` | Control or SQL file missing from image | Check `Dockerfile.test` COPY steps |

## Constraints

- Tests must be deterministic — no timestamps, no random data, no `SERIAL` columns
- Thai segmentation output depends on the built-in dictionary; do not load custom dict files in regress tests
- Always assert exact token text, not just `IS NOT NULL` — weak assertions hide regressions
- Never commit `regress/results/` — it is gitignored

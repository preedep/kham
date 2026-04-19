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

## Thai Test Case Checklist

Target: ~60 tests across 4 suites. Current `kham_fts.sql` (19 tests) covers the smoke/happy path.
Use this checklist when expanding coverage. Check off items as suites are added.

### kham_fts.sql — Smoke tests (19 ✅ done)
- [x] Extension loads (`CREATE EXTENSION kham_pg`)
- [x] `ts_token_type('kham')` returns all 6 types (tokid 1–6)
- [x] `ts_parse` pure Thai — correct tokens and tokid
- [x] `ts_parse` Thai sentence with stopword — stopword appears (kham_dict is `simple`, no PG-level filter)
- [x] `ts_parse` mixed Thai + Latin + Number
- [x] `ts_parse` empty string → 0 rows
- [x] `to_tsvector` non-null check
- [x] `to_tsvector::text` exact lexeme positions — pure Thai (`'กิน':1 'ข้าว':2`)
- [x] `to_tsvector::text` exact lexeme positions — sentence with stopword
- [x] `plainto_tsquery` match and no-match
- [x] `to_tsquery` match and no-match
- [x] `to_tsquery` Latin lowercased by simple dict (`Python` → `python`)
- [x] GIN index on table — Thai search returns correct row
- [x] GIN index on table — Latin search returns correct row
- [x] GIN index on table — no match returns 0 rows
- [x] `ts_rank` non-zero for matching document

### kham_thai.sql — Thai language edge cases (0 / ~20 ☐ todo)
Token types:
- [ ] Single Thai character (`ก`) → tokid=6 (Unknown — below TCC threshold)
- [ ] Thai numeral string (`๑๒๓`) → tokid=3 (Number)
- [ ] Thai numeral mixed with Thai (`ราคา๑๕๐บาท`) → Thai + Number + Thai tokens
- [ ] OOV/brand name (`เปปซี่`) → one or more tokid=6 Unknown tokens
- [ ] Punctuation in Thai sentence (`ราคา 500 บาท ลด 10%`) — `%` → tokid=4

Segmentation correctness:
- [ ] Simple 3-word Thai sentence (`แมวกินปลา`) → 3 Thai tokens
- [ ] Compound word (`โรงพยาบาล`) → `โรง` + `พยาบาล` (2 tokens)
- [ ] Common phrase (`สวัสดีครับ`) → `สวัสดี` + `ครับ`
- [ ] Developer compound (`นักพัฒนา`) → `นัก` + `พัฒนา`
- [ ] Whitespace never appears in `ts_parse` output (filter confirmed)

Stopword behaviour:
- [ ] `กับ` appears in `ts_parse` output (not filtered by parser)
- [ ] `ที่` appears in `ts_parse` output
- [ ] `ของ` appears in `ts_parse` output
- [ ] `กับ` appears in `to_tsvector::text` (kham_dict=simple does NOT strip stopwords)

Thai + other scripts:
- [ ] Thai sentence with Arabic number (`กินข้าว 3 มื้อ`) → Thai + Number tokens
- [ ] Thai + Latin brand (`Python สำหรับ AI`) — tokid breakdown correct
- [ ] Mixed Thai + number + percent — all three token types present

Normalisation (verify kham normalises before indexing):
- [ ] Identical meaning text with/without สระลอย reorder → same tsvector lexemes
- [ ] Duplicate วรรณยุกต์ input → deduplicated lexeme in tsvector

### kham_operators.sql — FTS operator coverage (0 / ~12 ☐ todo)
- [ ] AND: `to_tsvector @@ to_tsquery('กิน & ปลา')` — both words present → true
- [ ] AND: one word missing → false
- [ ] OR: `to_tsquery('กิน | หมู')` — one word present → true
- [ ] OR: neither word present → false
- [ ] NOT: `to_tsquery('กิน & !หมู')` — กิน present, หมู absent → true
- [ ] NOT: `to_tsquery('กิน & !ปลา')` — both present → false
- [ ] Phrase: `phraseto_tsquery('kham', 'กิน ข้าว')` matches adjacent tokens
- [ ] Phrase: non-adjacent tokens do not phrase-match
- [ ] `websearch_to_tsquery('kham', 'กิน ปลา')` → both tokens found
- [ ] `websearch_to_tsquery` with `-` exclusion operator
- [ ] `ts_debug('kham', 'กินข้าว')` — verify alias and lexemes columns

### kham_ranking.sql — Ranking and real-world scenario (0 / ~10 ☐ todo)
- [ ] `ts_rank` higher for more occurrences of query term
- [ ] `ts_rank_cd` returns non-zero for match
- [ ] ORDER BY `ts_rank DESC` returns most-relevant row first
- [ ] `ts_headline('kham', body, query)` returns non-null
- [ ] `ts_headline` wraps matched Thai term in `<b>` tags
- [ ] Table with 5+ rows, ranked search returns correct top-1
- [ ] Product catalogue: INSERT 10 Thai product names, search by ingredient → correct rows
- [ ] Article search: INSERT 3 Thai article bodies, search returns ranked results
- [ ] NULL body column — `to_tsvector('kham', NULL)` → NULL (no error)
- [ ] `ts_rank` = 0 for non-matching document

## Constraints

- Tests must be deterministic — no timestamps, no random data, no `SERIAL` columns
- Thai segmentation output depends on the built-in dictionary; do not load custom dict files in regress tests
- Always assert exact token text, not just `IS NOT NULL` — weak assertions hide regressions
- Never commit `regress/results/` — it is gitignored
- When adding a suite, register it in `kham-pg/docker/entrypoint.sh` before running

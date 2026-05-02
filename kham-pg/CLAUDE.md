# kham-pg

PostgreSQL text-search parser extension (`cdylib`) wrapping `kham-core`'s `FtsTokenizer`.

## Architecture

```
PostgreSQL fmgr  ──▶  src/shim.c (C)  ──▶  kham_*_impl() (Rust, src/lib.rs)
                       PG_MODULE_MAGIC
                       PG_FUNCTION_INFO_V1
                       PG_GETARG_POINTER / PG_GETARG_INT32 / PG_RETURN_*
                       palloc / pfree / pstrdup / ereport
```

## Parser callback signatures

| Callback        | SQL signature                             | Notes |
|-----------------|-------------------------------------------|-------|
| `kham_start`    | `(internal, int4) → internal`             | raw `char*` + `int4` length |
| `kham_gettoken` | `(internal, internal, internal) → int4`   | state + char** + int* output |
| `kham_end`      | `(internal) → void`                       | frees state |
| `kham_lextypes` | `(internal) → internal`                   | returns palloc'd `LexDescr[]` |
| `kham_headline` | `(internal, internal, tsquery) → internal` | fills startsel/stopsel/fragdelim; marks selected words |

**Critical:** `kham_start` receives a raw `char*` + `int4` — NOT a varlena `text*`. Use `PG_GETARG_POINTER(0)` + `PG_GETARG_INT32(1)`, never `PG_GETARG_TEXT_PP`.

## Dictionary — kham_fts_dict

Thai and Named tokens go through a custom dictionary that expands each token to up to 6 lexemes at the **same tsvector position**:
1. The normalised word itself
2. Its lk82 Thai Soundex code (enables phonetic-fuzzy search)
3. Its RTGS romanization (if in the built-in map; enables Latin-script search)

Latin, Number, and Unknown tokens use `kham_dict` (simple lowercase pass-through).

### PG16+ lexize calling convention — CRITICAL

In PG 13 and earlier, the lexize callback's 4th argument was `BoolGetDatum(false/true)` (an isNull flag). **Since PG 16 it is a `List*` pointer** of subsequent tokens for multi-word recognition — always non-NULL for real token calls. Any code reading `PG_GETARG_BOOL(3)` interprets this pointer as "true" and returns NULL (stopword) for every token.

Fix: ignore arg3 entirely. Use `token == NULL || len <= 0` to detect the end-of-input finalization call instead:

```c
/* arg3 = List* of subsequent tokens (PG16+); NOT a bool isNull flag */
const char *token = (const char *) PG_GETARG_POINTER(1);
int         len   = PG_GETARG_INT32(2);
if (token == NULL || len <= 0)
    PG_RETURN_POINTER(NULL);  /* end-of-input finalization */
```

## Token type integers

| PG tokid | alias     | `TokenKind`              |
|----------|-----------|--------------------------|
| 1        | `thai`    | `TokenKind::Thai`        |
| 2        | `latin`   | `TokenKind::Latin`       |
| 3        | `number`  | `TokenKind::Number`      |
| 4        | `punct`   | `TokenKind::Punctuation` |
| 5        | `emoji`   | `TokenKind::Emoji`       |
| 6        | `unknown` | `TokenKind::Unknown`     |
| 7        | `named`   | `TokenKind::Named(_)`    |

`LexDescr[]` in `kham_lextypes_shim` must be null-terminated (lexid=0 sentinel at index 7).

## SQL install objects (`kham_pg--0.x.y.sql`)

Created in this order:
1. `CREATE FUNCTION kham_start/gettoken/end/lextypes/headline/dict_lexize` — registers C symbols
2. `CREATE FUNCTION kham_dict_lexize_udom83/kham_dict_lexize_metasound` — udom83 / MetaSound variants (0.7.0+)
3. `CREATE TEXT SEARCH PARSER kham` — wires up the parser functions
4. `CREATE TEXT SEARCH TEMPLATE kham_fts_template` + `CREATE TEXT SEARCH DICTIONARY kham_fts_dict` — expands Thai/Named tokens to [word, lk82_soundex, RTGS_romanization, ascii_number?, pos_tag?]; suppresses stopwords
5. `CREATE TEXT SEARCH TEMPLATE/DICTIONARY kham_fts_template_udom83/kham_fts_dict_udom83` — udom83 soundex variant (0.7.0+)
6. `CREATE TEXT SEARCH TEMPLATE/DICTIONARY kham_fts_template_metasound/kham_fts_dict_metasound` — MetaSound variant (0.7.0+)
7. `CREATE TEXT SEARCH DICTIONARY kham_dict` — `simple` template (lowercase pass-through for Latin/Unknown)
8. `CREATE TEXT SEARCH CONFIGURATION kham` — uses `kham` parser
9. `ALTER … ADD MAPPING FOR thai, named WITH kham_fts_dict` — phonetic/stopword/POS expansion
10. `ALTER … ADD MAPPING FOR number WITH kham_fts_dict` — Thai digit normalization (0.7.0+; was kham_dict)
11. `ALTER … ADD MAPPING FOR latin, unknown WITH kham_dict` — simple pass-through
12. `CREATE FUNCTION kham_tsvector(text) RETURNS tsvector` — SQL STABLE helper; shorthand for `to_tsvector('kham', …)` (0.7.0+)
13. `CREATE FUNCTION kham_tsquery(text) RETURNS tsquery` — SQL STABLE helper; shorthand for `plainto_tsquery('kham', …)` (0.7.0+)

Punctuation and emoji have no mapping — PG discards those token types at index time.

**0.7.0 behavior changes vs 0.6.0:**
- Stopwords suppressed: `kham_fts_dict` returns NULL for Thai stopwords → token omitted from tsvector.
- Thai number normalization: `number` tokens now route through `kham_fts_dict`, which adds the ASCII form as a colocated lexeme (e.g. ๑๒๓ → ['๑๒๓', '123']).
- POS lexemes: tokens with known POS emit `pos_<tag>` (e.g. `pos_noun`, `pos_verb`) as an extra colocated lexeme.
- Two new soundex dict alternatives: `kham_fts_dict_udom83` and `kham_fts_dict_metasound`.

## README files — two separate documents

| File | Purpose |
|------|---------|
| `README.md` | Workspace-facing developer README. May reference `make -C kham-pg`, Docker regress, CLAUDE.md, and other workspace concepts. |
| `README.pgxn.md` | **PGXN distribution README.** Standalone — zero references to the workspace, CLAUDE.md, Docker regress, or any path prefixes. Bundled as `README.md` inside the PGXN zip by `make dist`, and rendered as the extension detail page on pgxn.org. |

**Rule:** keep `README.pgxn.md` pure — only build requirements, `make install`, SQL usage examples, and public links. Never add workspace-specific commands or internal developer notes to it.

## Build requirements

- `pg_config` in `PATH` **or** `PG_CONFIG=/path/to/pg_config`
- C compiler (clang or gcc) — `cc` crate compiles `src/shim.c`
- **macOS only:** `brew install gettext` — PG headers include `libintl.h`. `build.rs` auto-detects via `brew --prefix gettext`.
- For regress tests: Docker with BuildKit

## Required C header include order

```c
#include "postgres.h"           // must be first
#include "fmgr.h"               // PG_FUNCTION_INFO_V1, PG_GETARG_*, PG_RETURN_*
#include "tsearch/ts_public.h"  // LexDescr
#include "utils/palloc.h"       // palloc, pfree, pstrdup
```

`varatt.h` is only needed for varlena args — `kham_start` uses raw pointer args so it is not included.

## PostgreSQL version support

| Version | Status |
|---------|--------|
| PG 14   | Supported — tested via `regress-matrix` |
| PG 15   | Supported — tested via `regress-matrix` |
| PG 16   | Supported — tested via `regress-matrix` |
| PG 17   | Supported — default CI target |
| PG 18   | Supported — tested via `regress-matrix` |

Compatibility notes:
- **PG 16+**: `lexize` callback 4th arg changed from `bool` to `List*`. `shim.c` ignores arg3 entirely — works on all versions.
- **PG 18+**: `PG_MODULE_MAGIC_DATA` is function-like. Guarded with `#ifdef PG_MODULE_ABI_DATA` in `shim.c`.

## Docker test environment

Multi-stage build (`kham-pg/docker/Dockerfile.test`):
- **Stage 1 (builder):** `debian:bookworm-slim` + `postgresql-server-dev-${PG_VERSION}` + Rust → `libkham_pg.so`
- **Stage 2 (runner):** `debian:bookworm-slim` + `postgresql-${PG_VERSION}` only (~200 MB vs ~2 GB single-stage)
- `PG_VERSION` build arg selects the major version (default 17). Each major version requires its own compiled `.so` because `PG_MODULE_MAGIC` is version-stamped.

```bash
make -C kham-pg regress                  # PG 17 (default)
make -C kham-pg regress PG_VERSION=16   # single-version override
make -C kham-pg regress-matrix          # PG 14, 15, 16, 17, 18 in sequence
```

Do **not** use Alpine/musl: Rust musl targets are static-only and do not support `cdylib`.

Key constraints:
- `dynamic_shared_memory_type = mmap` must be set before `pg_ctl start` (PG 17 removed `none`)
- pg_ctl/initdb run as `postgres` via `gosu`
- `pg_regress` binary: `$(pg_config --pgxs | dirname | dirname)/test/regress/pg_regress`
- Use `--outputdir=.` so results land at `regress/results/` (gitignored)
- Linux cdylib: all PG-facing symbols defined as `#[no_mangle] pub extern "C"` in `lib.rs`
- `PG_MODULE_MAGIC_DATA` portability: PGDG PG17 uses object-like form; Homebrew/PG18+ uses function-like form. `shim.c` guards with `#ifdef PG_MODULE_ABI_DATA`
- macOS linker: `build.rs` emits `-undefined dynamic_lookup` so PG server symbols resolve at dlopen time

## Regress tests

Expected output: `kham-pg/regress/expected/` (committed). Results: `kham-pg/regress/results/` (gitignored).

Test files: `kham_fts.sql`, `kham_features.sql`, `kham_thai.sql`, `kham_operators.sql`, `kham_ranking.sql`, `kham_advanced.sql`

`kham_features.sql` (0.7.0+) covers: stopword suppression, Thai number normalization, udom83/MetaSound dict variants, POS lexeme indexing and querying.

**NE test words:** Use single-syllable words (e.g. จีน) for named entity regress tests — multi-syllable words (e.g. กรุงเทพ) are split by the segmenter before NE tagging. Verify with `Tokenizer::new().segment("candidate")` before adding to the test.

**Updating expected output:** NEVER write expected `.out` files by hand — the format has invisible trailing spaces on every column header line, a `NOTICE` line when the extension already exists, and no `CREATE TABLE`/`INSERT 0 N` command tags for single-line DDL. Always capture from Docker:
```bash
# capture actual output for all tests
docker compose -f kham-pg/docker/docker-compose.yml run --rm --build \
  -v "$(pwd)/kham-pg/regress/results:/kham/kham-pg/regress/results" regress

# promote to expected after verifying correctness
for f in kham-pg/regress/results/*.out; do
  cp "$f" kham-pg/regress/expected/"$(basename "$f")"
done
```

**Regress test authoring pitfalls:**

- **lk82 soundex cross-word matching** — Thai labial consonants (ป ผ พ ภ บ ม) all fall in the same soundex class. A query for ปลา (pla) will phonetically match documents containing ผัก (phak) or other labial-initial words. Avoid `ts_rank(doc_A) > ts_rank(doc_B)` comparisons between Thai documents; the phonetic match usually makes them equal. Use `ts_rank(...) > 0` to verify a match exists, or compare a match against a clearly zero-rank result from a non-Thai document.

- **Single-word tsvector inputs** — `to_tsvector('kham', 'ปลา')` (a single standalone Thai word) may behave differently from multi-word input. Always use natural Thai phrases (`'กินข้าวกับปลา'`) for ts_rank tests to ensure the segmenter operates in its normal context. The `@@` match operator works on single words; ts_rank comparisons do not reliably.

- **pg_regress output format** — Column headers have a trailing space (` is_emoji `, not ` is_emoji`). Single-line DDL (`CREATE TABLE foo (...);`) produces no command tag; multi-line DDL produces `CREATE TABLE` / `INSERT 0 N`. The second and later `CREATE EXTENSION IF NOT EXISTS` calls emit a `NOTICE` line. These are invisible when editing by hand — capture from Docker instead.

- **MetaSound code format** — MetaSound initial-consonant groups 10+ use uppercase letters (A–J), not digits. `metasound("ปลา") = "B06G16"` (ป → group B). The pattern `'^[0-9]'` misses these codes; use `'^[0-9A-J]{3}'` to match any MetaSound code regardless of initial-consonant group.

- **Stopword position collapsing** — When `kham_fts_dict` returns NULL for a stopword, PostgreSQL does NOT reserve a position slot for it. Subsequent tokens shift down: in `'กินข้าวกับปลา'` with กับ suppressed, กินข้าว=pos1 and ปลา=pos2 (not pos3). `phraseto_tsquery('กินข้าว ปลา')` therefore matches as adjacent. Write phrase-distance tests against sentences with no stopwords between the tokens of interest, or account for this shift explicitly.

## Benchmark suite

`kham-pg/bench/` contains a Docker-based benchmark with two sections:

```bash
make -C kham-pg bench
```

### Section 1 — cache-warm batch throughput (`bench.sql`)

`generate_series` + `CASE i%3` cycles 3 unique documents. PostgreSQL caches STABLE function results within a single `ExprContext`; after the first 3 calls, subsequent iterations hit the cache. Numbers represent **amortised cache-warm throughput** — useful for estimating bulk indexing rates, not single-call latency.

| Operation | Iterations |
|-----------|-----------|
| `to_tsvector` small (~63 B) | 50 000 |
| `to_tsvector` medium (~630 B) | 5 000 |
| `to_tsvector` large (~6.3 KB) | 500 |
| `plainto_tsquery` single word | 50 000 |
| `plainto_tsquery` 3 words | 50 000 |

### Section 2 — true per-call latency (`pgbench`)

pgbench runs each transaction in a fresh `ExprContext`; the function-result cache does NOT carry over between transactions. 20 unique Thai sentences (`bench_setup.sql`) are seeded first; each pgbench transaction picks one at random via `\set id random(1,20)`. Numbers represent **actual single-call tokenizer latency**.

| Script | Transactions |
|--------|-------------|
| `bench_pgbench_tsvector.sql` | 10 000 |
| `bench_pgbench_tsquery.sql`  | 10 000 |

GIN-indexed scan and `ts_rank` are excluded from the Docker suite — building a stored tsvector column or large table triggers thousands of real `to_tsvector` calls that cause the bench to hang in Docker. Use `EXPLAIN ANALYZE` or `pgbench` against a real PG instance for GIN/ts_rank timings.

Files: `bench/bench.sql`, `bench/bench_setup.sql`, `bench/bench_pgbench_tsvector.sql`, `bench/bench_pgbench_tsquery.sql`, `bench/Dockerfile.bench`, `bench/docker-compose.yml`, `bench/entrypoint.sh`.

**`RAISE NOTICE` format pitfall** — `RAISE NOTICE` substitutes bare `%` positionally; it does NOT support `printf`-style width/precision specifiers like `%-42s` or `%10.3f`. Use `format()` + `to_char()` instead:

```sql
-- Wrong: %-42s and %10.3f are emitted as literals
RAISE NOTICE '%-42s %10.3f', label, value;

-- Correct:
RAISE NOTICE '%', format('%-42s %10s', label, to_char(value::numeric, 'FM9990.000'));
```

**Function-result cache vs. constant-folding** — two distinct mechanisms:
- *Constant-folding*: optimizer evaluates a STABLE function at plan time when all inputs are literals. Avoided by binding inputs to a PL/pgSQL variable.
- *Function-result cache*: runtime caches STABLE results within one `ExprContext`. With only 3 unique `CASE i%3` inputs, PG caches after 3 calls; all subsequent generate_series rows are cache hits. This is why Section 1 numbers are cache-warm throughput, not cold-call latency. Use pgbench (Section 2) for cold-call measurements.

## unsafe policy

`unsafe` is confined to `src/lib.rs` (FFI boundary). `src/shim.c` is plain C. Do not add `unsafe` to any other crate.

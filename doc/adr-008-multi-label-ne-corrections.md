# ADR-008 — Multi-Label NE Tags in kham-tnc Corrections

**Status:** Accepted  
**Date:** 2026-05-03  
**Scope:** kham-tnc annotation layer only; kham-core unaffected

---

## Context

When human annotators review a Thai corpus in kham-tnc, a single token can legitimately
belong to more than one Named Entity category. Examples:

| Token | Labels | Reason |
|-------|--------|--------|
| จุฬาลงกรณ์มหาวิทยาลัย | ORG + PLACE | University = institution (ORG) that occupies a campus location (PLACE) |
| กรุงเทพมหานคร | PLACE + ORG | Capital city (PLACE) and administrative corporation (ORG) |
| สยามพิวรรธน์ | ORG + PLACE | Shopping-mall operator (ORG) and the physical mall compound (PLACE) |

The kham-tnc corrections table (`correct_ne TEXT`) currently stores one NE tag per word.
The question is where multi-label support should live and how it should be represented.

---

## Decision

### 1. kham-core stays single-label

`kham-core` exposes `TokenKind::Named(NamedEntityKind)` as a public API used by all
bindings (Python/WASM/C FFI/pg/sqlite). Changing the payload from a single enum to a
`Vec` or set would be a **breaking change** across every binding and downstream consumer.
The gazetteer (`ne_th.tsv`) assigns one canonical NE tag per entry; the greedy
longest-match tagger returns one tag per merged span. This is unchanged.

### 2. Multi-label lives in the corrections layer only

The `corrections.correct_ne` column accepts a pipe-delimited string such as `"PLACE|ORG"`.
This represents a human annotation override — it is not produced by the core segmenter.

Storage rule: tags are stored in a canonical order (alphabetical), so `ORG|PLACE` is
normalised to `PLACE|ORG` at save time if the exporter needs a stable key.

### 3. Export convention — "primary label last" for ne_th.tsv

When a multi-label correction is exported to `ne_th.tsv`, each tag is written on its own
line:

```
กรุงเทพมหานคร	PLACE
กรุงเทพมหานคร	ORG
```

kham-core's TSV parser uses a `BTreeMap` with last-entry-wins on duplicate keys. The
annotator should place the **most semantically specific / primary label last** so that
kham-core picks it as the canonical tag. In the example above, `ORG` ends up as the
stored tag.

If both labels are equally valid, annotators should choose the label that best serves
the downstream FTS / search use case (typically PLACE for geographic entities, ORG for
institutions).

### 4. UI — NE pills become multi-select toggles

In the tag editor modal the NE pill row changes from single-select (radio) to multi-select
(checkbox-style toggle). Selecting "— ไม่ระบุ" clears all other NE selections. Selecting
any specific tag deactivates "ไม่ระบุ" and toggles that tag independently.

---

## Alternatives Considered

### A. Change `kham-core` to multi-label

`TokenKind::Named(Vec<NamedEntityKind>)`, `FtsToken.ne: Vec<NamedEntityKind>`,
`NeTagger` map to `Vec<NamedEntityKind>`. This is a breaking public API change requiring
updates to Python bindings (`kham-python/src/lib.rs`), WASM bindings, C FFI header
(`kham.h`), kham-pg, and kham-sqlite. The FTS pipeline would need to decide how to
serialize multiple NE tags into PostgreSQL `ts_vector` — there is no standard mechanism.
Deferred to a future ADR if kham-core use cases require it.

### B. Separate `token_ne_tags` junction table in kham-tnc

A proper `(word, ne_tag)` join table. Zero ambiguity, clean SQL joins. But requires a
schema migration of the `corrections` table, complicates every query that reads
`correct_ne`, and adds join overhead for a feature that affects < 1% of entries. Not
worth the complexity at current scale.

### C. JSON array in correct_ne

`correct_ne = '["PLACE","ORG"]'`. Supports future extension (confidence scores, source
attribution). Heavier than pipe-delimited; SQLite JSON functions required for filtering;
overkill for a 2–3 element list of fixed strings.

---

## Consequences

- **kham-core:** no changes, no version bump.
- **kham-tnc:** `correct_ne` column accepts pipe-delimited strings without a schema
  migration (the column is `TEXT`, any string is valid).
- **Export:** `api.rs` NE TSV export handler splits on `|` and emits one row per tag.
  Downstream re-import into `ne_th.tsv` must be reviewed manually to confirm which label
  is "primary" (last in the file).
- **UI rendering:** corrections table and tag modal render multiple NE chips when the
  value contains `|`.
- **WangchanBERTa sidecar:** the `/api/suggest` endpoint returns a single NE tag from the
  transformer model. The "ใช้" (apply) button toggles that tag on without clearing
  existing selections, so human annotators can supplement the AI suggestion with
  additional labels.

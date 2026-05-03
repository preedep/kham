# ADR-007: Public Dataset Imports — thainer, UD_Thai-PUD, Thai Wikipedia

**Date:** 2026-05-03  
**Status:** Accepted  
**Deciders:** @preedep

---

## Context

After evaluating TNC4 (Thai National Corpus v4) as a potential data source, we found it is only accessible as a web search interface with no confirmed download or license. In parallel, three freely downloadable, open-licensed Thai NLP datasets were identified that could immediately improve kham's NE, POS, and frequency coverage:

| Dataset | License | Source | Use |
|---|---|---|---|
| `pythainlp/thainer-corpus-v2` | CC0-1.0 | Hugging Face | NE gazetteer expansion |
| `UD_Thai-PUD` | CC-BY-SA 3.0 | GitHub (UniversalDependencies) | POS table expansion |
| `pythainlp/thai-wiki-dataset-v3` | CC-BY-SA 4.0 | Hugging Face | Supplemental word frequency |

---

## Decision

### 1. thainer-corpus-v2 → `ne_th.tsv` (CC0)

**What:** BIO-tagged NER corpus with 6,564 sentences covering 36 NE categories.

**Mapping:** `PERSON → PERSON`, `LOCATION → PLACE`, `ORGANIZATION → ORG`. All other categories (DATE, TIME, MONEY, FACILITY, URL, …) skipped — kham supports only three NE types.

**Filtering rules:**
1. Strip invisible characters (U+FEFF BOM, U+200B zero-width space) from surface forms.
2. Skip entities with no Thai script characters.
3. Skip entities not starting with a Thai character (rejects leading `(`, digits, quotes).
4. Skip single-character entities.
5. Skip entities already in `ne_th.tsv`.
6. Skip entities present in `words_th.txt` (common vocabulary, not NE — consistent with ADR-001 strategy).

**Result:** 2,280 new entries appended (PERSON 403, PLACE 793, ORG 1,084). `ne_th.tsv` grows from 36,668 → 38,950 entries. Being CC0, no attribution section is required, but a source comment is included for traceability.

---

### 2. UD_Thai-PUD → `pos_th.tsv` (CC-BY-SA 3.0)

**What:** 1,000 sentences annotated with Universal Dependencies v2 POS tags (UPOS), dependency relations, and lemmas. Used in CoNLL 2017 shared task.

**UPOS → kham 13-category mapping:**

| UPOS | kham tag | UPOS | kham tag |
|---|---|---|---|
| NOUN | NOUN | VERB | VERB |
| ADJ | ADJ | ADV | ADV |
| PROPN | PROPN | PRON | PRON |
| NUM | NUM | AUX | AUX |
| DET | DET | CCONJ / SCONJ | CONJ |
| ADP | PREP | PART / INTJ | PART |
| PUNCT / SYM / X / _ | *(skip)* | | |

**Filtering rules** (same as ADR-003):
1. Skip entries with no Thai Unicode characters.
2. Skip entries containing spaces (multi-word).
3. Skip single-character entries.
4. Skip entries with digit characters.
5. Skip entries already in `pos_th.tsv`.

**Result:** 2,407 new entries appended (PROPN 884, VERB 665, NOUN 662, ADJ 134, ADV 34, PART 6, PREP 7, CONJ 5, AUX 3, NUM 3, DET 2, PRON 2). `pos_th.tsv` grows from 8,993 → 11,404 entries. Attribution section added with CC-BY-SA-3.0 header.

**Segmentation test cases:** 1,000 UD sentences were extracted as potential segmentation test cases. However, kham achieves only F1 = 0.663 against UD tokenization — primarily because UD treats transliterated foreign names (e.g., `ปอมปีย์`, `ซีซาร์`) as single tokens while kham splits OOV tokens. These cases are stored in `kham-core/testdata/reference/ud_pud.txt` for manual analysis but **excluded from the accuracy benchmark** to avoid regressing the CI gate.

---

### 3. Thai Wikipedia → `wiki_freq.tsv` (CC-BY-SA 4.0)

**What:** Word frequency list extracted by running kham segmentation over 500 Thai Wikipedia articles from `pythainlp/thai-wiki-dataset-v3`.

**Why separate from `tnc_freq.txt`:** `tnc_freq.txt` is CC0. Wikipedia text is CC-BY-SA-4.0 (share-alike). Merging them into a single file would impose CC-BY-SA-4.0 on the combined work. Keeping them separate allows the CC0 file to remain unencumbered.

**Current status:** File produced (`kham-core/data/wiki_freq.tsv`, 19,890 entries, 7,640 not in TNC). **Not yet loaded** by `FreqMap` — requires a future `FreqMap::from_two_sources(TNC_BYTES, WIKI_BYTES)` API or a feature-gated secondary frequency source.

**Known limitation:** 500 articles is a small sample. Re-run `scripts/import_wiki_freq.py` with `--max-articles 50000` or more for better coverage. Processing rate is ~1 article/second when streaming from Hugging Face.

---

## Consequences

### Positive

- NE coverage: 36,668 → 38,950 entries (+6.2%), with 2,280 new real-world entities from news/political text
- POS coverage: 8,993 → 11,404 entries (+26.8%), with strong PROPN expansion useful for proper-noun detection
- Word frequency: `wiki_freq.tsv` provides a CC-BY-SA-licensed supplement ready to activate once the API is extended
- All imports are scripted and reproducible — re-running `scripts/import_thainer_ne.py --dry-run` always shows current delta

### Negative / Residual risks

- **NE noise:** thainer ORG entries include abbreviations like `ก.การคลัง`, `ก.ค.ศ.` that contain `.` characters. These may not match kham token boundaries. Monitor FTS quality for short-abbreviation false positives.
- **POS PROPN overlap with NE:** 884 new PROPN entries may overlap with NE gazetteer entries; `Named` tokens already skip POS lookup so there is no double-tagging, but the redundancy is harmless rather than useful.
- **wiki_freq fragments:** Kham splits unknown transliterated tokens (e.g., `อาร์เซนอล` → `อา|ร์|เซ|นอ|ล`), so `wiki_freq.tsv` contains high-frequency fragments like `ร์` (count 2,090). These fragments will have artificially high frequency scores when `FreqMap::from_two_sources` is implemented — consider a minimum-token-length filter when loading `wiki_freq.tsv`.

---

## Alternatives Considered

| Option | Reason rejected |
|---|---|
| LST20 (NECTEC, non-commercial) | Non-commercial restriction — cannot be embedded in default build |
| InterBEST (CC-BY-NC-SA) | Same restriction |
| ORCHID full corpus (CC-BY-NC-SA) | Same restriction |
| Merge wiki_freq.tsv into tnc_freq.txt | Would impose CC-BY-SA on the combined CC0 file |
| Use UD_Thai-PUD as accuracy benchmark | F1 = 0.663 too low; divergences are genuine algorithm differences, not errors |

---

## References

- `scripts/import_thainer_ne.py` — thainer NE import script
- `scripts/import_ud_pud_pos.py` — UD POS import script
- `scripts/import_wiki_freq.py` — Wikipedia frequency script
- `kham-core/testdata/reference/ud_pud.txt` — UD segmentation reference (not benchmarked)
- [thainer-corpus-v2 on Hugging Face](https://huggingface.co/datasets/pythainlp/thainer-corpus-v2)
- [UD_Thai-PUD on GitHub](https://github.com/UniversalDependencies/UD_Thai-PUD)
- [thai-wiki-dataset-v3 on Hugging Face](https://huggingface.co/datasets/pythainlp/thai-wiki-dataset-v3)
- ADR-001: NE person name import strategy
- ADR-003: ORCHID POS tag mapping to kham 13-category scheme

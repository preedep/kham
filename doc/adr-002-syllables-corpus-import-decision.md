# ADR-002: syllables_th.txt Import Decision — Abbreviations Skipped, Syllables Excluded

**Date:** 2026-04-25  
**Status:** Accepted  
**Deciders:** @preedep

---

## Context

PyThaiNLP ships `syllables_th.txt` (CC0, 10,322 lines) as a sub-word segmentation aid. The file contains:

| Category | Count | Description |
|---|---|---|
| Plain syllables (≤ 6 chars, no dot) | 9,673 | Thai phonetic syllable units |
| Abbreviation forms (contain `.`) | 561 | Thai government / institution abbreviations |
| Thai digits and special marks | 10 | ๐–๙, ฯ, ๆ |

69 of the 561 abbreviation forms already exist in `kham-core/data/abbrev_th.tsv`.  
492 abbreviation forms are absent from `abbrev_th.tsv`.

PyThaiNLP uses this file as a longest-match dictionary for its ETCC tokenizer:

```python
Tokenizer(get_corpus("etcc.txt"), engine="longest")  # etcc.py
```

---

## Decision

### 1. Do not add the 9,673 syllable entries to `words_th.txt`

kham-core's segmenter works in two stages:

1. **`pre_tokenizer`** — splits input into script-homogeneous spans (Thai / Latin / Number / Punctuation).
2. **`segmenter`** (DAG/newmm) — runs dictionary matching only *within* Thai spans.

Adding raw syllables to the word dictionary would make the DP scorer prefer syllable-length matches over whole-word matches wherever a syllable string is a prefix of a longer word. This causes **over-segmentation**: e.g. if `กิน` is in the dict, adding the syllable `กิ` would split `กิน` into `กิ` + `น` (Unknown) whenever the longer match scores no better.

kham-core already handles sub-syllable boundaries via its **TCC rules** (`src/tcc.rs`). There is no gain from adding syllable units to the word dictionary.

### 2. Do not add the 492 abbreviation forms to `abbrev_th.tsv`

`abbrev_th.tsv` requires both the abbreviated *form* and its canonical *expansion*:

```
# form         primary_expansion    [alt1  alt2...]
ก.ค.           กรกฎาคม
พ.ศ.           พุทธศักราช
```

`syllables_th.txt` provides only the abbreviated forms (e.g. `ก.ก.ต.`, `กนอ.`, `กทพ.`) with no expansions.
Without expansions, the entries cannot drive the pre-tokenisation text replacement in `AbbrevMap::expand_text`.

Additionally, the abbreviations belong to a specialised government/administrative domain. Providing incorrect or guessed expansions would silently corrupt FTS queries.

### 3. Do not implement an ETCC tokenizer from `etcc.txt`

`etcc.txt` (133,584 lines) is a dictionary for a different algorithm (ETCC longest-match), not a set of rules that improve kham-core's regex-based TCC. Implementing ETCC would be an additive feature (`feature = "etcc"`), not a TCC enhancement, and is deferred.

---

## Consequences

### Positive

- No over-segmentation regression from syllable dictionary pollution.
- No silent corruption of abbreviation expansions from missing expansion data.
- `words_th.txt` and `abbrev_th.tsv` remain high-precision, manually-curated data sets.

### Negative / Residual risks

- 492 government abbreviations (e.g. `ก.ล.ต.`, `กนอ.`, `กทพ.`) remain unhandled. Users querying by abbreviation in FTS will not match expanded canonical forms unless they type the abbreviation exactly.
- Future work: curate expansions for high-frequency government abbreviations from an authoritative source (e.g. Royal Thai Government Gazette).

---

## Alternatives Considered

| Option | Reason rejected |
|---|---|
| Add syllables to `words_th.txt` | Causes over-segmentation; TCC rules already handle boundaries |
| Add abbreviation forms without expansions to `abbrev_th.tsv` | `AbbrevMap::expand_text` requires expansions; forms-only entries are inert and misleading |
| Guess expansions from abbreviation patterns | Error-prone; incorrect expansions corrupt FTS silently |
| Implement ETCC tokenizer from `etcc.txt` | Different algorithm; additive feature, not a TCC improvement |

---

## References

- PyThaiNLP corpus: <https://github.com/PyThaiNLP/pythainlp/tree/main/pythainlp/corpus>
- `syllables_th.txt` CC0 — PyThaiNLP
- `etcc.txt` CC0 — PyThaiNLP; usage in `pythainlp/tokenize/etcc.py`
- kham-core TCC implementation: `kham-core/src/tcc.rs`
- kham-core abbreviation expansion: `kham-core/src/abbrev.rs`

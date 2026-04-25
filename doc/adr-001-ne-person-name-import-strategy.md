# ADR-001: NE Person Name Import Strategy — Dictionary-Filter Approach

**Date:** 2026-04-23  
**Status:** Accepted (license corrected by ADR-004)  
**Deciders:** @preedep

> **License correction (2026-04-25):** This ADR originally stated the person name corpora were Apache-2.0. The authoritative source (`pythainlp/corpus/corpus_license.md`) confirms the actual license is **CC-BY-SA-4.0** (thai-names-corpus by Korkeat Wannapat). Attribution headers in `ne_th.tsv` have been corrected. See ADR-004.

---

## Context

The NE gazetteer (`kham-core/data/ne_th.tsv`) was initially hand-curated (~200 entries). PyThaiNLP
provides two open-licensed person-name corpora under Apache-2.0:

- `person_names_female_th.txt` — 5,098 Thai feminine given names
- `person_names_male_th.txt` — 7,124 Thai masculine given names

Importing these would increase the PERSON category from ~10 entries to ~10,000, dramatically improving
named-entity recall for Thai text. However, Thai given names are derived from common vocabulary
morphemes (Sanskrit/Pali loanwords used in everyday speech), so a naive full import causes severe
false positives.

### Observed problem

A sample of names from the lists also appear as ordinary Thai words:

| Name | Common meaning | False-positive risk |
|------|---------------|---------------------|
| กนก | gold, a flower | HIGH — appears in any text about gold |
| กมล | lotus | HIGH — appears in poetry, botany texts |
| กร | hand, action | HIGH — extremely common morpheme |
| กัลยา | beautiful woman | HIGH — common adjective |
| กาญจนา | related to gold/Kanchanaburi | HIGH — appears in place names |
| พร | blessing | HIGH — appears everywhere |

Adding these to the gazetteer as `PERSON` would tag every occurrence — in any context — as a
named entity, breaking FTS precision and `ts_parse` token type accuracy in `kham-pg`.

### Why not import all names without filtering?

Tested on a realistic Thai sentence: "กนก คือ ดอกไม้ สีทองอีกชื่อหนึ่ง" (กนก is a golden flower).
Without filtering, the word กนก would be tagged `Named(Person)` even though it refers to a plant.
The false-positive rate for common Thai content was estimated at 15–20% of Thai tokens.

---

## Decision

**Import only names that do not appear in `words_th.txt` (the built-in Thai word dictionary).**

Filter logic:
```python
if name not in dictionary:   # dictionary = set of lines from words_th.txt
    include(name, "PERSON")
else:
    discard(name)            # common Thai word — too ambiguous
```

Results after filtering:
- Female names: 5,098 → **4,542 kept**, 556 discarded
- Male names: 7,124 → **5,720 kept**, 1,404 discarded
- Combined unique (deduplicated): **10,041** names added
  - Female-only: 4,321
  - Male-only: 5,499
  - Unisex (appear in both lists): 221

Names with ≤ 3 Thai chars in the kept set: **59** — these are not in the dictionary so their
false-positive risk is low (they are rare/modern names with no established common-word meaning).

---

## Consequences

### Positive

- Named-entity recall for Thai personal names increases dramatically
- Multi-token NE tagging (implemented in ADR-002) handles compound names that the segmenter splits
  (e.g. กนกวรรณ → กนก+วร+รณ → merged as `Named(Person)`)
- Remaining false positives are bounded: only names whose full string is not a dictionary word
- License is compatible: Apache-2.0, same as `words_th.txt` and `stopwords_th.txt`

### Negative / Residual risks

- **Residual false positives:** Names whose components are dictionary words but the full name is not
  (e.g. กนกวรรณ — "กนกวรรณ" as a whole is not in the dict, but it means "golden writing").
  In practice, compound names appearing verbatim in running text almost always refer to a person.
- **Proper-name ambiguity:** Some filtered-in names still share spelling with rare dictionary words
  not covered by `words_th.txt`. Manual curation remains the ultimate precision mechanism.
- **Gazetteer size:** ~10,000 entries increases the NE lookup cost. The greedy 5-token scan is
  O(5 × log N) per Thai token; at N ≈ 10,400 this is ~70 comparisons per token — acceptable.
- **No surnames:** This import covers given names only. Surnames are a separate corpus not yet
  imported (they tend to be more unambiguous and could be added in a future ADR).

### Not done / Future options

- **Surname import:** Thai surnames (`surnames_th.txt`) are less ambiguous and could complement
  this import. Planned as a follow-up.
- **Frequency-based filtering:** Instead of a binary dictionary check, weight by TNC corpus
  frequency — exclude names whose full string appears frequently as a non-NE context.
- **Context-sensitive NE (ML):** A CRF or BiLSTM model would eliminate most residual false
  positives but requires `#[cfg(feature = "ml")]` gating and a labelled corpus.

---

## Alternatives Considered

| Option | Reason rejected |
|--------|----------------|
| Import all names unfiltered | ~15–20% false-positive rate on common Thai text; unacceptable for `kham-pg` token type accuracy |
| Import only multi-syllable names (≥ 2 segments) | Requires running the segmenter on 12k names; slower; "multi-syllable" still includes กัลยา, กาญจนา which are ambiguous |
| Manual curation only | Current 10-entry PERSON set has near-zero recall; not scalable |
| Surname list instead of given names | Surnames are less common in Thai text; lower impact on recall |

---

## References

- PyThaiNLP corpus: <https://github.com/PyThaiNLP/pythainlp/tree/main/pythainlp/corpus>
- thai-names-corpus: <https://github.com/korkeatw/thai-names-corpus/> (CC-BY-SA-4.0)
- `words_th.txt` source: PyThaiNLP CC0-1.0 (`kham-core/data/words_th.txt`)
- Multi-token NE implementation: `kham-core/src/ne.rs` — `NeTagger::tag_tokens`
- ADR-004: family names import and full license correction

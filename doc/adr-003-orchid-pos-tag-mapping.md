# ADR-003: ORCHID POS Tag Mapping to kham-core 13-Category Scheme

**Date:** 2026-04-25  
**Status:** Accepted  
**Deciders:** @preedep

---

## Context

kham-core's `PosTagger` uses a hand-curated TSV (`kham-core/data/pos_th.tsv`) with **338 entries** across 13 categories. PyThaiNLP ships `pos_orchid_unigram.json` (CC-BY-4.0), a unigram POS model trained on the ORCHID corpus with **15,211 entries** across **44 tags**.

Importing ORCHID data would expand POS coverage roughly **45×**, significantly improving the `FtsToken::pos` field accuracy in the FTS pipeline.

### ORCHID tagset (44 tags, partial)

| ORCHID tag | Meaning | Count |
|---|---|---|
| NCMN | Common noun | 10,660 |
| NPRP | Proper noun | 1,508 |
| VACT | Active verb | 1,147 |
| VSTA | Stative verb | 414 |
| VATT | Attributive verb (adj-like) | 258 |
| ADVN | Adverb | 202 |
| CMTR | Classifier / measure word | 195 |
| JSBR | Subordinating conjunction | 132 |
| RPRE | Preposition | 81 |
| NLBL | Noun label / abbreviation | 79 |
| … | … | … |

### Import challenges

1. **Tag granularity mismatch**: ORCHID has 44 tags; kham-core has 13. A mapping is required.
2. **English entries**: 5,855 of 15,211 entries contain no Thai script (English words, Latin abbreviations, mixed phrases). These cannot be matched by kham-core's Thai-only POS lookup and must be excluded.
3. **Multi-word entries**: Entries like `"มา กว่า"` (space-separated) cannot be looked up as a single token. Excluded.
4. **Single-character entries**: Single-char tokens (e.g. `จ` tagged NLBL) are already handled by the segmenter or pre_tokenizer. Excluded to avoid false POS assignments.
5. **Unmapped tag `DIAC`** (16 entries): Diacritical markers; mapped to `PART` as they function as particles.
6. **License**: CC-BY-4.0 requires attribution in data file headers.

---

## Decision

### Tag mapping (ORCHID → kham-core)

| ORCHID tag | kham-core tag | Rationale |
|---|---|---|
| NCMN | NOUN | Common noun |
| NPRP | PROPN | Proper noun |
| NLBL | NOUN | Noun label / title abbreviation |
| NTTL | NOUN | Title word (นาย, นาง, ดร.) |
| NCNM | NUM | Numeric noun |
| FIXN | NOUN | Fixed noun expression |
| NONM | NOUN | Nominal |
| VACT | VERB | Active verb |
| VSTA | VERB | Stative verb (is-a / has-a predicates) |
| FIXV | VERB | Fixed verb expression |
| VATT | ADJ | Attributive verb — functions as adjective in Thai |
| ADVN | ADV | Adverb |
| ADVP | ADV | Adverb phrase head |
| ADVS | ADV | Sentential adverb |
| ADVI | ADV | Interrogative adverb |
| CMTR | CLAS | Classifier / measure |
| CNIT | CLAS | Numeral classifier |
| CLTV | CLAS | Collective classifier |
| CVBL | CLAS | Verbal classifier |
| CFQC | CLAS | Frequentative classifier |
| JSBR | CONJ | Subordinating conjunction |
| JCRG | CONJ | Coordinating conjunction |
| JCMP | CONJ | Comparative conjunction |
| RPRE | PREP | Preposition |
| PNTR | PRON | Pronoun |
| PDMN | PRON | Demonstrative pronoun |
| PPRS | PRON | Personal pronoun |
| PREL | PRON | Relative pronoun |
| DCNM | NUM | Numeral determiner (หนึ่ง, สอง attributively) |
| DONM | NUM | Ordinal numeral |
| DDAN | DET | Definite determiner (นั้น, นี้) |
| DDAC | DET | Attributive determiner |
| DIBQ | DET | Interrogative determiner |
| DDBQ | DET | Distributive determiner |
| DIAQ | DET | Interrogative quantifier |
| DDAQ | DET | Distributive quantifier |
| XVBM | AUX | Pre-verb modifier |
| XVAE | AUX | Post-verb ending modifier |
| XVMM | AUX | Mid-verb modifier |
| XVAM | AUX | Aspect marker |
| XVBB | AUX | Pre-verb basic modifier |
| NEG | PART | Negation particle |
| EITT | PART | Exclamation / interjection |
| DIAC | PART | Diacritical marker (particle-like) |
| PUNC | *(skip)* | Punctuation — handled by `TokenKind::Punctuation` |

### Filtering rules applied

1. **Skip** entries with no Thai Unicode characters (no `[฀-๿]` codepoints).
2. **Skip** entries containing spaces (multi-word phrases).
3. **Skip** single-character entries.
4. **Skip** entries whose word already exists in `pos_th.tsv` (existing hand-curated data takes precedence).
5. **Skip** `PUNC` and `CMTR@PUNC` tags (punctuation is not POS-tagged in kham-core).

### Result

| Metric | Count |
|---|---|
| Total ORCHID entries | 15,211 |
| New Thai single-word entries added | 9,052 |
| Skipped — already in pos_th.tsv | 259 |
| Skipped — no Thai characters | 5,855 |
| Skipped — unmapped/punct tag | 16 + 29 |

New entries by kham-core tag: NOUN 5,328 · VERB 1,416 · PROPN 1,261 · ADV 313 · ADJ 222 · CONJ 144 · CLAS 119 · PREP 66 · AUX 51 · DET 45 · PRON 38 · NUM 36 · PART 13.

### Attribution

The ORCHID-derived section of `pos_th.tsv` carries the following header:

```
# ORCHID corpus — CC-BY-4.0 — PyThaiNLP Project
# Tag mapping: see doc/adr-003-orchid-pos-tag-mapping.md
```

---

## Consequences

### Positive

- POS coverage expands from 338 to ~9,390 entries (~28× increase).
- `FtsToken::pos` field is now meaningful for a much larger vocabulary.
- PROPN entries (1,261) provide a secondary signal for proper noun detection, complementing the NE gazetteer.
- VSTA (stative verbs) and VATT (attributive verbs) are correctly imported — stative verbs are tagged VERB, attributive verbs (functioning as adjectives) are tagged ADJ.

### Negative / Residual risks

- **Tag collapse**: ORCHID's 44 tags carry more information than kham-core's 13. For example, VACT (action verb) and VSTA (stative verb) both become VERB. Downstream consumers cannot distinguish them.
- **Unigram ambiguity**: A unigram model assigns one tag per word regardless of context. Highly ambiguous Thai words (e.g. ดี can be VSTA "to be good" or NCMN "bile") will receive a single fixed tag.
- **NPRP → PROPN overlap with NE**: 1,261 proper nouns tagged PROPN may partially overlap with NE gazetteer entries. The NE tagger runs before POS — `Named` tokens already skip POS lookup, so there is no double-tagging.

---

## Alternatives Considered

| Option | Reason rejected |
|---|---|
| Map VSTA → ADJ (stative verb as adjective) | Thai stative verbs pattern as verbs syntactically; ADJ is less accurate |
| Map VATT → VERB | VATT forms modify nouns directly like adjectives; ADJ is more accurate for FTS use |
| Import NPRP entries into `ne_th.tsv` instead of `pos_th.tsv` | NE gazetteer already has 10,400 entries via ADR-001; NPRP unigram model adds noise (no NE type label) |
| Import all entries including English | English words in Thai POS data are transliterated loans; kham-core's POS lookup is Thai-script only |
| Use perceptron model instead of unigram | Perceptron weights are not a word→tag table; cannot be converted to TSV format without inference code |

---

## References

- PyThaiNLP corpus: <https://github.com/PyThaiNLP/pythainlp/tree/main/pythainlp/corpus>
- `pos_orchid_unigram.json` CC-BY-4.0 — PyThaiNLP Project
- ORCHID corpus: Sornlertlamvanich et al. 1997, NECTEC
- kham-core POS implementation: `kham-core/src/pos.rs`
- kham-core POS data: `kham-core/data/pos_th.tsv`

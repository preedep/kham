# ADR-004: Family Names Import and License Correction

**Date:** 2026-04-25  
**Status:** Accepted  
**Deciders:** @preedep

---

## Context

The NE gazetteer (`kham-core/data/ne_th.tsv`) already contains ~10,041 Thai given names (male and female) imported under ADR-001. The `family_names_th.txt` corpus from PyThaiNLP contains **9,836 Thai family names** not yet imported.

### License correction

ADR-001 incorrectly attributed the person name corpora as **Apache-2.0**. The authoritative source (`pythainlp/corpus/corpus_license.md`) confirms all three name files originate from the [thai-names-corpus](https://github.com/korkeatw/thai-names-corpus/) by Korkeat Wannapat and are licensed under **CC-BY-SA-4.0**:

| File | Correct license |
|---|---|
| `person_names_male_th.txt` | CC-BY-SA-4.0 |
| `person_names_female_th.txt` | CC-BY-SA-4.0 |
| `family_names_th.txt` | CC-BY-SA-4.0 |

The PLACE/ORG sections of `ne_th.tsv` (countries, provinces, organisations) remain CC0-1.0 or hand-curated (MIT OR Apache-2.0).

This ADR also corrects ADR-001's license statement.

### Family names import analysis

Applying the same ADR-001 filter strategy (exclude names that appear in `words_th.txt`):

| Metric | Count |
|---|---|
| Family names total | 9,836 |
| Excluded — appear in `words_th.txt` (ambiguous) | 584 |
| Excluded — already in `ne_th.tsv` | 272 |
| New entries added | **8,980** |

Sample filtered-out names (also common Thai words):  
`กรรณิการ์` (a flower), `กรุณา` (please/kindness), `กล้าหาญ` (brave), `กลิ่นหอม` (fragrant scent).

---

## Decision

### 1. Import 8,980 filtered family names into `ne_th.tsv`

Apply the same dictionary-filter strategy as ADR-001:

```python
if name not in words_th and name not in existing_ne and len(name) > 1:
    include(name, "PERSON")
```

Family names are appended as a distinct section with a CC-BY-SA-4.0 attribution header.

### 2. Do not gate behind `feature = "extended-ne"`

Given names (10,041 entries) are already embedded in the default build since v0.2.0 without a feature flag. Adding a feature flag for family names while given names remain ungated would create an inconsistent API. The pragmatic decision is to embed family names in the same section.

### 3. Fix attribution headers throughout `ne_th.tsv`

- File header: correct "Apache-2.0" → "CC-BY-SA-4.0" for person name sources; correct "Apache-2.0" → "CC0-1.0" for `countries_th.txt`
- Section headers: correct inline attribution for female/male given name sections

### 4. License note in data file

The file header documents that `ne_th.tsv` contains mixed-license data:

```
# License note: PERSON sections (given names + family names) are CC-BY-SA-4.0.
#   PLACE and ORG sections are CC0-1.0 or hand-curated (MIT OR Apache-2.0).
```

---

## Consequences

### Positive

- NE Person coverage expands from ~10,063 to **~19,043 entries** (+8,980 family names)
- Multi-token NE matching now resolves compound person names including surnames
- License attribution is now accurate throughout `ne_th.tsv`

### Negative / Residual risks

- **CC-BY-SA share-alike on PERSON sections:** Consumers who redistribute a modified version of the PERSON sections of `ne_th.tsv` must do so under CC-BY-SA-4.0. The kham-core source code (MIT OR Apache-2.0) is unaffected — CC licenses treat code and data separately.
- **Residual false positives:** Family names are generally less ambiguous than given names (they rarely appear as common vocabulary), but some remain (e.g. `กรรณิการ์` → correctly excluded; `กงพาน` → included, unlikely to appear as a non-NE token).
- **No surname disambiguation:** A name like `สมชาย กงพาน` (given + family) may tag both as Person separately, which is correct. However, a text referencing the Klong Phan (กลองพาน) instrument is not affected since `กลองพาน` ≠ `กงพาน`.

---

## Alternatives Considered

| Option | Reason rejected |
|---|---|
| `feature = "extended-ne"` flag | Given names already embedded without flag since v0.2.0; inconsistent to gate only family names |
| Separate `ne_th_family_ccbysa.tsv` file | Adds complexity (two files to load, two builtin() paths); share-alike already applies to existing given names |
| Import all 9,836 without dictionary filter | ~584 family names overlap with Thai vocabulary; false-positive risk established by ADR-001 analysis |
| Skip family names entirely | Significant NER recall gap for full Thai person names (given + family); counterproductive |

---

## References

- PyThaiNLP corpus license: <https://github.com/PyThaiNLP/pythainlp/blob/main/pythainlp/corpus/corpus_license.md>
- thai-names-corpus: <https://github.com/korkeatw/thai-names-corpus/>
- CC-BY-SA-4.0: <https://creativecommons.org/licenses/by-sa/4.0/>
- ADR-001: `doc/adr-001-ne-person-name-import-strategy.md` (given names import; license corrected by this ADR)
- kham-core NE implementation: `kham-core/src/ne.rs`

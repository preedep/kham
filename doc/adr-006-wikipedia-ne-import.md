# ADR-006: Wikipedia Thai Titles — NE PLACE/ORG Import

**Date:** 2026-04-25  
**Status:** Accepted  
**Deciders:** @preedep

---

## Context

The NE gazetteer (`kham-core/data/ne_th.tsv`) contains ~19,043 PERSON entries (given names + family names from ADR-001, ADR-004) and a small hand-curated set of PLACE/ORG entries (Thai provinces, countries). PLACE and ORG coverage is minimal for landmarks, institutions, and foreign places that appear in Thai text.

`wikipedia_titles_th.txt` from PyThaiNLP is a CC-BY-SA-4.0 dump of **290,055 Thai Wikipedia article titles** — all Thai-script, no Latin, no spaces. It is a rich source of proper nouns.

### Why Wikipedia titles work for gazetteer expansion

Wikipedia titles are article names: they are almost always proper nouns. Non-proper-noun articles exist (หัวใจ = heart, แว่นตา = glasses) but they are common words and will be caught by the `words_th.txt` filter.

---

## Decision

### Filtering strategy

Apply sequential filters; an entry is included only if it passes all of them:

| Filter | Count removed |
|---|---|
| `> 25 Thai chars` (complex compound phrases unlikely to appear verbatim) | 24,774 |
| Already in `ne_th.tsv` | 3,418 |
| Appears in `words_th.txt` (ambiguous common vocabulary) | 15,693 |
| Starts with exclusion prefix (see below) | 11,345 |
| Contains `ของ` (descriptive/possessive phrase) | 1,984 |
| No PLACE or ORG keyword match | 215,601 |
| **Remaining — imported** | **17,240** |

**Exclusion prefixes** catch action nouns, royal persons (not organizations), battle/operation names, and list articles:

```
การ  ความ  ผู้  เรื่อง
สมเด็จ  พระเจ้า  พระบาท  พระยา  พระราช  พระบรม
เจ้าพระยา  หลวง  หม่อม
กรมหมื่น  กรมหลวง  กรมพระ  กรมขุน   ← royal ranks, not ORG departments
ยุทธการ  ปฏิบัติการ  สงคราม  การรบ  …  ← military operations
ที่ราบ  ปล่อง  แนวเขา                    ← geographic type nouns, not proper names
รายชื่อ  รายการ  รายนาม  …              ← list articles
มติ  สนธิสัญญา  ข้อตกลง  …             ← treaties / resolutions
```

### Keyword classification

**PLACE** — keyword must be a **prefix** of the title (or, for 4 highly distinctive infix markers, appear anywhere):

- Administrative: `จังหวัด`, `อำเภอ`, `ตำบล`, `หมู่บ้าน`, `แขวง`, `เขต`
- Political units: `ประเทศ`, `สาธารณรัฐ`, `ราชอาณาจักร`, `รัฐ`, `แคว้น`, `มณฑล`, `สหพันธ์สาธารณรัฐ`, `สหพันธรัฐ`
- City prefixes: `กรุง`, `นคร`, `เมือง`
- Geographic features (prefix only): `เขา`, `ดอย`, `ภู`, `เกาะ`, `อ่าว`, `ถ้ำ`, `หาด`
- Transport: `ท่าอากาศยาน`, `สนามบิน`, `ท่าเรือ`, `ถนน`, `ทางหลวง`
- Conservation: `อุทยานแห่งชาติ`, `เขตรักษาพันธุ์`, `วนอุทยาน`
- Water: `แม่น้ำ`, `ลำน้ำ`, `คลอง`, `แหลม`, `ช่องแคบ`, `คาบสมุทร`
- Infix-allowed (distinctive): `ทะเลสาบ`, `น้ำตก`, `มหาสมุทร`, `หมู่เกาะ`

**ORG** — keyword must be a **prefix** of the title:

- Education: `มหาวิทยาลัย`, `วิทยาลัย`, `โรงเรียน`, `สถาบัน`
- Healthcare: `โรงพยาบาล`
- Religion: `วัด`, `มัสยิด`, `โบสถ์`, `อาราม`
- Transport: `สถานีรถไฟ`, `สถานีรถไฟฟ้า`
- Government: `กระทรวง`, `กรมทหาร`, `กองทัพ`, `กองพล`, `กองพัน`, `สำนักงาน`
- Commerce: `บริษัท`, `ธนาคาร`, `ห้าง`, `ตลาดหลักทรัพย์`
- Sports: `สนามกีฬา`, `สนามแข่ง`
- Civil society: `องค์การ`, `องค์กร`, `สมาคม`, `มูลนิธิ`, `สหพันธ์`, `สหภาพ`, `พรรค`
- Cultural: `พิพิธภัณฑ์`, `หอสมุดแห่งชาติ`, `หอศิลป์`
- Media: `สถานีโทรทัศน์`, `สถานีวิทยุ`
- Diplomatic: `สถานเอกอัครราชทูต`, `สถานกงสุล`

### Import results

| Category | Entries added |
|---|---|
| PLACE | 7,705 |
| ORG | 9,535 |
| **Total new entries** | **17,240** |

Selected high-value entries included: `ท่าอากาศยานสุวรรณภูมิ`, `ท่าอากาศยานดอนเมือง`, `เกาะสมุย`, `เกาะช้าง`, `ดอยสุเทพ`, `ภูกระดึง`, `วัดพระแก้ว`, `วัดโพธิ์`, `วัดอรุณ`, `โรงพยาบาลจุฬาลงกรณ์`, `มหาวิทยาลัยมหิดล`, `สถานีรถไฟหัวลำโพง`, and thousands of international places and institutions.

### No PERSON import from Wikipedia

Wikipedia person article titles (politicians, athletes, historical figures, etc.) are excluded because:
1. PERSON names are already well-covered by the 19,043-entry given-name + family-name corpus (ADR-001, ADR-004)
2. Person articles in Wikipedia follow naming conventions that may include birth name, titles, and transliterated foreign names — hard to classify reliably
3. Foreign person names transliterated into Thai are not in `words_th.txt`, so the ambiguity filter doesn't apply; however, without keyword classification there is no reliable way to distinguish them from other proper nouns

### License

`wikipedia_titles_th.txt` is CC-BY-SA-4.0 (Thai Wikipedia / Wikimedia Foundation). The imported section of `ne_th.tsv` inherits this license. The PLACE/ORG section is therefore CC-BY-SA-4.0. This extends the existing share-alike obligation already in place for the PERSON sections (ADR-004).

The `ne_th.tsv` file header is updated to document the mixed-license composition:

```
# PLACE and ORG sections:
#   Hand-curated (MIT OR Apache-2.0) and Wikipedia titles (CC-BY-SA-4.0)
```

---

## Consequences

### Positive

- NE PLACE coverage expands from ~200 hand-curated entries to **~7,900** (×40)
- NE ORG coverage expands from ~30 hand-curated entries to **~9,565** (×320)
- International places (countries, cities, rivers, mountains) and foreign institutions are now tagged correctly
- Thai temples, universities, hospitals, airports, train stations, and national parks are covered
- Multi-token NE matching (`NeTagger::tag_tokens`) handles compound names that the segmenter splits (e.g. `ท่าอากาศยาน` + `สุวรรณภูมิ` → merged as `Named(Place)`)

### Negative / Residual risks

- **False positives**: The keyword-prefix approach classifies by structure, not semantics. A few movie or book titles that happen to start with `วัด`, `สนาม`, `พรรค`, etc. may be included. The `words_th.txt` filter removes common vocabulary, but obscure content titles are not filtered.
- **25-char cap excludes some valid NEs**: Long compound names like `สถานีโทรทัศน์กองทัพบกช่อง5` (26 chars) are excluded. This is intentional — very long entries are unlikely to appear verbatim in running text within the 5-token greedy scan window.
- **CC-BY-SA share-alike**: Consumers who redistribute a modified version of the PLACE/ORG sections must do so under CC-BY-SA-4.0. The kham-core source code (MIT OR Apache-2.0) is unaffected.
- **No person names from Wikipedia**: Foreign historical figures, politicians, and athletes are not tagged as Named(Person) by this import. Adding them would require a more sophisticated classification step.

---

## Alternatives Considered

| Option | Reason rejected |
|---|---|
| Import all 290k titles | ~215k are non-NE topics (concepts, events, animals, plants, etc.); would pollute gazetteer and cause severe false positives |
| ML-based NE classification (CRF/BERT) | Requires labelled corpus and inference infrastructure; out of scope for a rules-based NLP library |
| Wikipedia category API for PLACE/ORG filtering | Requires network access at build time; defeats embedded, offline-first design |
| Include titles > 25 chars | Long compounds fragment into 5+ tokens; greedy 5-token NE scan cannot cover them; compile-time binary size increases disproportionately |
| Import PERSON names from Wikipedia | Too ambiguous without category metadata; person name corpus already comprehensive |

---

## References

- PyThaiNLP corpus license: <https://github.com/PyThaiNLP/pythainlp/blob/main/pythainlp/corpus/corpus_license.md>
- Thai Wikipedia: CC-BY-SA-4.0, Wikimedia Foundation
- CC-BY-SA-4.0: <https://creativecommons.org/licenses/by-sa/4.0/>
- ADR-001: given names import (`doc/adr-001-ne-person-name-import-strategy.md`)
- ADR-004: family names import and license correction (`doc/adr-004-family-names-import.md`)
- kham-core NE implementation: `kham-core/src/ne.rs`

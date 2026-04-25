# kham Roadmap & Action Checklist

Tracks pending improvements, data imports, and feature work post-v0.3.0.

## Released

| Version | Highlights |
|---|---|
| v0.1.0 | Core segmenter, DARTS dict, TCC, pre-tokenizer, CLI, Python/WASM/C bindings |
| v0.1.2 | PostgreSQL FTS5 extension (`kham-pg`), stopwords, synonyms, ngrams, `FtsTokenizer` |
| v0.1.3 | pg_regress suite (67 tests across 4 suites) |
| v0.2.0 | POS tagging, NER, RTGS romanization, number normalization, SQLite FTS5 (`kham-sqlite`) |
| v0.3.0 | Abbreviation expansion (`AbbrevMap`), Thai date parsing, sentence segmentation |

---

## Active priorities

- [ ] **PGXN upload** — upload `kham_pg-0.3.0.zip` once pgxn.org account is active
- [ ] **`ts_headline` support** — HEADLINE callback in `kham-pg/src/shim.c` + `lib.rs`

---

## PyThaiNLP corpus imports

Source: <https://github.com/PyThaiNLP/pythainlp/tree/main/pythainlp/corpus>

### CC0 — no attribution required (embed freely)

- [x] **`orst_words_th.txt`** → merged into `kham-core/data/words_th.txt` (74 new words, CC0)
- [x] **`negations_th.txt`** → already present in `stopwords_th.txt` (ไม่, แต่); no action needed
- [ ] **`etcc.txt`** → not directly applicable to `tcc.rs`; it is a 133k-entry dictionary for ETCC longest-match tokenization (a different algorithm). Could be a future `feature = "etcc"` tokenizer. See ADR-002.
- [x] **`syllables_th.txt`** → reviewed; syllable entries skipped (over-segmentation risk); abbreviation entries skipped (no expansions provided). See ADR-002.
- [x] **`ttc_freq.txt`** → 2,410 TTC-only words appended to `tnc_freq.txt` (CC0). See ADR-005.
- [x] **`phupha_word_freqs.txt`** → excluded; character-level data, not word-level. See ADR-005.

### CC-BY-4.0 — attribution required in docs/license headers

- [ ] **`th_en_transliteration_v1.4.tsv`** → deferred; this is transliteration (loan word → English source), not RTGS romanization. Would need a separate `transliterate()` API.
- [x] **`pos_orchid_unigram.json`** → 8,691 new entries appended to `kham-core/data/pos_th.tsv` (CC-BY-4.0, PyThaiNLP). See ADR-003.

### CC-BY-SA-4.0 — share-alike; keep in separate optional TSV files

- [x] **`person_names_male_th.txt`** + **`person_names_female_th.txt`** → already in `ne_th.tsv` since v0.2.0 (10,041 entries); attribution corrected to CC-BY-SA-4.0 (was wrongly Apache-2.0). See ADR-001, ADR-004.
- [x] **`family_names_th.txt`** → 8,980 entries appended to `ne_th.tsv` (CC-BY-SA-4.0, filtered by words_th.txt). See ADR-004.
- [x] **`wikipedia_titles_th.txt`** → 17,240 entries appended to `ne_th.tsv` (7,705 PLACE + 9,535 ORG, CC-BY-SA-4.0). See ADR-006.

---

## Deferred / low priority

- [ ] **kham-pg token type 7 for Named** — change `Named(_) => 1` → `Named(_) => 7` in `lib.rs`; add `named` lextype in `shim.c`; requires Docker pg_regress update
- [ ] **kham-sqlite v2** — synonym expansion via `FTS5_TOKEN_COLOCATED`, normalization, stopword filtering
- [ ] **Spelling correction** — edit-distance based; requires significant ML or DP work
- [ ] **Word embeddings / semantic similarity** — requires ML inference; defer indefinitely

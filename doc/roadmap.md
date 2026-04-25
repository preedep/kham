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
| v0.4.0 | Compound-first DP scoring (F1 0.418→0.975 vs PyThaiNLP), Thai Soundex (lk82/udom83/MetaSound/cross-lang), NE/POS/freq data expansion, `kham-bench-accuracy`, `--soundex` CLI flag |

---

## Active priorities

- [ ] **PGXN upload** — upload `kham_pg-0.3.0.zip` once pgxn.org account is active
- [ ] **`ts_headline` support** — HEADLINE callback in `kham-pg/src/shim.c` + `lib.rs`

---

## Accuracy benchmark

Unit tests verify specific known cases but do not measure overall segmentation quality or catch regressions across real text. An accuracy benchmark would report **precision / recall / F1** at word-boundary level and enable comparisons against PyThaiNLP and nlpO3.

**Corpus constraints:** BEST corpus is excluded (non-CC0). Options for gold-standard data:

- Curate a CC0 gold set from `kham-core/testdata/` (safe, limited coverage)
- Source a CC0/CC-BY corpus (e.g., ORCHID subsets or a manually annotated set)
- Generate synthetic benchmarks from the existing dictionary + known segmentations

**Suggested implementation:**

- New binary crate `kham-bench-accuracy` (separate from criterion perf benchmarks)
- Reads `input|tok1|tok2|…` files from `kham-core/testdata/` (same format as integration tests)
- Outputs per-file and aggregate precision / recall / F1
- Runs via `cargo run -p kham-bench-accuracy` — not part of `cargo bench` (which is throughput only)
- CI gate: fail if F1 drops below a configurable threshold vs. a stored baseline

- [x] **Accuracy benchmark** — `kham-bench-accuracy` binary, precision/recall/F1 against gold corpus; `--threshold` CI gate; `--verbose` failing-case output
- [x] **PyThaiNLP comparison script** — `scripts/compare_pythainlp.py`; 39 built-in sentences; kham vs `word_tokenize(engine='newmm')`; 37/39 agreed (94.9%), F1 0.975; **genuine diffs: 0** (remaining 2 are confirmed PyThaiNLP errors — see below)

### PyThaiNLP segmentation divergences (kham is correct)

Two sentences in `KNOWN_PYTHAINLP_ERRORS` (scripts/compare_pythainlp.py) where kham and PyThaiNLP newmm disagree but kham is linguistically correct. Both are frequency-score tie-breaks where PyThaiNLP's frequency table differs from TNC.

| Sentence | kham (correct) | PyThaiNLP (wrong) | Root cause |
|---|---|---|---|
| `ซื้อหนังสือสามเล่มจากร้าน` | `จาก\|ร้าน` | `จา\|กร้าน` | จาก=174k+ร้าน=13k vs จา=2k+กร้าน=142; `จา` (archaic vow) + `กร้าน` (calloused) is meaningless in a shopping context |
| `รัฐบาลไทยประกาศมาตรการควบคุมโรคระบาด` | `มาตรการ\|ควบคุม` | `มาตร\|การควบคุม` | มาตรการ=4k+ควบคุม=11k vs มาตร=646+การควบคุม=**0 TNC hits**; `มาตรการ` is the standard government term |

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

## Thai phonetic encoding (Soundex)

Phonetic encoding groups words/names by sound — useful for fuzzy search, spell correction,
and name matching (especially transliterated foreign names). All rule-based variants are
pure-Rust `no_std` compatible. Suggested module: `kham-core/src/soundex.rs`, public API:
`soundex_lk82(word: &str) -> String`, etc.

Module: `kham-core/src/soundex.rs`. Public API: `SoundexAlgorithm` enum + `soundex(word, algo)` + `sounds_like(a, b, algo)` + direct `lk82(word)` / `udom83(word)` / `metasound(word)` functions.

| Algorithm | Effort | Notes |
|---|---|---|
| **lk82** (Lorchirachoonkul 1982) | Low | 4-char alphanumeric code; most widely used in Thai NLP |
| **udom83** (Udompanich 1983) | Low | Finer sibilant/liquid groupings; 4-char code |
| **MetaSound** (Snae & Brückner 2009) | Medium | Per-syllable [initial][vowel][final] triple; variable-length code |
| **Thai–English cross-language** (Suwanvisat & Prasitjutrakul 1998) | Medium | Encodes transliterated Thai↔English names to the same code; requires both Thai and English phonetic tables |
| **HMM + trigram hybrid** | High | Uses Hidden Markov Models and phonetic trigrams; requires labelled training data — defer until ML infrastructure exists |

**FTS integration opportunity:** lk82/udom83 codes could be emitted as synonyms in
`FtsTokenizer` (alongside RTGS romanization), enabling phonetic-fuzzy full-text search
with zero schema change.

- [x] **lk82** — implemented; 12 consonant groups, 4-char code, unit tests
- [x] **udom83** — implemented alongside lk82; finer sibilant/liquid groupings, unit tests
- [x] **MetaSound** — per-syllable [initial][vowel][final] triple; variable-length code, unit tests
- [x] **FTS integration** — `.soundex(SoundexAlgorithm)` builder on `FtsTokenizer`; emits code into `synonyms` for Thai/Named tokens; 5 tests
- [x] **Thai–English cross-language Soundex** — `thai_english_soundex(word, &rom)` + `english_soundex(word)` + `sounds_like_cross_lang`; works transparently for Thai→English and English→English pairs
- [ ] **HMM + trigram hybrid** — deferred; requires ML training data

---

## Deferred / low priority

- [ ] **kham-pg token type 7 for Named** — change `Named(_) => 1` → `Named(_) => 7` in `lib.rs`; add `named` lextype in `shim.c`; requires Docker pg_regress update
- [ ] **kham-sqlite v2** — synonym expansion via `FTS5_TOKEN_COLOCATED`, normalization, stopword filtering
- [ ] **Spelling correction** — edit-distance based; requires significant ML or DP work
- [ ] **Word embeddings / semantic similarity** — requires ML inference; defer indefinitely

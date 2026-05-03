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
| v0.5.0 | `kham-pg` ts_headline support, `kham_fts_dict` custom dictionary template (phonetic + RTGS lexemes), C FFI parity (`kham-capi`), full WASM/Python feature parity, kham-web live demo site |
| v0.5.1 | WASM u32 overflow fix for large numbers, NE tag correction (ประเทศไทย PERSON→PLACE), kham-web dark mode |
| v0.6.0 | SpellChecker, KeyExtractor, spell/keyword bindings (WASM/Python/CAPI), kham-sqlite custom synonyms/dict/Windows/Android, CI pytest + wasm tests |
| v0.7.0 | Stopword suppression, Thai number normalization, udom83/MetaSound soundex dict variants, POS lexeme expansion (`pos_verb`, `pos_noun`, …), `kham_features` regress suite |
| v0.8.0 | `Token::confidence: f32`, `TokenStream` streaming iterator, `SpellChecker::correct_text` / `did_you_mean`, `RomanizationMap::romanize_sentence`, `KeyExtractor::extract_phrases`, CLI `--format` / `--confidence` / `--min-confidence` / `--romanize` |
| v0.8.2 | NE gazetteer 36,668 → 38,950 entries (thainer CC0), POS table 8,993 → 11,404 entries (UD_Thai-PUD CC-BY-SA-3.0), `wiki_freq.tsv` supplemental frequency data (CC-BY-SA-4.0, not yet loaded), import scripts |

---

## v0.6.0 — Released

| Feature | Module | Priority |
|---------|--------|----------|
| **Thai spell correction** | `spell.rs` | High — edit distance + lk82 phonetic ranking; builds on existing dict + soundex |
| **Rule-based RTGS romanization** | extend `romanizer.rs` | High — table-only (~415 entries) misses OOV words; rule engine covers all Thai |
| **NE gazetteer expansion** | `ne_th.tsv` | High — ~400 entries is too small for real-world use; target 50k+ |
| **Keyword extraction (TF-IDF)** | `keyword.rs` | Medium — TF-IDF over segmented tokens; useful for search ranking |
| **Dict merge API** | `dict.rs` | Medium — `Tokenizer::builder().dict_merge(custom)` overlay without full trie rebuild |
| **PGXN upload** | infra | High — publish `kham-pg` to pgxn.org |
| **kham-capi / kham-cli publish** | infra | Medium — publish v0.5.1 to crates.io |

- [x] **Thai spell correction** (`spell.rs`) — Levenshtein edit distance over dictionary candidates, re-ranked by lk82 phonetic similarity and TNC frequency; `SpellChecker::builtin()` + `SpellChecker::suggestions(word, max_n) -> Vec<Suggestion>`
- [x] **Rule-based RTGS romanization** — `romanize_word(word) -> String` rule engine added to `romanizer.rs`; handles leading vowels (เ แ โ ใ ไ), above/below diacritics, following vowels, thanthakat silent mark; `romanize_or_rule()` + `romanize_owned()` methods expose it; table still takes priority; 109 doctests pass
- [x] **NE gazetteer expansion** — already completed: 36,670 entries (8k PLACE, 19k PERSON, 9.6k ORG); Wikipedia titles, family names, full country list imported in prior releases
- [x] **Keyword extraction** (`keyword.rs`) — `KeyExtractor::builtin()` + `extract(text, max_n) -> Vec<Keyword>`; TF × IDF_proxy scoring (no transcendentals, `no_std` safe); IDF proxy = `(max_tnc_freq + 1) / (tnc_freq + 1)`; stopwords + single-char tokens excluded; 104 tests pass
- [x] **Dict merge API** — FtsTokenizerBuilder::dict_merge() fast overlay; no trie rebuild
- [ ] **PGXN upload** — publish `kham-pg` to pgxn.org
- [ ] **kham-capi / kham-cli publish** — publish to crates.io

---

## Active priorities

- [ ] **PGXN upload** — upload `kham_pg-0.5.0.zip` once pgxn.org account is active
- [x] **`ts_headline` support** — HEADLINE callback in `kham-pg/src/shim.c` + `lib.rs`; fills startsel/stopsel/fragdelim; marks matching QI_VAL operands; 5 regress tests

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

## TNC4 (Thai National Corpus v4) — Investigation Items

Source: <https://awirote.medium.com/thai-national-corpus-v-4-tnc4-4778ecbac05b> · Web app: <https://app.stichula.org/py/tnc4/>

**What TNC4 is:** A new web search application built by Prof. Wirote Aroonmanakun (original TNC creator) and hosted by the Sirinat Thai Language Institute (STIL), Chulalongkorn University. It is a research/internal project — **not yet a formal public corpus release** with a downloadable data package. The corpus contains 34 million words with Universal Dependencies POS tags, covering 15 genres (academic, fiction, newspaper, biography, law, etc.) and multiple domains. Corpus history: TNC1 (14M words, 2006) → TNC2 → TNC3 (public, ~100M words on arts.chula.ac.th) → TNC4 (web app only, ~34M, status unknown).

**Current blockers for any data import:**
- Raw corpus data not confirmed as downloadable (web UI only; CSV export is per-query, not bulk)
- License for TNC4 raw data not stated (existing `tnc_freq.txt` in kham is CC0 from PyThaiNLP's older TNC extract)
- UD POS tags would need mapping to kham's 13-category ORCHID scheme (ADR-003)

Two potential high-value imports if raw data becomes available under CC0/CC-BY:

- [ ] **TNC4 frequency data refresh** — TNC4's word-frequency list could replace or supplement `tnc_freq.txt` (current: 106,120 types). Better scores improve DP scorer tiebreaking (`DpScore` field 4) and spell correction ranking. **Action**: contact Prof. Wirote Aroonmanakun (awirote@gmail.com per Medium profile) or STIL to ask about data access and license.

- [ ] **TNC4 POS expansion** — 34M UD-tagged tokens far exceed the current 8,691-entry `pos_th.tsv`. If downloadable under CC-BY, map UD→kham 13-category scheme (see ADR-003) and filter low-confidence annotations (known issue: `สวย` tagged ADP/NUM in edge cases). **Action**: same contact as above; check if `arts.chula.ac.th/ling/tnc3` offers bulk POS data.

**LST20 (non-commercial, not embeddable by default):** NECTEC's LST20 (3.16M words, 288K NE spans, 16 POS tags) — <https://huggingface.co/datasets/lst-nectec/lst20>. Non-commercial restriction means it cannot be embedded in the default build; deferred.

---

## Public dataset imports (available now, no registration)

These datasets are directly downloadable today. Excluded datasets with CC-BY-NC-SA or non-commercial clauses: ORCHID full corpus, InterBEST, LST20, OpenSubtitles.

### CC0 — no attribution required (embed freely)

- [x] **`thainer-corpus-v2`** (PyThaiNLP, CC0) — 2,280 new NE entries appended to `ne_th.tsv` (403 PERSON, 793 PLACE, 1,084 ORG). Script: `scripts/import_thainer_ne.py`. Total ne_th.tsv: 36,668 → 38,950 entries. See ADR-007.

### CC-BY-SA 3.0 — attribution required; keep in separate optional TSV files

- [x] **`UD_Thai-PUD`** (Universal Dependencies, CC-BY-SA 3.0) — 2,407 new POS entries appended to `pos_th.tsv` (PROPN 884, VERB 665, NOUN 662, ADJ 134, …). Script: `scripts/import_ud_pud_pos.py`. Total pos_th.tsv: 8,993 → 11,404 entries. Segmentation sentences in `testdata/reference/ud_pud.txt` (F1 0.663, excluded from benchmark). See ADR-007.

### CC-BY-SA 4.0 — attribution required; keep in separate optional TSV files

- [x] **Thai Wikipedia word frequency** — `kham-core/data/wiki_freq.tsv` created (CC-BY-SA-4.0); 19,890 entries from 500 articles, 7,640 wiki-only new words. Script: `scripts/import_wiki_freq.py --max-articles N`. **Not yet loaded** by FreqMap — needs `FreqMap::from_two_sources` API. See ADR-007.
- [ ] **`FreqMap::from_two_sources(tnc: &[u8], wiki: &[u8])`** — load both frequency files; wiki entries supplement TNC with a minimum-token-length filter (≥ 2 Thai chars) to exclude segmentation fragments like `ร์`.

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

- [x] **kham-pg token type 7 for Named** — `Named(_) => 7` in `lib.rs`; `named` lextype in `shim.c`; `ADD MAPPING FOR named` in SQL; pg_regress test 20 verifies
- [x] **kham-pg soundex + RTGS dictionary** — `kham_fts_dict` custom template expands Thai/Named tokens to `[word, lk82_soundex, rtgs?]` at same tsvector position; fixed PG16+ lexize calling convention (arg3 is `List*`, not `bool isNull`); 9 new regress tests (28-31 + updated 8-27)
- [x] **kham-sqlite soundex** — lk82/udom83/MetaSound codes emitted as `FTS5_TOKEN_COLOCATED` tokens; default lk82; override via `tokenize='kham soundex=udom83'`; disable with `soundex=none`
- [x] **kham-sqlite stopword suppression** — `stopwords on` xCreate argument; stopword tokens skipped in `xTokenize`; default off (backward-compatible)
- [x] **kham-sqlite ngram_size** — `ngram_size N` xCreate argument; controls char n-gram size for Unknown tokens; default 3; 0 disables n-grams
- [x] **Android build** — kham-sqlite built for arm64-v8a, armeabi-v7a, x86_64, x86 via NDK in release CI; iOS static lib deferred
- [x] **Spelling correction** — shipped in v0.6.0; see v0.6.0 section above
- [ ] **Word embeddings / semantic similarity** — requires ML inference; defer indefinitely

---

## kham-tnc — Professional Thai Corpus Analysis Tool

A self-hosted web application for professional corpus linguistics research, built entirely on `kham-core`. Fills the same niche as TNC4 (Chulalongkorn) but open-source, self-hostable, and user-corpus-first (bring your own text).

### Architecture

```
kham-tnc/
├── src/
│   ├── main.rs          # axum web server entry point
│   ├── indexer.rs       # corpus ingestion: segment → POS/NE tag → write to SQLite
│   ├── corpus.rs        # corpus registry (multiple named corpora)
│   ├── kwic.rs          # KWIC search and concordance
│   ├── freq.rs          # frequency list builder
│   ├── collocate.rs     # collocation statistics (MI, logDice, t-score, LL)
│   ├── ngram.rs         # n-gram frequency and pattern search
│   ├── keyword.rs       # keyword comparison across two corpora
│   ├── query.rs         # query parser: word, POS filter, NE filter, wildcard, proximity
│   └── api.rs           # REST JSON API (mirrors web UI)
├── static/              # HTMX + Tailwind frontend (no npm build step)
│   ├── index.html
│   ├── app.js
│   └── style.css
├── Cargo.toml           # depends on kham-core, axum, rusqlite, serde
└── CLAUDE.md
```

**Storage:** SQLite — one DB per corpus. Each DB has:
- `tokens` table: `(doc_id, pos, word, pos_tag, ne_tag, char_start, char_end)`
- `docs` table: `(doc_id, filename, genre, domain, char_count, token_count)`
- FTS5 virtual table via `kham-sqlite` for full-text search

### Phase 1 — Core Analysis (MVP)

**Corpus management**
- [ ] Upload plain-text files (.txt, .csv) via web UI or CLI (`kham-tnc index corpus.txt --genre news`)
- [ ] Corpus overview: document count, token count, type count, genre/domain breakdown
- [ ] Multiple corpora — switch between them in the UI; one SQLite DB per corpus

**KWIC / Concordance**
- [ ] Search by exact word — returns N concordance lines (default 50) with left/right context
- [ ] Wildcard search: `ภาษา*` (prefix), `*ศาสตร์` (suffix), `*ภาษา*` (contains)
- [ ] Proximity search: `word1 <1-5> word2` — two words within N positions of each other
- [ ] Sort concordance by: left context, node word, right context, document, random
- [ ] Genre/domain filter on all searches
- [ ] Paginate results; export concordance as CSV

**Frequency analysis**
- [ ] Word frequency list — total, per-genre, per-domain; normalized (per million words)
- [ ] Filter by POS tag (`--pos NOUN`), NE type (`--ne PLACE`), min frequency threshold
- [ ] Dispersion score — how evenly a word is spread across documents (Juilland's D)
- [ ] Export frequency table as CSV/TSV

**Collocation analysis**
- [ ] Given a node word, compute collocates in L1–L5 / R1–R5 span (configurable)
- [ ] Statistics computed: **MI**, **logDice**, **t-score**, **log-likelihood (LL)**, **Dice**, raw freq
- [ ] Filter by collocation direction (left-only, right-only, both), minimum co-occurrence count
- [ ] Sort by any statistic; export as CSV

### Phase 2 — Linguistic Depth

**POS-aware search**
- [ ] POS constraint in query: `[pos=NOUN]`, `[pos=VERB] ของ [pos=NOUN]`
- [ ] Show POS distribution for a search term — how often tagged as VERB vs NOUN vs ADJ, etc.
- [ ] NE constraint: `[ne=PLACE]` — find all PLACE tokens; frequency by NE type

**N-gram analysis**
- [ ] Bigram and trigram frequency lists with MI / logDice scores
- [ ] Pattern search: `การ [pos=VERB]` — fixed word + POS slot
- [ ] Cluster view: all N-grams containing a given word

**Visualizations (web UI)**
- [ ] Frequency trend chart — word frequency by document order or date (if metadata available)
- [ ] Dispersion plot — dot matrix showing where a word appears in the corpus
- [ ] Genre/domain bar chart per search result
- [ ] Collocation heatmap: L5–R5 span × frequency strength

**Thai-specific features**
- [ ] Phonetic search: find all words that sound like the query (lk82 / MetaSound)
- [ ] RTGS display toggle — show romanized form alongside each token in concordance
- [ ] NE highlighting in concordance lines (Person=blue, Place=green, Org=orange)
- [ ] Compound-word boundary display: render segmentation boundaries visually (กิน|ข้าว)

### Phase 3 — Professional Features

**Multi-corpus comparison**
- [ ] Keyword analysis — compare two corpora; rank words by keyness (log-likelihood or %DIFF)
- [ ] Frequency ratio: word is N× more common in corpus A than corpus B
- [ ] Side-by-side concordance from two corpora for the same query

**Corpus statistics**
- [ ] Vocabulary growth curve (type-token ratio by sample size)
- [ ] Average word length, sentence length distributions
- [ ] Hapax legomena (words appearing exactly once) list

**REST API**
- [ ] All analysis endpoints available as JSON API (`/api/kwic`, `/api/freq`, `/api/collocate`, etc.)
- [ ] API key auth for multi-user deployments
- [ ] OpenAPI spec generated from route annotations

**Deployment**
- [ ] Single binary — `kham-tnc serve --corpus mydata.sqlite --port 8080`
- [ ] Docker image: `nickmsft/kham-tnc`
- [ ] CLI mode — all analysis runs headlessly: `kham-tnc freq --corpus my.sqlite --word ภาษา`

### Comparison with TNC4

| Feature | TNC4 (Chula) | kham-tnc |
|---|---|---|
| Corpus | Fixed (34M words) | User-supplied (any size) |
| Self-hosted | No | Yes |
| Open source | No | Yes |
| POS search | NOUN filter only | Full 13-category constraint |
| NE search | No | PERSON / PLACE / ORG filter |
| Phonetic search | No | lk82 / MetaSound |
| RTGS display | No | Yes |
| Collocate stats | MI, LL | MI, logDice, t-score, LL, Dice |
| Export | CSV | CSV, JSON, TSV |
| API | No | REST JSON |
| Multi-corpus compare | No | Yes (Phase 3) |

# ADR-005: Frequency Corpus Merge — TTC Included, Phupha Excluded

**Date:** 2026-04-25  
**Status:** Accepted  
**Deciders:** @preedep

---

## Context

kham-core's DP segmentation scorer uses `FreqMap` (`src/freq.rs`) as the **fourth tiebreaker** in the 4-field `DpScore`:

1. Minimise unknowns
2. Minimise token count (compound-first; matches PyThaiNLP newmm)
3. Maximise dict-word matches
4. **Maximise TNC frequency** ← FreqMap contribution

> **Note (2026-04-25):** Priority 2 was changed from "Maximise dict-word matches" to "Minimise token count". Splitting a compound into two known words scores more dict matches than keeping it whole, causing spurious splits. Placing token-count minimisation above dict-match maximisation fixes this and raises sentence-level agreement with PyThaiNLP newmm from 2.6% → 94.9% (F1: 0.418 → 0.975).

The built-in frequency data comes from `kham-core/data/tnc_freq.txt` (Thai National Corpus, CC0, 106,120 word types). Two additional CC0 corpora were available for evaluation:

| Corpus | File | Entries | Max count | CC0? |
|---|---|---|---|---|
| TNC | `tnc_freq.txt` | 106,120 | 818,364 | ✅ |
| TTC | `ttc_freq.txt` | 19,493 | 63,126 | ✅ |
| Phupha | `phupha_word_freqs.txt` | 62,264 | 1,119,315,948 | ✅ |

### TTC analysis

Thai Textbook Corpus word frequencies. Clean word-level data, tab-separated `word\tcount`. Overlap with TNC:

| Metric | Count |
|---|---|
| Words in both TNC and TTC | 17,083 |
| TTC-only (not in TNC) | 2,410 |

TTC-only words are dominated by Thai reduplicated forms (`ต่างๆ`, `อื่นๆ`, `จริงๆ`, `เด็กๆ`) and compound expressions that the TNC did not cover. These are genuine high-value additions for the DP scorer.

### Phupha analysis

**Character-level data — unsuitable for word-level FreqMap.**

Inspection of `phupha_word_freqs.txt` reveals it is not a word frequency file:

- Top 10 entries are all single Thai consonants (`น`, `ร`, `อ`, `ก`, `ม`, …) with counts in the **billions** (1.1 billion for `น`)
- The first multi-character entry is the literal English word `"word"` at 335 million
- Scale mismatch: TNC max is ~818k; Phupha max is ~1.1 billion (1,370× larger)

Adding Phupha to the FreqMap would:
1. Assign astronomically high scores to single Thai consonants, making the DP scorer prefer bare-consonant tokens over whole-word matches — **catastrophic segmentation regression**
2. Pollute the word-level frequency space with character-level statistics

---

## Decision

### 1. Merge TTC into `tnc_freq.txt` — TTC-only words appended

Strategy: **append TTC-only words** (not already in TNC) to the bottom of `tnc_freq.txt` with their TTC counts. Words that exist in both corpora keep their TNC count unchanged.

Rationale:
- TNC is the authoritative large-scale corpus (106k types, ~818k peak count). Its counts correctly reflect relative word importance for segmentation.
- For words only in TTC, any non-zero frequency is better than 0 (the default for unknown words). TTC counts provide the non-zero tiebreaker signal.
- Raw sum or normalization would be more complex without meaningfully improving segmentation quality — both corpora rank common words similarly; the only gain is in new vocabulary, not re-weighting existing entries.
- **Zero code change** — `FreqMap::from_tsv` already handles duplicate keys (last duplicate wins); appending TTC-only entries requires no changes to `src/freq.rs`.

Result: 2,410 new word types added. Top new entries: `ต่างๆ` (3,608), `อื่นๆ` (1,525), `เด็กๆ` (1,237), `จริงๆ` (1,153).

### 2. Exclude Phupha entirely

`phupha_word_freqs.txt` contains character-level (not word-level) frequency data. Including it in the word-level `FreqMap` would corrupt DP scorer tiebreaking and cause severe segmentation regressions. It is excluded permanently.

---

## Consequences

### Positive

- 2,410 reduplicated and compound Thai forms gain non-zero frequency scores, improving DP tiebreaking for these patterns
- No API change — `FreqMap` loading is unchanged; data is self-contained in `tnc_freq.txt`
- Both corpora are CC0 — no attribution obligation

### Negative / Residual risks

- TTC-only entries have counts on a different scale than TNC (TTC max ~63k vs TNC ~818k). Words unique to TTC will score lower than common TNC words — correct behaviour, but means TTC words are at a slight relative disadvantage if they also appear (with misspelling/variant) in TNC
- Low-frequency TTC entries (count = 1) add marginal signal; they prevent zero-score ties but do not meaningfully discriminate between rare words

---

## Alternatives Considered

| Option | Reason rejected |
|---|---|
| Normalize then sum (relative freq) | Requires float intermediates; FreqMap uses `u32` counts; complexity not justified by the tiebreaker role |
| Max across corpora | Same result as append for TTC-only words; for overlap words, TNC always wins anyway (818k > 63k) |
| Include Phupha | Character-level data causes catastrophic segmentation regression |
| Separate `ttc_freq.txt` builtin | Would require API change (`FreqMap::from_two_sources` etc.); single merged file is simpler |
| Scale TTC counts to TNC range then sum | Marginal improvement for overlap words; TNC ordering already correct; complexity not justified |

---

## References

- PyThaiNLP corpus license: <https://github.com/PyThaiNLP/pythainlp/blob/main/pythainlp/corpus/corpus_license.md>
- TTC and Phupha: CC0-1.0, PyThaiNLP
- kham-core FreqMap: `kham-core/src/freq.rs`
- kham-core DP scorer: `kham-core/src/segmenter.rs` — `DpScore`

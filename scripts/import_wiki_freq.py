#!/usr/bin/env python3
"""
Build a Thai word frequency list from pythainlp/thai-wiki-dataset-v3 (CC-BY-SA 4.0)
using kham for segmentation. Writes kham-core/data/wiki_freq.tsv.

This file is kept SEPARATE from tnc_freq.txt (CC0) because Wikipedia text is
CC-BY-SA 4.0. Do not merge the two files.

To use wiki_freq.tsv in kham-core FreqMap, load it alongside tnc_freq.txt:
  FreqMap::from_two_sources(TNC_BYTES, WIKI_BYTES)  [future API; see roadmap]

Usage:
  source .venv/bin/activate
  python scripts/import_wiki_freq.py [--max-articles N] [--dry-run]

Options:
  --max-articles N   Process at most N articles (default: all ~196k)
  --dry-run          Print stats only, do not write
"""

import argparse
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).parent.parent
WIKI_FREQ_TSV = ROOT / "kham-core/data/wiki_freq.tsv"
WORDS_TXT = ROOT / "kham-core/data/words_th.txt"
TNC_FREQ = ROOT / "kham-core/data/tnc_freq.txt"

THAI_WORD_RE = re.compile(r"^[฀-๿]{2,}$")


def load_tnc_words(path: Path) -> set[str]:
    words = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            parts = line.split("\t")
            if parts:
                words.add(parts[0])
    return words


def main():
    parser = argparse.ArgumentParser(description="Build wiki word frequency from Thai Wikipedia")
    parser.add_argument("--max-articles", type=int, default=0,
                        help="Max articles to process (0 = all)")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    print("Loading kham segmenter …")
    try:
        import kham
        segment_fn = kham.segment
    except ImportError:
        print("ERROR: kham not found. Run: maturin develop -m kham-python/Cargo.toml", file=sys.stderr)
        sys.exit(1)

    print("Loading TNC word set (to track overlap) …")
    tnc_words = load_tnc_words(TNC_FREQ)
    print(f"  {len(tnc_words):,} TNC entries")

    print("Streaming pythainlp/thai-wiki-dataset-v3 (CC-BY-SA 4.0) …")
    from datasets import load_dataset
    ds = load_dataset("pythainlp/thai-wiki-dataset-v3", split="train", streaming=True, trust_remote_code=True)

    freq: Counter = Counter()
    n_articles = 0
    n_tokens = 0
    limit = args.max_articles if args.max_articles > 0 else None

    import sys
    log_interval = max(1, (limit or 10000) // 20)

    for row in ds:
        text = row.get("text") or row.get("content") or ""
        if not text:
            continue

        tokens = segment_fn(text)
        for tok in tokens:
            if THAI_WORD_RE.match(tok):
                freq[tok] += 1
                n_tokens += 1

        n_articles += 1
        if n_articles % log_interval == 0:
            print(f"  {n_articles:,} articles, {n_tokens:,} tokens, {len(freq):,} unique words …", flush=True)

        if limit and n_articles >= limit:
            break

    print(f"\nDone: {n_articles:,} articles, {n_tokens:,} Thai tokens, {len(freq):,} unique word types")

    tnc_overlap = sum(1 for w in freq if w in tnc_words)
    wiki_only = len(freq) - tnc_overlap
    print(f"  Overlap with tnc_freq.txt: {tnc_overlap:,}")
    print(f"  Wiki-only words:           {wiki_only:,}")

    if args.dry_run:
        top = freq.most_common(20)
        print("\nTop 20 words by frequency:")
        for w, c in top:
            marker = " (in TNC)" if w in tnc_words else ""
            print(f"  {w}\t{c}{marker}")
        print("\n[dry-run] No file written.")
        return

    header = (
        "# Thai word frequencies extracted from Thai Wikipedia\n"
        "# Source: https://huggingface.co/datasets/pythainlp/thai-wiki-dataset-v3\n"
        "# License: CC BY-SA 4.0 — https://creativecommons.org/licenses/by-sa/4.0/\n"
        "# Segmented with kham (https://github.com/preedep/kham)\n"
        f"# Articles processed: {n_articles:,}\n"
        "# Format: word<TAB>count  (sorted by count descending)\n"
        "# NOTE: Keep this file SEPARATE from tnc_freq.txt (which is CC0).\n"
        "#       Do not merge — different licenses.\n"
    )

    lines = [f"{w}\t{c}" for w, c in freq.most_common()]
    WIKI_FREQ_TSV.write_text(header + "\n".join(lines) + "\n", encoding="utf-8")
    print(f"\nWrote {len(freq):,} entries to {WIKI_FREQ_TSV}")


if __name__ == "__main__":
    main()

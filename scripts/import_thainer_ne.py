#!/usr/bin/env python3
"""
Import named entities from pythainlp/thainer-corpus-v2 (CC0) into ne_th.tsv.

NE type mapping:
  PERSON       → PERSON
  LOCATION     → PLACE
  ORGANIZATION → ORG
  All others (DATE, TIME, MONEY, FACILITY, URL, …) → skipped

Filtering (same strategy as ADR-001):
  - Skip entities that appear in words_th.txt (common vocabulary, not NE)
  - Skip entities already in ne_th.tsv
  - Skip entities with no Thai characters
  - Skip single-character entities
  - Skip entities containing whitespace or digits only

Usage:
  source .venv/bin/activate
  python scripts/import_thainer_ne.py [--dry-run]
"""

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
NE_TSV = ROOT / "kham-core/data/ne_th.tsv"
WORDS_TXT = ROOT / "kham-core/data/words_th.txt"

NE_TYPE_MAP = {
    "PERSON": "PERSON",
    "LOCATION": "PLACE",
    "ORGANIZATION": "ORG",
}

THAI_RE = re.compile(r"[฀-๿]")
INVISIBLE = re.compile(r"[﻿​‌‍­]")


def load_wordset(path: Path) -> set[str]:
    words = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            words.add(line)
    return words


def load_existing_ne(path: Path) -> set[str]:
    entities = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            parts = line.split("\t")
            if parts:
                entities.add(parts[0])
    return entities


def extract_entities(dataset) -> dict[str, set[str]]:
    """Extract NE surface strings grouped by kham NE type."""
    collected: dict[str, set[str]] = {"PERSON": set(), "PLACE": set(), "ORG": set()}

    for row in dataset:
        words = row["words"]
        ner_ids = row["ner"]
        label_names = dataset.features["ner"].feature.names

        i = 0
        while i < len(words):
            label = label_names[ner_ids[i]]
            if label.startswith("B-"):
                ne_raw = label[2:]
                kham_type = NE_TYPE_MAP.get(ne_raw)
                if kham_type:
                    span_tokens = [words[i]]
                    i += 1
                    cont = "I-" + ne_raw
                    while i < len(words) and label_names[ner_ids[i]] == cont:
                        span_tokens.append(words[i])
                        i += 1
                    surface = INVISIBLE.sub("", "".join(span_tokens).strip())
                    if surface:
                        collected[kham_type].add(surface)
                    continue
            i += 1

    return collected


THAI_START_RE = re.compile(r"^[฀-๿]")
NON_THAI_CLEAN_RE = re.compile(r"^[^฀-๿]+|[^฀-๿\w.]+$")


def is_valid(word: str) -> bool:
    if len(word) <= 1:
        return False
    if not THAI_RE.search(word):
        return False
    if not THAI_START_RE.match(word):
        return False
    if " " in word or "\t" in word:
        return False
    return True


def main():
    parser = argparse.ArgumentParser(description="Import thainer NE into ne_th.tsv")
    parser.add_argument("--dry-run", action="store_true", help="Print stats only, do not write")
    args = parser.parse_args()

    print("Loading words_th.txt …")
    common_words = load_wordset(WORDS_TXT)
    print(f"  {len(common_words):,} common words loaded")

    print("Loading existing ne_th.tsv …")
    existing_ne = load_existing_ne(NE_TSV)
    print(f"  {len(existing_ne):,} existing NE entries")

    print("Downloading pythainlp/thainer-corpus-v2 (CC0) …")
    from datasets import load_dataset, concatenate_datasets

    splits = load_dataset("pythainlp/thainer-corpus-v2", trust_remote_code=True)
    all_data = concatenate_datasets([splits["train"], splits["validation"], splits["test"]])
    print(f"  {len(all_data):,} rows across all splits")

    print("Extracting NE spans …")
    collected = extract_entities(all_data)

    new_entries: dict[str, list[str]] = {"PERSON": [], "PLACE": [], "ORG": []}
    skipped_common = 0
    skipped_existing = 0
    skipped_invalid = 0

    for kham_type, entities in collected.items():
        for word in sorted(entities):
            if not is_valid(word):
                skipped_invalid += 1
            elif word in existing_ne:
                skipped_existing += 1
            elif word in common_words:
                skipped_common += 1
            else:
                new_entries[kham_type].append(word)

    total_new = sum(len(v) for v in new_entries.values())
    print(f"\nResults:")
    print(f"  New PERSON: {len(new_entries['PERSON']):,}")
    print(f"  New PLACE:  {len(new_entries['PLACE']):,}")
    print(f"  New ORG:    {len(new_entries['ORG']):,}")
    print(f"  Total new:  {total_new:,}")
    print(f"  Skipped (already in ne_th.tsv): {skipped_existing:,}")
    print(f"  Skipped (in words_th.txt):       {skipped_common:,}")
    print(f"  Skipped (invalid):               {skipped_invalid:,}")

    if args.dry_run:
        print("\n[dry-run] No changes written.")
        return

    if total_new == 0:
        print("Nothing to add.")
        return

    append_block = "\n# ── thainer-corpus-v2 (CC0, PyThaiNLP) ─────────────────────────────────────\n"
    append_block += "# Source: https://huggingface.co/datasets/pythainlp/thainer-corpus-v2\n"
    append_block += "# License: CC0-1.0\n"

    for section_tag, section_label in [("PERSON", "PERSON"), ("PLACE", "LOCATION"), ("ORG", "ORGANIZATION")]:
        words = new_entries[section_tag]
        if not words:
            continue
        append_block += f"\n# ── thainer: {section_label} ──\n"
        for w in words:
            append_block += f"{w}\t{section_tag}\n"

    with open(NE_TSV, "a", encoding="utf-8") as f:
        f.write(append_block)

    print(f"\nAppended {total_new:,} new entries to {NE_TSV}")


if __name__ == "__main__":
    main()

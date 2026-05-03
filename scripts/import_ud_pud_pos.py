#!/usr/bin/env python3
"""
Import POS tags and segmentation test cases from UD_Thai-PUD (CC-BY-SA 3.0).

Source: https://github.com/UniversalDependencies/UD_Thai-PUD
License: CC BY-SA 3.0 — attribution required; entries stored in a separate
         CC-BY-SA-3.0-marked section of pos_th.tsv.

UD UPOS → kham 13-category mapping:
  NOUN  → NOUN    VERB  → VERB    ADJ   → ADJ     ADV   → ADV
  PROPN → PROPN   PRON  → PRON    NUM   → NUM      AUX   → AUX
  DET   → DET     CCONJ → CONJ    SCONJ → CONJ     ADP   → PREP
  PART  → PART    INTJ  → PART
  PUNCT / SYM / X / _ → skipped

Filtering (same rules as ADR-003):
  - Skip entries with no Thai Unicode characters
  - Skip entries containing spaces (multi-word)
  - Skip single-character entries
  - Skip entries already in pos_th.tsv

Also writes valid segmentation sentences to kham-core/testdata/ud_pud.txt.

Usage:
  source .venv/bin/activate
  python scripts/import_ud_pud_pos.py [--dry-run]
"""

import argparse
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).parent.parent
POS_TSV = ROOT / "kham-core/data/pos_th.tsv"
TESTDATA = ROOT / "kham-core/testdata/ud_pud.txt"

UPOS_MAP = {
    "NOUN": "NOUN", "VERB": "VERB", "ADJ": "ADJ", "ADV": "ADV",
    "PROPN": "PROPN", "PRON": "PRON", "NUM": "NUM", "AUX": "AUX",
    "DET": "DET", "CCONJ": "CONJ", "SCONJ": "CONJ", "ADP": "PREP",
    "PART": "PART", "INTJ": "PART",
}
SKIP_UPOS = {"PUNCT", "SYM", "X", "_"}

THAI_RE = re.compile(r"[฀-๿]")
DIGIT_RE = re.compile(r"\d")


def load_existing_pos(path: Path) -> set[str]:
    words = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            parts = line.split("\t")
            if parts:
                words.add(parts[0])
    return words


def is_valid(word: str) -> bool:
    if len(word) <= 1:
        return False
    if not THAI_RE.search(word):
        return False
    if " " in word or "\t" in word:
        return False
    if DIGIT_RE.search(word):
        return False
    return True


def parse_conllu(path: Path):
    """Yield (sentence_id, [(form, upos), ...]) for each sentence."""
    sentences = []
    current = []
    sent_id = None

    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("# sent_id"):
            sent_id = line.split("=", 1)[-1].strip()
        elif line.startswith("#") or not line.strip():
            if current:
                sentences.append((sent_id, current))
                current = []
                sent_id = None
        else:
            parts = line.split("\t")
            if len(parts) < 4:
                continue
            token_id, form, _lemma, upos = parts[0], parts[1], parts[2], parts[3]
            if "-" in token_id or "." in token_id:
                continue  # skip multi-word tokens and empty nodes
            current.append((form, upos))

    if current:
        sentences.append((sent_id, current))

    return sentences


def clone_ud_pud(tmpdir: str) -> Path:
    dest = Path(tmpdir) / "UD_Thai-PUD"
    print("Cloning UD_Thai-PUD …")
    subprocess.run(
        ["git", "clone", "--depth=1", "--quiet",
         "https://github.com/UniversalDependencies/UD_Thai-PUD.git", str(dest)],
        check=True,
    )
    return dest


def build_testdata_line(tokens: list[tuple[str, str]]) -> str | None:
    """Build a testdata line input|tok1|tok2|... from Thai-only sentence tokens."""
    thai_forms = [f for f, upos in tokens if THAI_RE.search(f) and upos not in SKIP_UPOS]
    if len(thai_forms) < 2:
        return None
    joined = "".join(thai_forms)
    return joined + "|" + "|".join(thai_forms)


def main():
    parser = argparse.ArgumentParser(description="Import UD_Thai-PUD POS data")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    print("Loading existing pos_th.tsv …")
    existing = load_existing_pos(POS_TSV)
    print(f"  {len(existing):,} existing entries")

    with tempfile.TemporaryDirectory() as tmpdir:
        repo = clone_ud_pud(tmpdir)
        conllu_files = list(repo.glob("*.conllu"))
        print(f"  Found {len(conllu_files)} CoNLL-U file(s)")

        all_sentences = []
        for cf in conllu_files:
            all_sentences.extend(parse_conllu(cf))

    print(f"  {len(all_sentences):,} sentences parsed")

    new_entries: dict[str, list[str]] = {tag: [] for tag in sorted(set(UPOS_MAP.values()))}
    seen_words: set[str] = set()
    skipped_invalid = 0
    skipped_existing = 0
    skipped_upos = 0

    testdata_lines: list[str] = []

    for _sent_id, tokens in all_sentences:
        td = build_testdata_line(tokens)
        if td:
            testdata_lines.append(td)

        for form, upos in tokens:
            if upos in SKIP_UPOS:
                skipped_upos += 1
                continue
            kham_tag = UPOS_MAP.get(upos)
            if not kham_tag:
                skipped_upos += 1
                continue
            if not is_valid(form):
                skipped_invalid += 1
                continue
            if form in existing or form in seen_words:
                skipped_existing += 1
                continue
            seen_words.add(form)
            new_entries[kham_tag].append(form)

    total_new_pos = sum(len(v) for v in new_entries.values())
    print(f"\nPOS results:")
    for tag, words in sorted(new_entries.items()):
        if words:
            print(f"  {tag}: {len(words):,}")
    print(f"  Total new POS entries: {total_new_pos:,}")
    print(f"  Skipped (existing):    {skipped_existing:,}")
    print(f"  Skipped (invalid):     {skipped_invalid:,}")
    print(f"  Skipped (PUNCT/SYM/X): {skipped_upos:,}")
    print(f"\nTestdata: {len(testdata_lines):,} sentences → {TESTDATA.name}")

    if args.dry_run:
        print("\n[dry-run] No changes written.")
        return

    if total_new_pos > 0:
        block = "\n# ── UD_Thai-PUD (CC-BY-SA 3.0, Universal Dependencies) ─────────────────────\n"
        block += "# Source: https://github.com/UniversalDependencies/UD_Thai-PUD\n"
        block += "# License: CC BY-SA 3.0 — https://creativecommons.org/licenses/by-sa/3.0/\n"
        block += "# UPOS → kham mapping: see doc/adr-003-orchid-pos-tag-mapping.md\n"

        for tag, words in sorted(new_entries.items()):
            if not words:
                continue
            block += f"\n# ── UD: {tag} ──\n"
            for w in sorted(words):
                block += f"{w}\t{tag}\n"

        with open(POS_TSV, "a", encoding="utf-8") as f:
            f.write(block)
        print(f"Appended {total_new_pos:,} new entries to {POS_TSV}")

    if testdata_lines:
        header = (
            "# UD_Thai-PUD segmentation test cases (CC-BY-SA 3.0, Universal Dependencies)\n"
            "# Source: https://github.com/UniversalDependencies/UD_Thai-PUD\n"
            "# Format: input|tok1|tok2|...\n"
            "# Note: tokens are Thai script only; non-Thai tokens stripped per sentence.\n"
        )
        TESTDATA.write_text(header + "\n".join(testdata_lines) + "\n", encoding="utf-8")
        print(f"Wrote {len(testdata_lines):,} test cases to {TESTDATA}")


if __name__ == "__main__":
    main()

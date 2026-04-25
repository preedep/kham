#!/usr/bin/env python3
"""
Compare kham and PyThaiNLP word_tokenize(engine='newmm') on a shared sentence set.

Finds disagreements between the two systems — useful for discovering dictionary
gaps and segmentation bugs in kham-core. kham and PyThaiNLP both use the newmm
algorithm, so large divergences indicate genuine bugs or dictionary differences,
not algorithmic noise.

Requirements
────────────
    pip install pythainlp

    # kham backend — pick one:
    #   A) Python bindings (fastest):
    #      maturin develop -m kham-python/Cargo.toml
    #   B) pre-built binary (automatic, no extra steps):
    #      cargo build -p kham-cli          (debug)
    #      cargo build -p kham-cli --release (faster)
    #   C) cargo run (slowest, always works):
    #      automatic fallback if no binary found

Usage
─────
    python scripts/compare_pythainlp.py
    python scripts/compare_pythainlp.py --sentences path/to/sentences.txt
    python scripts/compare_pythainlp.py --show-all
    python scripts/compare_pythainlp.py --export-testdata           # disagreements → testdata format
    python scripts/compare_pythainlp.py --export-testdata --agreed  # agreed cases (high confidence)
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

# ── Known PyThaiNLP errors ────────────────────────────────────────────────────
#
# Cases where kham is linguistically correct but PyThaiNLP newmm disagrees.
# Root cause: both paths tie on token count and dict-word count; the winner is
# decided by TNC frequency score, and PyThaiNLP's frequency table differs from
# kham's, tipping the tie the wrong way.
#
# Verified 2026-04-25 via TNC frequency lookup.

KNOWN_PYTHAINLP_ERRORS: dict[str, str] = {
    "ซื้อหนังสือสามเล่มจากร้าน": (
        "PyThaiNLP: จา|กร้าน — kham: จาก|ร้าน. "
        "Both paths are 2 tokens, 2 dict words; TNC freq decides. "
        "จาก=174,522 + ร้าน=13,560 vs จา=2,288 + กร้าน=142. "
        "kham is correct; จา (archaic vow) + กร้าน (calloused skin) is meaningless here."
    ),
    "รัฐบาลไทยประกาศมาตรการควบคุมโรคระบาด": (
        "PyThaiNLP: มาตร|การควบคุม — kham: มาตรการ|ควบคุม. "
        "Both paths are 2 tokens, 2 dict words; TNC freq decides. "
        "มาตรการ=4,274 + ควบคุม=11,310 vs มาตร=646 + การควบคุม=0 (zero TNC hits). "
        "kham is correct; มาตรการ (measures/policy) is the standard government term here."
    ),
}

# ── Built-in sentence set ─────────────────────────────────────────────────────
#
# Covers: common vocabulary, ambiguous compounds, named entities, mixed script,
# loanwords, numbers, classifiers, longer sentences.

BUILTIN_SENTENCES: list[str] = [
    # Common daily life
    "กินข้าวกับปลา",
    "วันนี้อากาศดีมาก",
    "ฉันไปตลาดซื้อของสด",
    "เขาเดินทางไปทำงานทุกวัน",
    "แม่ค้าขายผักในตลาดสด",
    "นักเรียนไปโรงเรียนตอนเช้า",
    "พ่อแม่รักลูกมากที่สุด",
    "หมาสองตัวนอนอยู่ใต้โต๊ะ",
    "ซื้อหนังสือสามเล่มจากร้าน",
    "ดื่มน้ำเย็นๆในวันร้อน",
    # Potentially ambiguous compounds
    "ตากลม",
    "นมแมว",
    "ความรักของแม่",
    "สถานการณ์ในประเทศ",
    "เดินทางไกลมาก",
    "พูดคุยกันอย่างสนุกสนาน",
    "ประชาชนออกมาประท้วงรัฐบาล",
    "หัวใจเต้นแรงมาก",
    "ตาดีกว่าหู",
    # Named entities
    "ทักษิณเดินทางไปไทย",
    "กรุงเทพมหานครเป็นเมืองหลวงของไทย",
    "มหาวิทยาลัยจุฬาลงกรณ์อยู่ในกรุงเทพ",
    "นายกรัฐมนตรีประกาศนโยบายใหม่",
    "ธนาคารแห่งประเทศไทยปรับอัตราดอกเบี้ย",
    # Mixed script and numbers
    "ธนาคาร100แห่งทั่วประเทศ",
    "ราคา99บาทต่อชิ้น",
    "โทรศัพท์iPhoneรุ่นใหม่",
    "COVID19ระบาดในหลายประเทศ",
    "สินค้าลด50เปอร์เซ็นต์",
    # Loanwords
    "คอมพิวเตอร์ราคาแพงมาก",
    "โทรศัพท์มือถือรุ่นใหม่",
    "อินเทอร์เน็ตความเร็วสูง",
    "ช็อปปิ้งออนไลน์สะดวกมาก",
    "แอปพลิเคชันใหม่น่าใช้",
    # Longer sentences
    "รัฐบาลไทยประกาศมาตรการควบคุมโรคระบาด",
    "นักวิทยาศาสตร์ค้นพบดาวเคราะห์ดวงใหม่",
    "เด็กนักเรียนได้รับทุนการศึกษาจากรัฐบาล",
    "ราคาน้ำมันปรับตัวสูงขึ้นทุกสัปดาห์",
    "สภาพอากาศเปลี่ยนแปลงอย่างรวดเร็ว",
]

# ── kham backend ──────────────────────────────────────────────────────────────

def _find_kham_cmd() -> list[str]:
    """Return the command prefix to invoke the kham CLI."""
    if shutil.which("kham"):
        return ["kham"]
    for p in ["target/release/kham", "target/debug/kham"]:
        if Path(p).exists():
            return [p]
    return ["cargo", "run", "-q", "-p", "kham-cli", "--"]


def _build_kham_segment():
    """Return (segment_fn, backend_label).  Prefers Python bindings; falls back to CLI."""
    try:
        import kham as _kham_py  # type: ignore[import]
        label = "kham-python (bindings)"

        def segment(text: str) -> list[str]:
            return [t for t in _kham_py.segment(text) if t and not t.isspace()]

    except ImportError:
        cmd = _find_kham_cmd()
        label = f"kham CLI ({' '.join(cmd)})"

        def segment(text: str) -> list[str]:
            try:
                r = subprocess.run(
                    [*cmd, "--sep", "\n", text],
                    capture_output=True, text=True, check=True,
                )
                return [t for t in r.stdout.splitlines() if t.strip()]
            except subprocess.CalledProcessError as exc:
                sys.exit(f"kham error: {exc.stderr.strip()}")

    return segment, label


# ── comparison logic ──────────────────────────────────────────────────────────

def _token_spans(text: str, tokens: list[str]) -> set[tuple[int, int]]:
    """Build (char_start, char_end) spans for tokens, skipping whitespace gaps."""
    chars = list(text)
    spans: set[tuple[int, int]] = set()
    pos = 0
    for tok in tokens:
        while pos < len(chars) and chars[pos].isspace():
            pos += 1
        end = pos + len(tok)
        spans.add((pos, end))
        pos = end
    return spans


@dataclass
class CmpResult:
    sentence: str
    kham: list[str]
    pythainlp: list[str]

    @property
    def agreed(self) -> bool:
        return self.kham == self.pythainlp

    def _spans(self):
        gold = _token_spans(self.sentence, self.pythainlp)
        sys_ = _token_spans(self.sentence, self.kham)
        tp = len(gold & sys_)
        fp = len(sys_ - gold)
        fn = len(gold - sys_)
        return tp, fp, fn

    @property
    def f1(self) -> float:
        tp, fp, fn = self._spans()
        p = tp / (tp + fp) if (tp + fp) else 1.0
        r = tp / (tp + fn) if (tp + fn) else 1.0
        return 2 * p * r / (p + r) if (p + r) else 0.0


# ── CLI ───────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Compare kham vs PyThaiNLP (newmm) tokenization.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--sentences", "-s", metavar="FILE",
        help="Plain-text file of sentences (one per line; # = comment). "
             "Defaults to the built-in set of %(default)s sentences.",
    )
    parser.add_argument(
        "--show-all", action="store_true",
        help="Show every sentence, not just disagreements.",
    )
    parser.add_argument(
        "--export-testdata", action="store_true",
        help="Print results in input|tok1|tok2|… format for testdata import "
             "(disagreements by default; use --agreed for high-confidence cases).",
    )
    parser.add_argument(
        "--agreed", action="store_true",
        help="With --export-testdata: export agreed cases instead of disagreements.",
    )
    args = parser.parse_args()

    # ── load sentences ──
    if args.sentences:
        p = Path(args.sentences)
        if not p.exists():
            sys.exit(f"File not found: {p}")
        sentences = [
            line.strip()
            for line in p.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.strip().startswith("#")
        ]
    else:
        sentences = BUILTIN_SENTENCES

    # ── load pythainlp ──
    try:
        from pythainlp.tokenize import word_tokenize  # type: ignore[import]
    except ImportError:
        sys.exit(
            "pythainlp not found.\n"
            "Install with:  pip install pythainlp\n"
            "or inside the project venv:  source .venv/bin/activate && pip install pythainlp"
        )

    kham_segment, kham_label = _build_kham_segment()

    if not args.export_testdata:
        print(f"kham       : {kham_label}")
        print(f"PyThaiNLP  : word_tokenize(engine='newmm')")
        print(f"Sentences  : {len(sentences)}")
        print()

    # ── run comparison ──
    results: list[CmpResult] = []
    for sent in sentences:
        kham_toks = kham_segment(sent)
        pthai_toks = [
            t for t in word_tokenize(sent, engine="newmm")
            if t and not t.isspace()
        ]
        results.append(CmpResult(sent, kham_toks, pthai_toks))

    # ── export mode ──
    if args.export_testdata:
        export = [r for r in results if r.agreed] if args.agreed else [r for r in results if not r.agreed]
        tag = "AGREED (high-confidence)" if args.agreed else "DISAGREED — review manually before adding"
        print(f"# {tag}")
        print(f"# kham: {kham_label} | PyThaiNLP: newmm")
        print(f"# Format: input|tok1|tok2|…")
        print()
        for r in export:
            if not args.agreed:
                print(f"# kham      : {' | '.join(r.kham)}")
                print(f"# pythainlp : {' | '.join(r.pythainlp)}")
            print(f"{r.sentence}|{'|'.join(r.kham)}")
            if not args.agreed:
                print()
        return

    # ── human-readable report ──
    disagreements = [r for r in results if not r.agreed]
    genuine_diffs = [r for r in disagreements if r.sentence not in KNOWN_PYTHAINLP_ERRORS]
    to_show = results if (args.show_all or not disagreements) else disagreements

    for r in to_show:
        if r.agreed:
            tag = "[OK     ]"
        elif r.sentence in KNOWN_PYTHAINLP_ERRORS:
            tag = "[KHAM-OK]"
        else:
            tag = "[DIFF   ]"
        print(f"{tag} {r.sentence!r}")
        if not r.agreed:
            print(f"         kham      : {' | '.join(r.kham)}")
            print(f"         pythainlp : {' | '.join(r.pythainlp)}")
            if r.sentence in KNOWN_PYTHAINLP_ERRORS:
                print(f"         note      : {KNOWN_PYTHAINLP_ERRORS[r.sentence]}")
            else:
                print(f"         F1 vs PyThaiNLP: {r.f1:.3f}")
        print()

    # ── aggregate stats ──
    agreed_n = sum(1 for r in results if r.agreed)
    known_err_n = sum(1 for r in results if r.sentence in KNOWN_PYTHAINLP_ERRORS and not r.agreed)
    total_tp = sum(r._spans()[0] for r in results)
    total_fp = sum(r._spans()[1] for r in results)
    total_fn = sum(r._spans()[2] for r in results)
    micro_p = total_tp / (total_tp + total_fp) if (total_tp + total_fp) else 1.0
    micro_r = total_tp / (total_tp + total_fn) if (total_tp + total_fn) else 1.0
    micro_f1 = 2 * micro_p * micro_r / (micro_p + micro_r) if (micro_p + micro_r) else 0.0

    print("─" * 64)
    print(f"Sentences        : {len(results)}")
    print(f"Agreed           : {agreed_n}/{len(results)}  ({100*agreed_n/len(results):.1f}%)")
    if known_err_n:
        print(f"kham correct (PyThaiNLP wrong) : {known_err_n}  (see KNOWN_PYTHAINLP_ERRORS)")
    print(f"Genuine diffs    : {len(genuine_diffs)}/{len(results)}")
    print(f"Micro P/R/F1 (PyThaiNLP as reference) : {micro_p:.3f} / {micro_r:.3f} / {micro_f1:.3f}")
    print()
    if genuine_diffs:
        print("Next steps:")
        print("  python scripts/compare_pythainlp.py --export-testdata          # review disagreements")
        print("  python scripts/compare_pythainlp.py --export-testdata --agreed # add confident cases")


if __name__ == "__main__":
    main()

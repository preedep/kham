#!/usr/bin/env python3
"""
Prepare a test corpus for kham-tnc.

Downloads a sample of Thai Wikipedia articles (CC-BY-SA 4.0) from
pythainlp/thai-wiki-dataset-v3, saves each article as a .txt file,
then indexes them into a SQLite corpus using the kham-tnc binary.

Usage:
  source .venv/bin/activate
  python scripts/prepare_tnc_testdata.py [--articles N] [--corpus PATH] [--out-dir PATH]

Options:
  --articles N    Number of articles to download (default: 500)
  --corpus PATH   SQLite output path (default: tnc_test.sqlite)
  --out-dir PATH  Directory to save .txt files (default: /tmp/tnc_testdata)
  --skip-index    Download only, do not run the indexer
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
BINARY = ROOT / "target" / "release" / "kham-tnc"


def build_binary():
    print("Building kham-tnc (release)…")
    r = subprocess.run(
        ["cargo", "build", "-p", "kham-tnc", "--release"],
        cwd=ROOT,
    )
    if r.returncode != 0:
        print("ERROR: cargo build failed", file=sys.stderr)
        sys.exit(1)
    print(f"  Binary: {BINARY}\n")


def download_articles(n: int, out_dir: Path) -> list[Path]:
    print(f"Streaming pythainlp/thai-wiki-dataset-v3 (CC-BY-SA 4.0) — {n} articles…")
    try:
        from datasets import load_dataset
    except ImportError:
        print("ERROR: `datasets` not found. Run: pip install datasets", file=sys.stderr)
        sys.exit(1)

    out_dir.mkdir(parents=True, exist_ok=True)
    ds = load_dataset(
        "pythainlp/thai-wiki-dataset-v3",
        split="train",
        streaming=True,
        trust_remote_code=True,
    )

    saved: list[Path] = []
    skipped = 0
    log_every = max(1, n // 10)

    for row in ds:
        text = row.get("text") or row.get("content") or ""
        text = text.strip()
        if len(text) < 300:          # skip stubs
            skipped += 1
            continue

        idx = len(saved)
        path = out_dir / f"wiki_{idx:05d}.txt"
        path.write_text(text, encoding="utf-8")
        saved.append(path)

        if len(saved) % log_every == 0:
            print(f"  {len(saved):,}/{n} articles saved…", flush=True)

        if len(saved) >= n:
            break

    print(f"  Saved {len(saved):,} articles  (skipped {skipped:,} stubs)\n")
    return saved


def index_files(files: list[Path], corpus: Path):
    print(f"Indexing {len(files):,} files into {corpus}…")
    corpus.unlink(missing_ok=True)
    for suf in ["-shm", "-wal"]:
        Path(str(corpus) + suf).unlink(missing_ok=True)

    ok = 0
    fail = 0
    log_every = max(1, len(files) // 10)

    for i, f in enumerate(files):
        r = subprocess.run(
            [str(BINARY), "index", str(f), "--corpus", str(corpus), "--genre", "wikipedia"],
            capture_output=True,
        )
        if r.returncode == 0:
            ok += 1
        else:
            fail += 1

        if (i + 1) % log_every == 0:
            print(f"  {i+1:,}/{len(files):,} indexed (ok={ok}, fail={fail})…", flush=True)

    print(f"\nDone: {ok:,} indexed, {fail:,} failed")
    print(f"Corpus: {corpus}")


def main():
    parser = argparse.ArgumentParser(description="Prepare kham-tnc test corpus")
    parser.add_argument("--articles", type=int, default=500)
    parser.add_argument("--corpus", default="tnc_test.sqlite")
    parser.add_argument("--out-dir", default="/tmp/tnc_testdata")
    parser.add_argument("--skip-index", action="store_true")
    args = parser.parse_args()

    corpus = Path(args.corpus)
    out_dir = Path(args.out_dir)

    # Build binary first (unless skip-index)
    if not args.skip_index:
        if not BINARY.exists():
            build_binary()
        else:
            print(f"Using existing binary: {BINARY}")

    # Download
    files = download_articles(args.articles, out_dir)

    if args.skip_index:
        print(f"--skip-index: files saved to {out_dir}")
        return

    # Index
    index_files(files, corpus)

    print(f"""
──────────────────────────────────────────────
 Corpus ready. Start the server:

   cargo run -p kham-tnc -- serve --corpus {corpus} --port 8080

 Then open: http://localhost:8080
──────────────────────────────────────────────""")


if __name__ == "__main__":
    main()

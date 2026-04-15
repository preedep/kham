#!/usr/bin/env python3
"""Compare kham-cli vs nlpO3 segmentation performance."""

import argparse
import subprocess
import time
import statistics
import json
import sys


def run_kham(text: str, iterations: int) -> list[float]:
    """Benchmark kham-cli."""
    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        result = subprocess.run(
            ["cargo", "run", "-p", "kham-cli", "--release", "--", text],
            capture_output=True, text=True,
        )
        elapsed = time.perf_counter() - start
        if result.returncode == 0:
            times.append(elapsed * 1000)  # ms
    return times


def run_nlpo3(text: str, iterations: int) -> list[float]:
    """Benchmark nlpO3 via Python."""
    script = f"""
import time
from nlpo3 import load_dict, segment
load_dict("words_th.txt", "default")
times = []
for _ in range({iterations}):
    start = time.perf_counter()
    segment({text!r}, "default")
    elapsed = time.perf_counter() - start
    times.append(elapsed * 1000)
import json
print(json.dumps(times))
"""
    result = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True, text=True,
    )
    if result.returncode == 0:
        return json.loads(result.stdout)
    return []


def report(name: str, times: list[float]):
    if not times:
        print(f"  {name}: NO DATA")
        return
    print(f"  {name}:")
    print(f"    mean:  {statistics.mean(times):.3f} ms")
    print(f"    p50:   {statistics.median(times):.3f} ms")
    print(f"    p99:   {sorted(times)[int(len(times)*0.99)]:.3f} ms")
    print(f"    stdev: {statistics.stdev(times) if len(times) > 1 else 0:.3f} ms")


def main():
    parser = argparse.ArgumentParser(description="Compare kham vs nlpO3")
    parser.add_argument("--input", required=True, help="Input text file")
    parser.add_argument("--iterations", type=int, default=100)
    args = parser.parse_args()

    with open(args.input) as f:
        text = f.read().strip()

    print(f"Input: {len(text)} chars, {len(text.encode())} bytes")
    print(f"Iterations: {args.iterations}\n")

    print("Running kham-cli...")
    kham_times = run_kham(text, args.iterations)

    print("Running nlpO3...")
    nlpo3_times = run_nlpo3(text, args.iterations)

    print("\n=== Results ===")
    report("kham", kham_times)
    report("nlpO3", nlpo3_times)

    if kham_times and nlpo3_times:
        ratio = statistics.mean(nlpo3_times) / statistics.mean(kham_times)
        print(f"\n  kham is {ratio:.2f}x {'faster' if ratio > 1 else 'slower'} than nlpO3")


if __name__ == "__main__":
    main()

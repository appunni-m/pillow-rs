#!/usr/bin/env python3
"""Compare pillow-rs benchmark results against Pillow baseline."""
import json
import sys


def load_bench(path: str) -> dict:
    with open(path) as f:
        return json.load(f)


def compare(rs_data: dict, pil_data: dict) -> list:
    results = []
    rs_benches = {b.get("name", ""): b for b in rs_data.get("benchmarks", [])}
    pil_benches = {b.get("name", ""): b for b in pil_data.get("benchmarks", [])}
    for name in sorted(set(rs_benches.keys()) & set(pil_benches.keys())):
        rs_stats = rs_benches[name].get("stats", {})
        pil_stats = pil_benches[name].get("stats", {})
        rs_mean = rs_stats.get("mean", 0) * 1000
        pil_mean = pil_stats.get("mean", 0) * 1000
        speedup = pil_mean / rs_mean if rs_mean > 0 else 0
        results.append({"name": name, "pillow_rs_ms": rs_mean, "pillow_ms": pil_mean, "speedup": speedup})
    return results


def print_table(results: list) -> None:
    print(f"{'Benchmark':<35} {'Pillow':>10} {'pillow-rs':>10} {'Speedup':>12}")
    print("-" * 67)
    for r in results:
        direction = "faster" if r["speedup"] > 1 else "slower"
        print(f"{r['name']:<35} {r['pillow_ms']:>8.1f}ms {r['pillow_rs_ms']:>8.1f}ms {r['speedup']:>7.2f}× {direction}")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: compare_benchmarks.py <pillow-rs-bench.json> <pillow-bench.json>")
        sys.exit(1)
    results = compare(load_bench(sys.argv[1]), load_bench(sys.argv[2]))
    print_table(results)

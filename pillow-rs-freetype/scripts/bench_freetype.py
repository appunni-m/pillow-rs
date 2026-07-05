#!/usr/bin/env python3
"""Run pillow-rs-freetype operation benchmarks.

The Rust benchmark path is always available and emits JSONL rows.  The C
FreeType comparison path is optional and uses scripts/bench_ft_ops.c as a
standalone helper; it is never linked into the Rust runtime crate.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_MATRIX = ROOT / "tests" / "fixtures" / "perf_operation_matrix.json"
DEFAULT_OUT = ROOT / "target" / "freetype-bench" / "latest.json"
HELPER_SRC = ROOT / "scripts" / "bench_ft_ops.c"
HELPER_BIN = ROOT / "target" / "freetype-bench" / "bench_ft_ops"


def run(cmd: list[str], *, cwd: pathlib.Path = ROOT, env: dict[str, str] | None = None) -> str:
    proc = subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return proc.stdout


def run_rust(matrix: pathlib.Path) -> list[dict[str, Any]]:
    stdout = run(
        [
            "cargo",
            "run",
            "-p",
            "pillow-rs-freetype",
            "--example",
            "bench_ops",
            "--release",
            "--locked",
            "--",
            str(matrix),
        ],
        cwd=ROOT.parent,
    )
    return [json.loads(line) for line in stdout.splitlines() if line.strip()]


def compile_c_helper(include_dir: pathlib.Path, lib_dir: pathlib.Path) -> pathlib.Path:
    HELPER_BIN.parent.mkdir(parents=True, exist_ok=True)
    compiler = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if compiler is None:
        raise RuntimeError("no C compiler found")
    run(
        [
            compiler,
            "-O3",
            "-std=c11",
            f"-I{include_dir}",
            str(HELPER_SRC),
            f"-L{lib_dir}",
            "-lfreetype",
            "-o",
            str(HELPER_BIN),
        ]
    )
    return HELPER_BIN


def run_c(
    matrix: pathlib.Path,
    helper: pathlib.Path,
    lib_dir: pathlib.Path,
) -> list[dict[str, Any]]:
    env = os.environ.copy()
    old_ld = env.get("LD_LIBRARY_PATH")
    env["LD_LIBRARY_PATH"] = (
        str(lib_dir) if not old_ld else f"{lib_dir}{os.pathsep}{old_ld}"
    )
    stdout = run([str(helper), str(matrix)], env=env)
    return [json.loads(line) for line in stdout.splitlines() if line.strip()]


def load_weights(matrix: pathlib.Path) -> dict[str, float]:
    data = json.loads(matrix.read_text())
    return {row["id"]: float(row.get("weight", 1.0)) for row in data.get("rows", [])}


def merge_rows(
    rust_rows: list[dict[str, Any]], c_rows: list[dict[str, Any]] | None
) -> list[dict[str, Any]]:
    c_by_id = {row["id"]: row for row in c_rows or []}
    merged = []
    for rust in rust_rows:
        row = dict(rust)
        c_row = c_by_id.get(row["id"])
        if c_row is not None:
            row["c_ns_total"] = c_row["c_ns_total"]
            row["c_ns_per_iter"] = c_row["c_ns_per_iter"]
            row["c_output_fingerprint"] = c_row.get("output_fingerprint")
            if c_row.get("output_sha256") and row.get("output_sha256") == c_row.get("output_sha256"):
                row["output_match"] = True
            elif c_row.get("output_sha256"):
                row["output_match"] = False
                row["c_output_sha256"] = c_row.get("output_sha256")
            if row["c_ns_per_iter"]:
                row["ratio_rust_to_c"] = row["rust_ns_per_iter"] / row["c_ns_per_iter"]
        merged.append(row)
    return merged


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    rank = (pct / 100.0) * (len(ordered) - 1)
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    fraction = rank - lower
    return ordered[lower] + (ordered[upper] - ordered[lower]) * fraction


def mean(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0


def summarize_rows(
    sample_rows: list[list[dict[str, Any]]],
    weights: dict[str, float],
) -> dict[str, Any]:
    by_id: dict[str, list[dict[str, Any]]] = {}
    order: list[str] = []
    for sample in sample_rows:
        for row in sample:
            row_id = row["id"]
            if row_id not in by_id:
                order.append(row_id)
                by_id[row_id] = []
            by_id[row_id].append(row)

    summary_rows: list[dict[str, Any]] = []
    rust_total_ns = 0.0
    c_total_ns = 0.0
    operation_count = 0
    weighted_rust_total = 0.0
    weighted_c_total = 0.0
    weighted_total = 0.0

    for row_id in order:
        samples = by_id[row_id]
        first = samples[0]
        rust_per_iter = [float(row["rust_ns_per_iter"]) for row in samples]
        c_per_iter = [
            float(row["c_ns_per_iter"])
            for row in samples
            if row.get("c_ns_per_iter") is not None
        ]
        speedups = [
            float(row["c_ns_per_iter"]) / float(row["rust_ns_per_iter"])
            for row in samples
            if row.get("c_ns_per_iter") and row.get("rust_ns_per_iter")
        ]
        rust_total = sum(float(row["rust_ns_total"]) for row in samples)
        c_total = sum(float(row.get("c_ns_total", 0)) for row in samples)
        iterations = int(first["iterations"])
        total_iterations = iterations * len(samples)
        weight = weights.get(row_id, 1.0)

        rust_total_ns += rust_total
        c_total_ns += c_total
        operation_count += total_iterations
        weighted_rust_total += weight * mean(rust_per_iter)
        if c_per_iter:
            weighted_c_total += weight * mean(c_per_iter)
        weighted_total += weight

        summary_rows.append(
            {
                "id": row_id,
                "operation": first["operation"],
                "iterations_per_sample": iterations,
                "sample_count": len(samples),
                "operation_count": total_iterations,
                "weight": weight,
                "rust_ns_per_iter_mean": mean(rust_per_iter),
                "rust_ns_per_iter_p90": percentile(rust_per_iter, 90),
                "rust_ns_per_iter_p99": percentile(rust_per_iter, 99),
                "c_ns_per_iter_mean": mean(c_per_iter),
                "c_ns_per_iter_p90": percentile(c_per_iter, 90),
                "c_ns_per_iter_p99": percentile(c_per_iter, 99),
                "speedup_vs_c_mean": mean(speedups),
                "speedup_vs_c_p90": percentile(speedups, 90),
                "speedup_vs_c_p99": percentile(speedups, 99),
                "rust_ns_total": int(rust_total),
                "c_ns_total": int(c_total),
            }
        )

    overall_speedup = c_total_ns / rust_total_ns if rust_total_ns else 0.0
    weighted_speedup = (
        weighted_c_total / weighted_rust_total if weighted_rust_total and weighted_c_total else 0.0
    )
    return {
        "rows": summary_rows,
        "overall": {
            "operation_count": operation_count,
            "rust_ns_total": int(rust_total_ns),
            "c_ns_total": int(c_total_ns),
            "speedup_vs_c_total": overall_speedup,
            "weighted_operation_weight": weighted_total,
            "weighted_speedup_vs_c": weighted_speedup,
        },
    }


def format_table(summary: dict[str, Any]) -> str:
    headers = [
        "id",
        "op",
        "count",
        "weight",
        "rust mean ns",
        "rust p90 ns",
        "rust p99 ns",
        "c mean ns",
        "c p90 ns",
        "c p99 ns",
        "mean speedup vs C",
        "p90 speedup",
        "p99 speedup",
    ]
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join(["---"] * len(headers)) + " |",
    ]
    for row in summary["rows"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    row["id"],
                    row["operation"],
                    str(row["operation_count"]),
                    f"{row['weight']:.2f}",
                    f"{row['rust_ns_per_iter_mean']:.1f}",
                    f"{row['rust_ns_per_iter_p90']:.1f}",
                    f"{row['rust_ns_per_iter_p99']:.1f}",
                    f"{row['c_ns_per_iter_mean']:.1f}",
                    f"{row['c_ns_per_iter_p90']:.1f}",
                    f"{row['c_ns_per_iter_p99']:.1f}",
                    f"{row['speedup_vs_c_mean']:.3f}x",
                    f"{row['speedup_vs_c_p90']:.3f}x",
                    f"{row['speedup_vs_c_p99']:.3f}x",
                ]
            )
            + " |"
        )
    overall = summary["overall"]
    lines.extend(
        [
            "",
            "| aggregate | value |",
            "| --- | --- |",
            f"| total operation count | {overall['operation_count']} |",
            f"| rust total ns | {overall['rust_ns_total']} |",
            f"| c total ns | {overall['c_ns_total']} |",
            f"| total speedup vs C | {overall['speedup_vs_c_total']:.3f}x |",
            f"| weighted operation weight | {overall['weighted_operation_weight']:.2f} |",
            f"| weighted speedup vs C | {overall['weighted_speedup_vs_c']:.3f}x |",
        ]
    )
    return "\n".join(lines)


def write_output(
    path: pathlib.Path,
    rows: list[dict[str, Any]],
    summary: dict[str, Any] | None = None,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload: dict[str, Any] = {"rows": rows}
    if summary is not None:
        payload["summary"] = summary
        payload["summary_markdown"] = format_table(summary)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix", type=pathlib.Path, default=DEFAULT_MATRIX)
    parser.add_argument("--out", type=pathlib.Path, default=DEFAULT_OUT)
    parser.add_argument("--compare-c", action="store_true")
    parser.add_argument("--samples", type=int, default=1)
    parser.add_argument("--table", action="store_true", help="print comparative summary table")
    parser.add_argument("--ft-include", type=pathlib.Path, default=pathlib.Path.home() / ".local/include/freetype2")
    parser.add_argument("--ft-lib", type=pathlib.Path, default=ROOT / "freetype/build")
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be >= 1")

    helper = None
    if args.compare_c:
        helper = compile_c_helper(args.ft_include, args.ft_lib)

    sample_rows = []
    rows: list[dict[str, Any]] = []
    for sample_index in range(args.samples):
        rust_rows = run_rust(args.matrix)
        c_rows = None
        if args.compare_c and helper is not None:
            c_rows = run_c(args.matrix, helper, args.ft_lib)
        merged = merge_rows(rust_rows, c_rows)
        for row in merged:
            row["sample_index"] = sample_index
        sample_rows.append(merged)
        rows.extend(merged)

    mismatches = [row for row in rows if row.get("output_match") is False]
    if mismatches:
        print("benchmark output mismatches:", file=sys.stderr)
        for row in mismatches:
            print(f"  {row['id']}", file=sys.stderr)
        return 1

    summary = summarize_rows(sample_rows, load_weights(args.matrix)) if args.compare_c else None
    write_output(args.out, rows, summary)
    if args.table and summary is not None:
        print(format_table(summary))
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

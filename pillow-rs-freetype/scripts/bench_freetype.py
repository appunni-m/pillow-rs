#!/usr/bin/env python3
"""Run pillow-rs-freetype operation benchmarks.

The Rust benchmark path is always available and emits JSONL rows.  The C
FreeType comparison path is optional and uses scripts/bench_ft_ops.c as a
standalone helper; it is never linked into the Rust runtime crate.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import platform
import shutil
import subprocess
import sys
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_MATRIX = ROOT / "tests" / "fixtures" / "perf_operation_matrix.json"
DEFAULT_OUT = ROOT / "target" / "freetype-bench" / "latest.json"
DEFAULT_REPORT = ROOT / "target" / "freetype-bench" / "latest.md"
HELPER_SRC = ROOT / "scripts" / "bench_ft_ops.c"
HELPER_BIN = ROOT / "target" / "freetype-bench" / "bench_ft_ops"
WORKSPACE_ROOT = ROOT.parent


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


def run_optional(cmd: list[str], *, cwd: pathlib.Path = ROOT) -> str | None:
    try:
        return run(cmd, cwd=cwd).strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None


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
        cwd=WORKSPACE_ROOT,
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


def load_matrix(matrix: pathlib.Path) -> dict[str, Any]:
    return json.loads(matrix.read_text())


def load_weights(matrix_data: dict[str, Any], profile: str) -> dict[str, float]:
    profiles = matrix_data.get("workload_profiles", {})
    if profile in profiles:
        weights = profiles[profile].get("weights", {})
        return {row_id: float(weight) for row_id, weight in weights.items()}
    if profile != "row_weight":
        available = ", ".join(sorted([*profiles.keys(), "row_weight"]))
        raise ValueError(f"unknown workload profile {profile!r}; available: {available}")
    return {
        row["id"]: float(row.get("weight", 1.0))
        for row in matrix_data.get("rows", [])
    }


def matrix_rows_by_id(matrix_data: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {row["id"]: row for row in matrix_data.get("rows", [])}


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


def weighted_mean(values: list[float], weights: list[float]) -> float:
    total_weight = sum(weights)
    if not values or total_weight == 0.0:
        return 0.0
    return sum(value * weight for value, weight in zip(values, weights, strict=True)) / total_weight


def weighted_percentile(values: list[float], weights: list[float], pct: float) -> float:
    if not values:
        return 0.0
    pairs = sorted(zip(values, weights, strict=True), key=lambda item: item[0])
    total_weight = sum(weight for _, weight in pairs)
    if total_weight == 0.0:
        return 0.0
    threshold = total_weight * pct / 100.0
    cumulative = 0.0
    for value, weight in pairs:
        cumulative += weight
        if cumulative >= threshold:
            return value
    return pairs[-1][0]


def median(values: list[float]) -> float:
    return percentile(values, 50)


def trimmed_mean(values: list[float], trim_fraction: float = 0.1) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    trim = int(len(ordered) * trim_fraction)
    if trim == 0 or trim * 2 >= len(ordered):
        return mean(ordered)
    return mean(ordered[trim:-trim])


def stddev(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    avg = mean(values)
    variance = sum((value - avg) ** 2 for value in values) / (len(values) - 1)
    return variance ** 0.5


def summarize_rows(
    sample_rows: list[list[dict[str, Any]]],
    weights: dict[str, float],
    matrix_by_id: dict[str, dict[str, Any]] | None = None,
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
    all_rust_per_iter = []
    all_c_per_iter = []
    all_speedups = []
    all_sample_weights = []
    groups: dict[str, dict[str, Any]] = {}

    for row_id in order:
        samples = by_id[row_id]
        first = samples[0]
        matrix_row = (matrix_by_id or {}).get(row_id, {})
        timing_category = timing_category_for_row(first, matrix_row)
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
        c_output_has_sha = any(row.get("c_output_sha256") for row in samples)
        output_match_checked = any(row.get("output_match") is not None for row in samples)
        rust_total = sum(float(row["rust_ns_total"]) for row in samples)
        c_total = sum(float(row.get("c_ns_total", 0)) for row in samples)
        iterations = int(first["iterations"])
        total_iterations = iterations * len(samples)
        weight = weights.get(row_id, 1.0)
        sample_weights = [float(iterations)] * len(samples)

        all_rust_per_iter.extend(rust_per_iter)
        all_c_per_iter.extend(c_per_iter)
        all_speedups.extend(speedups)
        all_sample_weights.extend(sample_weights)

        group = groups.setdefault(
            timing_category,
            {
                "operation_count": 0,
                "rust_ns_total": 0.0,
                "c_ns_total": 0.0,
                "weighted_operation_weight": 0.0,
                "weighted_rust_total": 0.0,
                "weighted_c_total": 0.0,
                "rust_per_iter": [],
                "c_per_iter": [],
                "speedups": [],
                "sample_weights": [],
            },
        )
        group["operation_count"] += total_iterations
        group["rust_ns_total"] += rust_total
        group["c_ns_total"] += c_total
        group["weighted_operation_weight"] += weight
        group["weighted_rust_total"] += weight * mean(rust_per_iter)
        if c_per_iter:
            group["weighted_c_total"] += weight * mean(c_per_iter)
        group["rust_per_iter"].extend(rust_per_iter)
        group["c_per_iter"].extend(c_per_iter)
        group["speedups"].extend(speedups)
        group["sample_weights"].extend(sample_weights)

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
                "timing_category": timing_category,
                "comparison_trust": matrix_row.get("comparison_trust", "unspecified"),
                "timing_boundary": matrix_row.get("timing_boundary", ""),
                "output_match_checked": output_match_checked,
                "c_output_has_sha256": c_output_has_sha,
                "iterations_per_sample": iterations,
                "sample_count": len(samples),
                "operation_count": total_iterations,
                "weight": weight,
                "rust_ns_per_iter_min": min(rust_per_iter),
                "rust_ns_per_iter_max": max(rust_per_iter),
                "rust_ns_per_iter_mean": mean(rust_per_iter),
                "rust_ns_per_iter_median": median(rust_per_iter),
                "rust_ns_per_iter_trimmed_mean": trimmed_mean(rust_per_iter),
                "rust_ns_per_iter_stddev": stddev(rust_per_iter),
                "rust_ns_per_iter_p90": percentile(rust_per_iter, 90),
                "rust_ns_per_iter_p99": percentile(rust_per_iter, 99),
                "c_ns_per_iter_min": min(c_per_iter) if c_per_iter else 0.0,
                "c_ns_per_iter_max": max(c_per_iter) if c_per_iter else 0.0,
                "c_ns_per_iter_mean": mean(c_per_iter),
                "c_ns_per_iter_median": median(c_per_iter),
                "c_ns_per_iter_trimmed_mean": trimmed_mean(c_per_iter),
                "c_ns_per_iter_stddev": stddev(c_per_iter),
                "c_ns_per_iter_p90": percentile(c_per_iter, 90),
                "c_ns_per_iter_p99": percentile(c_per_iter, 99),
                "speedup_vs_c_min": min(speedups) if speedups else 0.0,
                "speedup_vs_c_max": max(speedups) if speedups else 0.0,
                "speedup_vs_c_mean": mean(speedups),
                "speedup_vs_c_median": median(speedups),
                "speedup_vs_c_trimmed_mean": trimmed_mean(speedups),
                "speedup_vs_c_stddev": stddev(speedups),
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
            **distribution_stats(all_rust_per_iter, all_c_per_iter, all_speedups, all_sample_weights),
        },
        "groups": summarize_groups(groups),
    }


def timing_category_for_row(row: dict[str, Any], matrix_row: dict[str, Any]) -> str:
    boundary = matrix_row.get("timing_boundary", "")
    if row.get("operation") == "load_font" or "construct" in boundary and "timed loop" in boundary:
        return "font_load_path_dependent"
    return "cached_font_operation"


def distribution_stats(
    rust_per_iter: list[float],
    c_per_iter: list[float],
    speedups: list[float],
    sample_weights: list[float],
) -> dict[str, float]:
    speedup_weights = sample_weights[: len(speedups)]
    return {
        "rust_ns_per_iter_mean": weighted_mean(rust_per_iter, sample_weights),
        "rust_ns_per_iter_median": weighted_percentile(rust_per_iter, sample_weights, 50),
        "rust_ns_per_iter_p90": weighted_percentile(rust_per_iter, sample_weights, 90),
        "rust_ns_per_iter_p99": weighted_percentile(rust_per_iter, sample_weights, 99),
        "c_ns_per_iter_mean": weighted_mean(c_per_iter, sample_weights[: len(c_per_iter)]),
        "c_ns_per_iter_median": weighted_percentile(c_per_iter, sample_weights[: len(c_per_iter)], 50),
        "c_ns_per_iter_p90": weighted_percentile(c_per_iter, sample_weights[: len(c_per_iter)], 90),
        "c_ns_per_iter_p99": weighted_percentile(c_per_iter, sample_weights[: len(c_per_iter)], 99),
        "speedup_vs_c_mean": weighted_mean(speedups, speedup_weights),
        "speedup_vs_c_median": weighted_percentile(speedups, speedup_weights, 50),
        "speedup_vs_c_p90": weighted_percentile(speedups, speedup_weights, 90),
        "speedup_vs_c_p99": weighted_percentile(speedups, speedup_weights, 99),
    }


def summarize_groups(groups: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    labels = {
        "cached_font_operation": "Cached font operations",
        "font_load_path_dependent": "Font load / path-dependent setup",
    }
    rows = []
    for category, group in sorted(groups.items()):
        rust_total = float(group["rust_ns_total"])
        c_total = float(group["c_ns_total"])
        weighted_rust = float(group["weighted_rust_total"])
        weighted_c = float(group["weighted_c_total"])
        rows.append(
            {
                "category": category,
                "label": labels.get(category, category),
                "operation_count": group["operation_count"],
                "rust_ns_total": int(rust_total),
                "c_ns_total": int(c_total),
                "speedup_vs_c_total": c_total / rust_total if rust_total else 0.0,
                "weighted_operation_weight": group["weighted_operation_weight"],
                "weighted_speedup_vs_c": weighted_c / weighted_rust if weighted_rust and weighted_c else 0.0,
                **distribution_stats(
                    group["rust_per_iter"],
                    group["c_per_iter"],
                    group["speedups"],
                    group["sample_weights"],
                ),
            }
        )
    return rows


def read_cpu_model() -> str | None:
    cpuinfo = pathlib.Path("/proc/cpuinfo")
    if not cpuinfo.exists():
        return platform.processor() or None
    for line in cpuinfo.read_text(errors="ignore").splitlines():
        if line.startswith("model name"):
            return line.split(":", 1)[1].strip()
    return platform.processor() or None


def read_cpu_governor() -> str | None:
    governors = sorted(pathlib.Path("/sys/devices/system/cpu").glob("cpu*/cpufreq/scaling_governor"))
    values = []
    for governor in governors[:8]:
        try:
            values.append(governor.read_text().strip())
        except OSError:
            continue
    return ",".join(sorted(set(values))) if values else None


def read_khz_file(path: pathlib.Path) -> int | None:
    try:
        return int(path.read_text().strip())
    except (OSError, ValueError):
        return None


def format_mhz(khz: float | int | None) -> str | None:
    if khz is None:
        return None
    return f"{float(khz) / 1000.0:.0f} MHz"


def read_cpu_frequencies() -> dict[str, Any]:
    policies = sorted(pathlib.Path("/sys/devices/system/cpu/cpufreq").glob("policy*"))
    current = []
    maximum = []
    for policy in policies:
        cur = read_khz_file(policy / "scaling_cur_freq")
        max_freq = read_khz_file(policy / "cpuinfo_max_freq")
        if cur is not None:
            current.append(cur)
        if max_freq is not None:
            maximum.append(max_freq)
    if not current and not maximum:
        return {}
    return {
        "current_min_mhz": format_mhz(min(current)) if current else None,
        "current_max_mhz": format_mhz(max(current)) if current else None,
        "current_mean_mhz": format_mhz(mean([float(value) for value in current])) if current else None,
        "cpuinfo_max_mhz": format_mhz(max(maximum)) if maximum else None,
        "policy_count": len(policies),
    }


def read_memory_info() -> dict[str, Any]:
    meminfo = pathlib.Path("/proc/meminfo")
    info: dict[str, Any] = {
        "total": None,
        "available": None,
        "speed": None,
        "clock": None,
        "source": "/proc/meminfo; speed/clock not exposed",
    }
    if meminfo.exists():
        values = {}
        for line in meminfo.read_text(errors="ignore").splitlines():
            key, _, rest = line.partition(":")
            values[key] = rest.strip()
        info["total"] = values.get("MemTotal")
        info["available"] = values.get("MemAvailable")

    # Desktop/server DIMM speed is usually exposed through SMBIOS/EDAC only
    # with elevated privileges or platform-specific drivers. Keep explicit
    # unknowns rather than manufacturing a number.
    for candidate in (
        pathlib.Path("/sys/class/dmi/id/product_version"),
        pathlib.Path("/sys/class/dmi/id/board_name"),
    ):
        try:
            value = candidate.read_text(errors="ignore").strip()
        except OSError:
            continue
        if value:
            info.setdefault("platform_hint", value)
            break
    return info


def build_metadata(args: argparse.Namespace, matrix_data: dict[str, Any]) -> dict[str, Any]:
    cc = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    return {
        "schema_version": 2,
        "created_utc": dt.datetime.now(dt.UTC).isoformat(),
        "git_sha": run_optional(["git", "rev-parse", "HEAD"], cwd=WORKSPACE_ROOT),
        "git_dirty": bool(run_optional(["git", "status", "--short"], cwd=WORKSPACE_ROOT)),
        "workspace_root": str(WORKSPACE_ROOT),
        "matrix": str(args.matrix),
        "matrix_version": matrix_data.get("version"),
        "workload_profile": args.profile,
        "sample_count": args.samples,
        "compare_c": args.compare_c,
        "rustc_version": run_optional(["rustc", "--version"], cwd=WORKSPACE_ROOT),
        "cargo_version": run_optional(["cargo", "--version"], cwd=WORKSPACE_ROOT),
        "python_version": sys.version.split()[0],
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu_model": read_cpu_model(),
        "cpu_governor": read_cpu_governor(),
        "cpu_frequency": read_cpu_frequencies(),
        "memory": read_memory_info(),
        "c_compiler": cc,
        "c_compiler_version": run_optional([cc, "--version"], cwd=WORKSPACE_ROOT).splitlines()[0] if cc else None,
        "ft_include": str(args.ft_include),
        "ft_lib": str(args.ft_lib),
        "timing_notes": [
            "Rust benchmark is cargo run --release --locked for the bench_ops example.",
            "C helper is standalone tooling compiled by this script and never linked into runtime code.",
            "Rows marked timing_only have C timing/fingerprint but not exact comparable C SHA-256 output parity.",
            "Exact correctness remains enforced by fixture parity tests.",
        ],
    }


def format_table(summary: dict[str, Any], *, include_aggregate: bool = True) -> str:
    headers = [
        "id",
        "op",
        "count",
        "weight",
        "trust",
        "rust total ms",
        "c total ms",
        "rust median ns",
        "rust mean ns",
        "rust stddev",
        "rust p90 ns",
        "rust p99 ns",
        "c median ns",
        "c mean ns",
        "c stddev",
        "c p90 ns",
        "c p99 ns",
        "median speedup vs C",
        "mean speedup vs C",
        "stddev speedup",
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
                    row["comparison_trust"],
                    f"{row['rust_ns_total'] / 1_000_000.0:.3f}",
                    f"{row['c_ns_total'] / 1_000_000.0:.3f}",
                    f"{row['rust_ns_per_iter_median']:.1f}",
                    f"{row['rust_ns_per_iter_mean']:.1f}",
                    f"{row['rust_ns_per_iter_stddev']:.1f}",
                    f"{row['rust_ns_per_iter_p90']:.1f}",
                    f"{row['rust_ns_per_iter_p99']:.1f}",
                    f"{row['c_ns_per_iter_median']:.1f}",
                    f"{row['c_ns_per_iter_mean']:.1f}",
                    f"{row['c_ns_per_iter_stddev']:.1f}",
                    f"{row['c_ns_per_iter_p90']:.1f}",
                    f"{row['c_ns_per_iter_p99']:.1f}",
                    f"{row['speedup_vs_c_median']:.3f}x",
                    f"{row['speedup_vs_c_mean']:.3f}x",
                    f"{row['speedup_vs_c_stddev']:.3f}x",
                    f"{row['speedup_vs_c_p90']:.3f}x",
                    f"{row['speedup_vs_c_p99']:.3f}x",
                ]
            )
            + " |"
        )
    if include_aggregate:
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


def format_ms(ns: int | float) -> str:
    return f"{float(ns) / 1_000_000.0:.3f} ms"


def aggregate_table(summary: dict[str, Any]) -> str:
    overall = summary["overall"]
    rows = [
        ("Total operation count", overall["operation_count"]),
        ("Rust total time", format_ms(overall["rust_ns_total"])),
        ("C total time", format_ms(overall["c_ns_total"])),
        ("Total speedup vs C", f"{overall['speedup_vs_c_total']:.3f}x"),
        ("Weighted operation weight", f"{overall['weighted_operation_weight']:.2f}"),
        ("Weighted speedup vs C", f"{overall['weighted_speedup_vs_c']:.3f}x"),
    ]
    lines = ["| Metric | Value |", "| --- | --- |"]
    for key, value in rows:
        lines.append(f"| {key} | {value} |")
    return "\n".join(lines)


def group_summary_table(summary: dict[str, Any]) -> str:
    lines = [
        "| Group | count | rust total ms | c total ms | total speedup vs C | weighted speedup vs C |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for group in summary.get("groups", []):
        lines.append(
            f"| {group['label']} | {group['operation_count']} | "
            f"{group['rust_ns_total'] / 1_000_000.0:.3f} | "
            f"{group['c_ns_total'] / 1_000_000.0:.3f} | "
            f"{group['speedup_vs_c_total']:.3f}x | "
            f"{group['weighted_speedup_vs_c']:.3f}x |"
        )
    return "\n".join(lines)


def overall_distribution_table(summary: dict[str, Any]) -> str:
    overall = summary["overall"]
    rows = [
        (
            "Rust ns/iter",
            overall["rust_ns_per_iter_mean"],
            overall["rust_ns_per_iter_median"],
            overall["rust_ns_per_iter_p90"],
            overall["rust_ns_per_iter_p99"],
        ),
        (
            "C ns/iter",
            overall["c_ns_per_iter_mean"],
            overall["c_ns_per_iter_median"],
            overall["c_ns_per_iter_p90"],
            overall["c_ns_per_iter_p99"],
        ),
        (
            "Per-row speedup vs C",
            overall["speedup_vs_c_mean"],
            overall["speedup_vs_c_median"],
            overall["speedup_vs_c_p90"],
            overall["speedup_vs_c_p99"],
        ),
    ]
    lines = ["| Distribution | mean | median | p90 | p99 |", "| --- | --- | --- | --- | --- |"]
    for label, avg, med, p90, p99 in rows:
        suffix = "x" if "speedup" in label else " ns"
        lines.append(
            f"| {label} | {avg:.3f}{suffix} | {med:.3f}{suffix} | "
            f"{p90:.3f}{suffix} | {p99:.3f}{suffix} |"
        )
    return "\n".join(lines)


def group_distribution_table(group: dict[str, Any]) -> str:
    rows = [
        (
            "Rust ns/iter",
            group["rust_ns_per_iter_mean"],
            group["rust_ns_per_iter_median"],
            group["rust_ns_per_iter_p90"],
            group["rust_ns_per_iter_p99"],
        ),
        (
            "C ns/iter",
            group["c_ns_per_iter_mean"],
            group["c_ns_per_iter_median"],
            group["c_ns_per_iter_p90"],
            group["c_ns_per_iter_p99"],
        ),
        (
            "Per-row speedup vs C",
            group["speedup_vs_c_mean"],
            group["speedup_vs_c_median"],
            group["speedup_vs_c_p90"],
            group["speedup_vs_c_p99"],
        ),
    ]
    lines = ["| Distribution | mean | median | p90 | p99 |", "| --- | --- | --- | --- | --- |"]
    for label, avg, med, p90, p99 in rows:
        suffix = "x" if "speedup" in label else " ns"
        lines.append(
            f"| {label} | {avg:.3f}{suffix} | {med:.3f}{suffix} | "
            f"{p90:.3f}{suffix} | {p99:.3f}{suffix} |"
        )
    return "\n".join(lines)


def split_operation_tables(summary: dict[str, Any]) -> str:
    labels = {
        "cached_font_operation": "Cached Font Operations",
        "font_load_path_dependent": "Font Load / Path-Dependent Setup",
    }
    lines = []
    for category in ("cached_font_operation", "font_load_path_dependent"):
        rows = [row for row in summary["rows"] if row.get("timing_category") == category]
        if not rows:
            continue
        lines.extend(["", f"### {labels.get(category, category)}", ""])
        lines.append(format_table({"rows": rows, "overall": summary["overall"]}, include_aggregate=False))
    return "\n".join(lines)


def metadata_table(metadata: dict[str, Any]) -> str:
    cpu_frequency = metadata.get("cpu_frequency") or {}
    memory = metadata.get("memory") or {}
    rows = [
        ("Created UTC", metadata.get("created_utc")),
        ("Git SHA", metadata.get("git_sha")),
        ("Git dirty", metadata.get("git_dirty")),
        ("Workload profile", metadata.get("workload_profile")),
        ("Samples", metadata.get("sample_count")),
        ("Matrix", metadata.get("matrix")),
        ("Matrix version", metadata.get("matrix_version")),
        ("Platform", metadata.get("platform")),
        ("Machine", metadata.get("machine")),
        ("CPU", metadata.get("cpu_model")),
        ("CPU governor", metadata.get("cpu_governor") or "not available"),
        ("CPU current min", cpu_frequency.get("current_min_mhz") or "not available"),
        ("CPU current max", cpu_frequency.get("current_max_mhz") or "not available"),
        ("CPU current mean", cpu_frequency.get("current_mean_mhz") or "not available"),
        ("CPU max", cpu_frequency.get("cpuinfo_max_mhz") or "not available"),
        ("CPU policy count", cpu_frequency.get("policy_count") or "not available"),
        ("Memory total", memory.get("total") or "not available"),
        ("Memory available", memory.get("available") or "not available"),
        ("Memory speed", memory.get("speed") or "not available"),
        ("Memory clock", memory.get("clock") or "not available"),
        ("Memory source", memory.get("source") or "not available"),
        ("Rust", metadata.get("rustc_version")),
        ("Cargo", metadata.get("cargo_version")),
        ("Python", metadata.get("python_version")),
        ("C compiler", metadata.get("c_compiler_version") or metadata.get("c_compiler")),
        ("FreeType include", metadata.get("ft_include")),
        ("FreeType lib", metadata.get("ft_lib")),
    ]
    lines = ["| Parameter | Value |", "| --- | --- |"]
    for key, value in rows:
        lines.append(f"| {key} | {value} |")
    return "\n".join(lines)


def benchmark_configuration_table(metadata: dict[str, Any]) -> str:
    rows = [
        ("Workload profile", metadata.get("workload_profile")),
        ("Samples", metadata.get("sample_count")),
        ("Compare C", metadata.get("compare_c")),
        ("Matrix", metadata.get("matrix")),
        ("Matrix version", metadata.get("matrix_version")),
        ("FreeType include", metadata.get("ft_include")),
        ("FreeType lib", metadata.get("ft_lib")),
    ]
    lines = ["| Parameter | Value |", "| --- | --- |"]
    for key, value in rows:
        lines.append(f"| {key} | {value} |")
    return "\n".join(lines)


def format_report(metadata: dict[str, Any], summary: dict[str, Any] | None) -> str:
    lines = [
        "# pillow-rs-freetype Benchmark Report",
        "",
        "This report is generated by `scripts/bench_freetype.py`. Raw samples and",
        "machine-readable summaries are stored in the paired JSON artifact.",
        "",
        "## Benchmark Configuration",
        "",
        benchmark_configuration_table(metadata),
        "",
        "## Environment",
        "",
        metadata_table(metadata),
        "",
        "## Trust Notes",
        "",
    ]
    for note in metadata.get("timing_notes", []):
        lines.append(f"- {note}")
    if metadata.get("git_dirty"):
        lines.append("- Warning: the benchmark was generated from a dirty worktree.")
    lines.extend(
        [
            "",
            "## Interpretation Notes",
            "",
            "- Aggregate speedup is the ratio of total C time to total Rust time.",
            "- Weighted speedup uses the selected workload profile weights.",
            "- Distribution rows are operation-count weighted. They describe the",
            "  distribution of row-level timings, not a replacement for aggregate speedup.",
            "- Per-row speedup percentiles are useful for spotting operation families,",
            "  but they are not mathematically equivalent to total speedup.",
            "- Font load/path-dependent setup is separated from cached font operations",
            "  because path-backed face creation can include filesystem and OS page-cache effects.",
        ]
    )
    lines.extend(["", "## Aggregate Summary", ""])
    if summary is None:
        lines.append("No C comparison summary was generated. Run with `--compare-c`.")
    else:
        lines.append(aggregate_table(summary))
        lines.extend(["", "## Operation Groups", ""])
        lines.append(group_summary_table(summary))
        lines.extend(["", "## Overall Distribution", ""])
        lines.append(overall_distribution_table(summary))
        for group in summary.get("groups", []):
            lines.extend(["", f"## {group['label']} Distribution", ""])
            lines.append(group_distribution_table(group))
        lines.extend(["", "## Per-Operation Results", ""])
        lines.append(split_operation_tables(summary))
    lines.extend(
        [
            "",
            "## Reproduction",
            "",
            "```bash",
            "python3 pillow-rs-freetype/scripts/bench_freetype.py --compare-c --samples 10 --profile default --table",
            "```",
            "",
        ]
    )
    return "\n".join(lines)


def write_output(
    path: pathlib.Path,
    report_path: pathlib.Path,
    rows: list[dict[str, Any]],
    metadata: dict[str, Any],
    summary: dict[str, Any] | None = None,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    payload: dict[str, Any] = {"metadata": metadata, "rows": rows}
    if summary is not None:
        payload["summary"] = summary
        payload["summary_markdown"] = format_table(summary)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    report_path.write_text(format_report(metadata, summary) + "\n")


def run_self_test() -> int:
    samples = [
        [
            {
                "id": "a",
                "operation": "op",
                "iterations": 10,
                "rust_ns_total": 100,
                "rust_ns_per_iter": 10,
                "c_ns_total": 200,
                "c_ns_per_iter": 20,
            },
            {
                "id": "b",
                "operation": "op",
                "iterations": 5,
                "rust_ns_total": 100,
                "rust_ns_per_iter": 20,
                "c_ns_total": 50,
                "c_ns_per_iter": 10,
            },
        ],
        [
            {
                "id": "a",
                "operation": "op",
                "iterations": 10,
                "rust_ns_total": 200,
                "rust_ns_per_iter": 20,
                "c_ns_total": 400,
                "c_ns_per_iter": 40,
            },
            {
                "id": "b",
                "operation": "op",
                "iterations": 5,
                "rust_ns_total": 200,
                "rust_ns_per_iter": 40,
                "c_ns_total": 100,
                "c_ns_per_iter": 20,
            },
        ],
    ]
    summary = summarize_rows(
        samples,
        {"a": 2.0, "b": 1.0},
        {
            "a": {"comparison_trust": "exact_sha256", "timing_boundary": "test"},
            "b": {"comparison_trust": "timing_only", "timing_boundary": "test"},
        },
    )
    assert summary["overall"]["operation_count"] == 30
    assert summary["overall"]["rust_ns_total"] == 600
    assert summary["overall"]["c_ns_total"] == 750
    assert round(summary["overall"]["speedup_vs_c_total"], 6) == 1.25
    assert round(summary["overall"]["weighted_speedup_vs_c"], 6) == 1.25
    assert summary["overall"]["rust_ns_per_iter_mean"] == 20.0
    assert summary["overall"]["rust_ns_per_iter_median"] == 20.0
    assert summary["overall"]["rust_ns_per_iter_p90"] == 40.0
    assert summary["overall"]["c_ns_per_iter_p99"] == 40.0
    assert summary["overall"]["speedup_vs_c_p90"] == 2.0
    assert summary["groups"][0]["category"] == "cached_font_operation"
    assert summary["rows"][0]["speedup_vs_c_mean"] == 2.0
    assert summary["rows"][1]["speedup_vs_c_mean"] == 0.5
    print("bench_freetype.py self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix", type=pathlib.Path, default=DEFAULT_MATRIX)
    parser.add_argument("--out", type=pathlib.Path, default=DEFAULT_OUT)
    parser.add_argument("--report", type=pathlib.Path, default=DEFAULT_REPORT)
    parser.add_argument("--compare-c", action="store_true")
    parser.add_argument("--profile", default="default")
    parser.add_argument("--samples", type=int, default=1)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--table", action="store_true", help="print comparative summary table")
    parser.add_argument("--ft-include", type=pathlib.Path, default=pathlib.Path.home() / ".local/include/freetype2")
    parser.add_argument("--ft-lib", type=pathlib.Path, default=ROOT / "freetype/build")
    args = parser.parse_args()
    if args.self_test:
        return run_self_test()
    if args.samples < 1:
        parser.error("--samples must be >= 1")
    matrix_data = load_matrix(args.matrix)
    try:
        weights = load_weights(matrix_data, args.profile)
    except ValueError as err:
        parser.error(str(err))

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

    metadata = build_metadata(args, matrix_data)
    summary = summarize_rows(sample_rows, weights, matrix_rows_by_id(matrix_data)) if args.compare_c else None
    write_output(args.out, args.report, rows, metadata, summary)
    if args.table and summary is not None:
        print(format_table(summary))
    print(f"wrote {args.out}")
    print(f"wrote {args.report}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

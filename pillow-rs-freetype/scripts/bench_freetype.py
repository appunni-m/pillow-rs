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


def write_output(path: pathlib.Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"rows": rows}, indent=2, sort_keys=True) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix", type=pathlib.Path, default=DEFAULT_MATRIX)
    parser.add_argument("--out", type=pathlib.Path, default=DEFAULT_OUT)
    parser.add_argument("--compare-c", action="store_true")
    parser.add_argument("--ft-include", type=pathlib.Path, default=pathlib.Path.home() / ".local/include/freetype2")
    parser.add_argument("--ft-lib", type=pathlib.Path, default=ROOT / "freetype/build")
    args = parser.parse_args()

    rust_rows = run_rust(args.matrix)
    c_rows = None
    if args.compare_c:
        helper = compile_c_helper(args.ft_include, args.ft_lib)
        c_rows = run_c(args.matrix, helper, args.ft_lib)

    rows = merge_rows(rust_rows, c_rows)
    mismatches = [row for row in rows if row.get("output_match") is False]
    if mismatches:
        print("benchmark output mismatches:", file=sys.stderr)
        for row in mismatches:
            print(f"  {row['id']}", file=sys.stderr)
        return 1

    write_output(args.out, rows)
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

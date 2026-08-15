#!/usr/bin/env python3
"""Capture a bounded profile for one maintained benchmark workflow.

This is a diagnostic lane, not a correctness or unit-test lane.  It executes
one declarative benchmark workflow through the same adapter used by the
managed benchmark and records the exact command, workload, backend, revision,
dirty state, phase timings, backend receipts, child RSS, and optional macOS
``sample``/``heap`` output.  A missing profiler or GPU adapter is retained as
an explicit receipt instead of changing the benchmark denominator.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import platform
import resource
import subprocess
import sys
import time
from typing import Any

try:
    import run_migration_benchmark as benchmark
    from run_migration_parity import (
        git_dirty,
        git_revision,
        process_group_options,
        reap_timed_out_process,
    )
except ModuleNotFoundError:  # imported as scripts.profile_migration_benchmark
    from scripts import run_migration_benchmark as benchmark
    from scripts.run_migration_parity import (
        git_dirty,
        git_revision,
        process_group_options,
        reap_timed_out_process,
    )

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
MANIFEST = FIXTURE_ROOT / "manifest.yaml"


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")


def rss_delta_bytes(before: resource.struct_rusage, after: resource.struct_rusage) -> int:
    """Normalize RUSAGE_CHILDREN max RSS to bytes on both supported kernels."""

    delta = max(0, int(after.ru_maxrss) - int(before.ru_maxrss))
    return delta if sys.platform == "darwin" else delta * 1024


def safe_slug(value: str) -> str:
    return "".join(character if character.isalnum() or character in "-_." else "_" for character in value)


def write_text(path: Path, value: str) -> str:
    path.write_text(value, encoding="utf-8")
    return str(path.relative_to(ROOT))


def run_optional_profiler(
    command: list[str],
    output: Path,
    *,
    timeout: int,
) -> dict[str, Any]:
    if not command:
        return {"status": "not_requested", "path": None, "stderr": None}
    try:
        result = subprocess.run(
            command,
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout,
            check=False,
        )
    except FileNotFoundError:
        return {
            "status": "tool_unavailable",
            "path": None,
            "stderr": f"missing profiler executable: {command[0]}",
        }
    except subprocess.TimeoutExpired as exc:
        return {
            "status": "timed_out",
            "path": None,
            "stderr": str(exc),
        }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        result.stdout + ("\n" + result.stderr if result.stderr else ""),
        encoding="utf-8",
    )
    return {
        "status": "completed" if result.returncode == 0 else "failed",
        "path": str(output.relative_to(ROOT)),
        "returncode": result.returncode,
        "stderr": result.stderr[-1000:] if result.stderr else None,
    }


def profile(args: argparse.Namespace) -> int:
    if args.backend not in {"pillow", "cpu", "simd", "gpu"}:
        raise ValueError("backend must be pillow, cpu, simd, or gpu")
    if args.repeat <= 0 or args.timeout <= 0 or args.sample_seconds < 0:
        raise ValueError("repeat, timeout, and sample-seconds must be non-negative/positive")

    manifest_data = benchmark.load_manifest(MANIFEST)
    workloads, _suites, _inputs = benchmark.load_benchmarks(manifest_data)
    workload = workloads.get(args.workload_id)
    if workload is None:
        raise ValueError(f"unknown benchmark workload: {args.workload_id}")
    if workload["input"]["kind"] == "workflow":
        case = benchmark.benchmark_workflow_case(workload)
    elif workload["input"]["kind"] == "parity_case":
        cases, _case_inputs = benchmark.load_parity_cases(manifest_data)
        case_id = workload["input"]["case_id"]
        try:
            case = cases[case_id]
        except KeyError as exc:
            raise ValueError(f"missing parity case for {args.workload_id}: {case_id}") from exc
    else:
        raise ValueError("profile target must be a workflow or parity-backed workload")
    side = "source" if args.backend == "pillow" else "target"
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    slug = f"{safe_slug(args.workload_id)}-{args.backend}"
    envelope_path = output_dir / f"{slug}.adapter.json"
    sample_path = output_dir / f"{slug}.sample.txt"
    heap_path = output_dir / f"{slug}.heap.txt"
    result_path = output_dir / f"{slug}.profile.json"

    command = [
        sys.executable,
        str(ROOT / "scripts" / "run_migration_parity.py"),
        "--side",
        side,
        "--manifest",
        str(MANIFEST),
        "--repeat",
        str(args.repeat),
        "--timings",
        "--timing-boundary",
        "whole_workflow",
        "--timing-step",
        "call",
        "--timeout",
        str(args.timeout),
    ]
    environment = {
        **os.environ,
        "MIGRATION_TARGET_BACKEND": "cpu" if args.backend == "pillow" else args.backend,
        "MIGRATION_STRICT_TARGET_BACKEND": "1",
        "PYTHONPATH": str(ROOT / "pillow-rs-py" / "python")
        + os.pathsep
        + os.environ.get("PYTHONPATH", ""),
    }
    started_at = now()
    before_rusage = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.monotonic()
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        env=environment,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        **process_group_options(),
    )

    sample_process: subprocess.Popen[str] | None = None
    sample_receipt: dict[str, Any]
    if args.sample_seconds and sys.platform == "darwin":
        sample_command = [
            "/usr/bin/sample",
            str(process.pid),
            str(args.sample_seconds),
            "-mayDie",
            "-file",
            str(sample_path),
        ]
        try:
            sample_process = subprocess.Popen(
                sample_command,
                cwd=ROOT,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            sample_receipt = {
                "status": "started",
                "command": sample_command,
                "path": str(sample_path.relative_to(ROOT)),
            }
        except FileNotFoundError:
            sample_receipt = {"status": "tool_unavailable", "path": None}
    elif args.sample_seconds:
        sample_receipt = {
            "status": "unsupported_platform",
            "platform": sys.platform,
            "path": None,
        }
    else:
        sample_receipt = {"status": "not_requested", "path": None}

    heap_receipt: dict[str, Any]
    if sys.platform == "darwin":
        heap_receipt = run_optional_profiler(
            ["/usr/bin/heap", "--forkCorpse", str(process.pid)],
            heap_path,
            timeout=min(30, args.timeout),
        )
    else:
        heap_receipt = {
            "status": "unsupported_platform",
            "path": None,
            "reason": sys.platform,
        }

    try:
        stdout, stderr = process.communicate(
            input=json.dumps([case], separators=(",", ":")),
            timeout=args.timeout,
        )
        timed_out = False
    except subprocess.TimeoutExpired:
        stdout, stderr = reap_timed_out_process(process)
        timed_out = True
    elapsed_ms = (time.monotonic() - started) * 1000.0
    after_rusage = resource.getrusage(resource.RUSAGE_CHILDREN)

    if sample_process is not None:
        try:
            sample_stdout, sample_stderr = sample_process.communicate(timeout=max(5, args.timeout))
            sample_receipt.update(
                {
                    "status": "completed" if sample_process.returncode == 0 else "failed",
                    "returncode": sample_process.returncode,
                    "stderr": sample_stderr[-1000:] if sample_stderr else None,
                }
            )
            if sample_stdout and not sample_path.is_file():
                sample_path.write_text(sample_stdout, encoding="utf-8")
        except subprocess.TimeoutExpired:
            sample_process.kill()
            sample_receipt.update({"status": "timed_out", "stderr": "sample timed out"})

    envelope: dict[str, Any] | None = None
    parse_error: str | None = None
    if stdout.strip():
        try:
            envelope = json.loads(stdout)
            envelope_path.write_text(json.dumps(envelope, indent=2) + "\n", encoding="utf-8")
        except json.JSONDecodeError as exc:
            parse_error = str(exc)
    status = "timed_out" if timed_out else "completed" if process.returncode == 0 and envelope else "failed"
    profile_result = {
        "schema": "pillow-rs/adapter-profile@1",
        "status": status,
        "identity": {
            "workload_id": args.workload_id,
            "case_id": case["case_id"],
            "backend": args.backend,
            "side": side,
            "revision": git_revision(),
            "dirty": git_dirty(),
            "machine_id": platform.node() or "unknown",
            "os": platform.platform(),
            "architecture": platform.machine(),
            "python": platform.python_version(),
            "started_at": started_at,
        },
        "command": {
            "argv": command,
            "cwd": str(ROOT),
            "repeat": args.repeat,
            "timeout_seconds": args.timeout,
        },
        "runtime": {
            "elapsed_ms": elapsed_ms,
            "exit_code": process.returncode,
            "child_max_rss_bytes_delta": rss_delta_bytes(before_rusage, after_rusage),
            "sample": sample_receipt,
            "heap": heap_receipt,
        },
        "adapter": {
            "envelope": str(envelope_path.relative_to(ROOT)) if envelope is not None else None,
            "parse_error": parse_error,
            "stderr": stderr[-2000:] if stderr else None,
            "timings_ns": envelope.get("timings_ns") if envelope else None,
            "telemetry": envelope.get("telemetry") if envelope else None,
            "execution": envelope.get("execution") if envelope else None,
        },
    }
    result_path.write_text(json.dumps(profile_result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(result_path.relative_to(ROOT)), "status": status}, sort_keys=True))
    return 0 if status == "completed" else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workload-id", required=True)
    parser.add_argument("--backend", choices=("pillow", "cpu", "simd", "gpu"), default="cpu")
    parser.add_argument("--repeat", type=int, default=40)
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--sample-seconds", type=int, default=5)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "build" / "migration-parity" / "profiles")
    return profile(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())

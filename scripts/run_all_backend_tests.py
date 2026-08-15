#!/usr/bin/env python3
"""Run the complete safe test campaign across every supported target lane.

The Python parity adapter selects one target backend per process, so a single
parity process cannot honestly exercise CPU, SIMD, and GPU at once.  This
orchestrator keeps the campaign as one maintained command while running the
active corpus once per backend.  It also runs the declared JS/WASM package
checks and records every lane in one generated evidence file.

GPU is gated by one bounded public parity case, then the full GPU corpus is
requested by default with a separate hard deadline.  Adapter absence is
recorded as explicitly skipped/not proven.  A timeout, crash, parity mismatch,
or other real GPU failure remains fatal.  Every child lane starts in its own
process group so a native driver wedge cannot outlive the lane deadline.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
from pathlib import Path
import signal
import subprocess
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "all-backends-test-result.json"
DEFAULT_TIMEOUT_SECONDS = 7200
GPU_SMOKE_TIMEOUT_SECONDS = 180
GPU_FULL_TIMEOUT_SECONDS = 300
PROCESS_REAP_TIMEOUT_SECONDS = 10
GIT_COMMAND_TIMEOUT_SECONDS = 10
GPU_SMOKE_CASE = "PIL.ImageFilter.UnsharpMask.behavior.default"
GPU_FULL_DISABLED_REASON = (
    "full GPU parity was disabled explicitly; the bounded smoke gate passed, "
    "but the full GPU corpus remains not proven"
)
BACKENDS = ("cpu", "simd")


def now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat().replace("+00:00", "Z")


def git_revision() -> str:
    returncode, stdout, _stderr, timed_out = run_command(
        ["git", "rev-parse", "HEAD"],
        timeout_seconds=GIT_COMMAND_TIMEOUT_SECONDS,
    )
    if returncode == 0 and not timed_out:
        return stdout.strip()
    return "unknown"


def process_group_options() -> dict[str, Any]:
    """Return platform options that isolate one complete child lane."""

    if os.name == "nt":
        return {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
    return {"start_new_session": True}


def kill_process_group(process: subprocess.Popen[str]) -> None:
    """Hard-stop the child and all descendants in its isolated group."""

    if os.name == "nt":
        try:
            killer = subprocess.Popen(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                cwd=ROOT,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                **process_group_options(),
            )
        except OSError:
            process.kill()
            return
        try:
            killer.communicate(timeout=PROCESS_REAP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            killer.kill()
            killer.communicate(timeout=PROCESS_REAP_TIMEOUT_SECONDS)
        return
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def reap_timed_out_process(
    process: subprocess.Popen[str],
) -> tuple[str, str]:
    """Kill the isolated group and reap its direct child without waiting forever."""

    kill_process_group(process)
    try:
        return process.communicate(timeout=PROCESS_REAP_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        process.kill()
        try:
            return process.communicate(timeout=PROCESS_REAP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired as exc:
            raise RuntimeError(
                "timed-out process group did not exit after hard termination"
            ) from exc


def run_command(
    command: list[str],
    *,
    env: dict[str, str] | None = None,
    timeout_seconds: int,
) -> tuple[int, str, str, bool]:
    """Run one maintained make/npm lane with a bounded process lifetime."""

    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    if timeout_seconds <= 0:
        raise ValueError("command timeout must be positive")
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        env=merged_env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        **process_group_options(),
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        stdout, stderr = reap_timed_out_process(process)
        return 124, stdout, stderr, True
    return process.returncode, stdout, stderr, False


def parity_lane_timeout(
    backend: str, *, requested_seconds: int, smoke: bool
) -> int:
    """Return the outer deadline for one parity lane."""

    if requested_seconds <= 0:
        raise ValueError("parity lane timeout must be positive")
    if backend != "gpu":
        return requested_seconds
    hard_limit = GPU_SMOKE_TIMEOUT_SECONDS if smoke else GPU_FULL_TIMEOUT_SECONDS
    return min(requested_seconds, hard_limit)


def result_summary(path: Path) -> dict[str, Any] | None:
    value = result_document(path)
    if value is None:
        return None
    summary = value.get("summary")
    return summary if isinstance(summary, dict) else None


def result_document(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def gpu_adapter_unavailable(result: dict[str, Any] | None) -> bool:
    """Return whether a GPU smoke result stopped before kernel execution."""

    if result is None:
        return False
    marker = "GPU adapter not available"
    for error in result.get("infrastructure_errors", []):
        if isinstance(error, dict) and marker in str(error.get("message", "")):
            return True
    for comparison in result.get("comparisons", []):
        if not isinstance(comparison, dict):
            continue
        target = comparison.get("target", {})
        if not isinstance(target, dict):
            continue
        for observation in target.get("observations", []):
            if not isinstance(observation, dict):
                continue
            error = observation.get("error", {})
            if isinstance(error, dict) and marker in str(error.get("message", "")):
                return True
    return False


def lane_record(
    *,
    lane_id: str,
    kind: str,
    backend: str | None,
    command: list[str],
    status: str,
    returncode: int | None,
    artifact: Path | None = None,
    summary: dict[str, Any] | None = None,
    timed_out: bool = False,
    reason: str | None = None,
) -> dict[str, Any]:
    record: dict[str, Any] = {
        "lane_id": lane_id,
        "kind": kind,
        "backend": backend,
        "command": command,
        "status": status,
        "returncode": returncode,
        "timed_out": timed_out,
    }
    if artifact is not None:
        try:
            record["artifact"] = artifact.relative_to(ROOT).as_posix()
        except ValueError:
            record["artifact"] = artifact.as_posix()
    if summary is not None:
        record["summary"] = summary
    if reason is not None:
        record["reason"] = reason
    return record


def run_parity_lane(
    backend: str,
    *,
    output_dir: Path,
    timeout_seconds: int,
    smoke: bool = False,
) -> dict[str, Any]:
    artifact = output_dir / (
        f"parity-{backend}-smoke.json" if smoke else f"parity-{backend}.json"
    )
    if smoke:
        command = [
            "make",
            "migration-parity-case",
            f"MIGRATION_PARITY_CASE={GPU_SMOKE_CASE}",
            f"MIGRATION_PARITY_CASE_OUTPUT={artifact}",
        ]
    else:
        command = [
            "make",
            "migration-parity-test",
            f"MIGRATION_PARITY_OUTPUT={artifact}",
        ]
    artifact.unlink(missing_ok=True)
    lane_timeout = parity_lane_timeout(
        backend, requested_seconds=timeout_seconds, smoke=smoke
    )
    returncode, stdout, stderr, timed_out = run_command(
        command,
        env={"MIGRATION_TARGET_BACKEND": backend},
        timeout_seconds=lane_timeout,
    )
    result = result_document(artifact)
    summary = result.get("summary") if isinstance(result, dict) else None
    if not isinstance(summary, dict):
        summary = None
    adapter_unavailable = smoke and gpu_adapter_unavailable(result)
    if adapter_unavailable and not timed_out:
        status = "skipped"
    else:
        status = (
            "passed"
            if returncode == 0 and summary is not None and not timed_out
            else "failed"
        )
    if timed_out:
        stage = "GPU smoke" if smoke else f"{backend.upper()} parity"
        reason = f"bounded {stage} timeout after {lane_timeout}s"
    elif adapter_unavailable:
        reason = "GPU adapter unavailable; full GPU lane was not executed"
    elif returncode != 0:
        reason = (stderr or stdout).strip().replace("\n", " ")[-1000:]
    elif summary is None:
        reason = "validated parity artifact was not produced"
    else:
        reason = None
    record = lane_record(
        lane_id=f"parity-{backend}{'-smoke' if smoke else ''}",
        kind="python-py3-parity",
        backend=backend,
        command=command,
        status=status,
        returncode=returncode,
        artifact=artifact,
        summary=summary,
        timed_out=timed_out,
        reason=reason,
    )
    if status != "passed" and not smoke:
        record["output_tail"] = (stderr or stdout).strip().replace("\n", " ")[-1000:]
    return record


def run_js_lane(*, output_dir: Path, timeout_seconds: int) -> dict[str, Any]:
    del output_dir  # The JS package check has no result artifact to relocate.
    command = ["make", "test-wasm"]
    returncode, stdout, stderr, timed_out = run_command(
        command,
        env={
            "MIGRATION_WASM_NO_OPT": "1",
            "NPM_CONFIG_CACHE": "/tmp/pillow-rs-npm-cache",
        },
        timeout_seconds=timeout_seconds,
    )
    status = "passed" if returncode == 0 and not timed_out else "failed"
    reason = None
    if timed_out:
        reason = "bounded command timeout"
    elif returncode != 0:
        reason = (stderr or stdout).strip().replace("\n", " ")[-1000:]
    return lane_record(
        lane_id="js-wasm-package",
        kind="javascript-wasm-package",
        backend=None,
        command=command,
        status=status,
        returncode=returncode,
        timed_out=timed_out,
        reason=reason,
    )


def run_campaign(args: argparse.Namespace) -> int:
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output_dir = output.parent / "all-backends"
    output_dir.mkdir(parents=True, exist_ok=True)

    lanes: list[dict[str, Any]] = []
    started_at = now()
    for backend in BACKENDS:
        lanes.append(
            run_parity_lane(
                backend,
                output_dir=output_dir,
                timeout_seconds=args.timeout,
            )
        )

    smoke = run_parity_lane(
        "gpu",
        output_dir=output_dir,
        timeout_seconds=args.timeout,
        smoke=True,
    )
    lanes.append(smoke)
    if smoke["status"] == "passed" and args.gpu_full:
        lanes.append(
            run_parity_lane(
                "gpu",
                output_dir=output_dir,
                timeout_seconds=args.timeout,
            )
        )
    else:
        lanes.append(
            lane_record(
                lane_id="parity-gpu",
                kind="python-py3-parity",
                backend="gpu",
                command=["make", "migration-parity-test", "MIGRATION_TARGET_BACKEND=gpu"],
                status="skipped",
                returncode=None,
                reason=(
                    "GPU smoke gate did not pass; full GPU lane was not executed"
                    if smoke["status"] != "passed"
                    else GPU_FULL_DISABLED_REASON
                ),
            )
        )

    lanes.append(run_js_lane(output_dir=output_dir, timeout_seconds=args.timeout))

    required_failures = [lane for lane in lanes if lane["status"] == "failed"]
    status = "passed" if not required_failures else "failed"
    if (smoke["status"] != "passed" or not args.gpu_full) and status == "passed":
        status = "passed_with_gpu_skipped"
    result = {
        "schema": "migration-parity/all-backends-test-result@1",
        "status": status,
        "started_at": started_at,
        "finished_at": now(),
        "revision": git_revision(),
        "gpu_gate": {
            "case_id": GPU_SMOKE_CASE,
            "status": smoke["status"],
            "timeout_seconds": GPU_SMOKE_TIMEOUT_SECONDS,
        },
        "gpu_full_requested": args.gpu_full,
        "lanes": lanes,
    }
    output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if status in {"passed", "passed_with_gpu_skipped"} else 1


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    gpu_group = parser.add_mutually_exclusive_group()
    gpu_group.add_argument(
        "--gpu-full",
        dest="gpu_full",
        action="store_true",
        help="run bounded full GPU parity after the smoke gate (default)",
    )
    gpu_group.add_argument(
        "--no-gpu-full",
        dest="gpu_full",
        action="store_false",
        help="run only the GPU smoke gate and mark full GPU parity not proven",
    )
    parser.set_defaults(gpu_full=True)
    return parser.parse_args(argv)


if __name__ == "__main__":
    raise SystemExit(run_campaign(parse_args()))

#!/usr/bin/env python3
"""Measure fresh image-operation requests with a bounded host worker queue.

The standalone ``pillow-rs/transpose-throughput-diagnostic@1`` JSON schema
contains policy, source hashes, input/reference byte identities, and one child
result per mode and subject. Each child result contains runtime/binary identity
and queue-depth results. A queue result retains every window and every request:
monotonic start/end timestamps, dimensions/mode/byte count, thread-local backend
receipt, and exact comparison outcome. Summaries use completed images divided
by measured window wall time; they are not reciprocal serial request latency.

Each request is Image.frombytes(...).transpose(0).transpose(2).tobytes(). The
16 immutable source byte strings vary by frame; every request creates a fresh
image and graph. Input generation, live Pillow reference generation, and exact
output comparison happen outside timing. Fresh image construction, backend
locking, upload, readback, host bytes construction, request scheduling, and
receipt capture are included in window wall time. Returned outputs are retained
until the window ends, then every output is compared byte-for-byte with its live
Pillow reference. Persistent workers and backend caches are warmed; image/result
materialization caches are never reused across requests.

Policy is fixed for every subject: queue depths 1/2/4, 16 frames per window,
5 warmup windows, then 5 samples of 20 measured windows. ``--check-only`` checks
one window per depth and deliberately emits no timing summary. The ordinary
migration benchmark schemas, workloads, and policies are unchanged. This
diagnostic measures host queue concurrency, not simultaneous GPU kernels.

``--operation equalize`` uses the same completed-work window policy for fresh
``ImageOps.equalize`` calls in L/RGB. Its deterministic tile is reduced to 64
levels before the per-frame offset to exercise nonidentity LUTs at the default
size; reference metadata records whether each output changes. Equalize keeps
input dimensions and requires one public operation and four GPU passes. The
transpose default, stimulus, receipt rules, and timing policy are unchanged.

``--operation invert`` measures fresh ``ImageOps.invert`` calls in L/RGB
under the same policy. It uses the full-range tile, preserves dimensions,
and requires one public operation and one GPU dispatch per request.

``--operation grayscale`` uses that same policy, producing L bytes from
L/RGB inputs. GPU receipts must account for the complete multichannel input
upload as well as the smaller L readback.

``--operation convert`` explicitly converts L to RGB and RGB to L, including
fresh construction and terminal export. It does not use the default-mode copy.

``--operation putpixel`` updates the center pixel of each fresh image, then
exports the complete result. Construction, mutation and export remain timed.

``--operation transform`` uses one nearest affine transform with fractional
coefficients and explicit fill, retaining the input size. It exercises fresh
source selection across the full image and requires one native GPU dispatch.

``--operation add`` and ``--operation subtract`` use the same two-image policy
with default scale/offset. ``--operation multiply`` uses the same fresh pair
policy with Pillow's truncated byte product.

``--operation screen`` measures ``ImageChops.screen`` with the same fresh pair
and complete-output policy used by multiply.

``--operation blend`` measures ``ImageChops.blend`` at alpha 0.3. Each request
constructs two fresh images from frame j and frame (j+1) modulo 16, blends them,
and exports bytes. The GPU receipt must also account for the second image.

``--operation image-blend`` uses that fresh pair policy for module-level
``Image.blend``, including both image buffers in GPU transfer receipts.

``--operation alpha-composite`` uses that two-image policy with native LA/RGBA
inputs and ``Image.alpha_composite``; every request completes one composite.

``--operation contrast`` includes construction of ``ImageEnhance.Contrast``
from each fresh image and ``enhance(0.3)`` before exporting bytes. Base-image
construction and its host mean calculation are inside the measured boundary.

``--operation color`` includes construction of ``ImageEnhance.Color`` and its
observable converted base, then ``enhance(0.3)`` and terminal bytes.

``--operation solarize`` applies ``ImageOps.solarize(image, 128)`` to each
fresh L/RGB image before exporting its bytes.

Use ``make migration-parity-transpose-throughput`` to build the replacement
without overwriting the Pillow oracle, or invoke this script after build-parity.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import datetime as dt
import hashlib
import importlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import traceback
from typing import Any

import run_migration_parity as parity


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "pillow-rs/transpose-throughput-diagnostic@1"
SUBJECTS = ("Pillow", "python-cpu", "python-simd", "python-gpu")
TRANSFORM_DATA = [0.87, 0.21, -1.25, -0.16, 1.13, 0.5]
DEPTHS = (1, 2, 4)
FRAMES = 16
WARMUPS = 5
ITERATIONS = 20
SAMPLES = 5


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_identity(path: Path) -> dict[str, Any]:
    path = path.resolve()
    return {"path": str(path), "size": path.stat().st_size,
            "sha256": digest(path.read_bytes())}


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=path.parent,
                                         prefix=f".{path.name}.", delete=False) as stream:
            temporary = Path(stream.name)
            json.dump(value, stream, separators=(",", ":"), allow_nan=False)
            stream.write("\n")
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def source_identity() -> dict[str, Any]:
    paths = {ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "Makefile",
             Path(__file__).resolve(), ROOT / "scripts/run_migration_parity.py",
             ROOT / "pillow-rs/Cargo.toml", ROOT / "pillow-rs-py/Cargo.toml"}
    for relative in ("pillow-rs/src", "pillow-rs-py/src", "pillow-rs-py/python"):
        paths.update(path for path in (ROOT / relative).rglob("*")
                     if path.is_file() and path.suffix in {".rs", ".wgsl", ".py"})
    files = {str(path.relative_to(ROOT)): digest(path.read_bytes())
             for path in sorted(paths) if path.is_file()}
    return {"revision": parity.git_revision(), "dirty": parity.git_dirty(),
            "files": files, "combined_sha256": digest(json.dumps(files, sort_keys=True).encode())}


def runtime_files(subject: str) -> dict[str, Any]:
    modules = ["PIL", "PIL.Image"]
    modules += ["PIL._imaging"] if subject == "Pillow" else ["pillow_rs", "pillow_rs._core"]
    result = {name: file_identity(Path(importlib.import_module(name).__file__))
              for name in modules}
    result["python_executable"] = file_identity(Path(sys.executable))
    return result


def stats(values: list[int | float]) -> dict[str, float | int] | None:
    if not values:
        return None
    ordered = sorted(values)
    return {"count": len(values), "min": ordered[0], "median": statistics.median(values),
            "mean": statistics.mean(values), "p95": ordered[math.ceil(len(values) * .95) - 1],
            "max": ordered[-1], "standard_deviation": statistics.pstdev(values)}


def patterned_frames(mode: str, size: tuple[int, int], directory: Path, operation: str = "transpose") -> list[dict[str, Any]]:
    """Fixed input generator; no reference/target output influences these bytes."""
    length = size[0] * size[1] * len(mode)
    tile = bytes((73 * index + 11 * (index // 17) + 29) & 255 for index in range(8192))
    if operation == "equalize":
        tile = bytes(value // 4 for value in tile)
    base = (tile * ((length + len(tile) - 1) // len(tile)))[:length]
    frames = []
    for frame_id in range(FRAMES):
        table = bytes((value + 41 * frame_id) & 255 for value in range(256))
        data = base.translate(table)
        path = directory / f"input-{frame_id:02d}.bin"
        path.write_bytes(data)
        frames.append({"frame_id": frame_id, "path": str(path), "length": len(data),
                       "sha256": digest(data)})
    if len({frame["sha256"] for frame in frames}) != FRAMES:
        raise RuntimeError("changing-input corpus contains duplicate frames")
    return frames


def result_size(plan: dict[str, Any]) -> list[int]:
    return list(reversed(plan["size"]) if plan.get("operation", "transpose") == "transpose" else plan["size"])


def result_mode(plan: dict[str, Any]) -> str:
    if plan.get("operation") == "convert":
        return "RGB" if plan["mode"] == "L" else "L"
    return "L" if plan.get("operation") == "grayscale" else plan["mode"]


def request(image_api: Any, core: Any, plan: dict[str, Any], data: bytes,
            frame_id: int, request_id: int) -> dict[str, Any]:
    if core is not None:
        core.take_pipeline_telemetry()  # drain this worker's previous receipt
    started = time.perf_counter_ns()
    try:
        image = image_api.frombytes(plan["mode"], tuple(plan["size"]), data)
        if plan.get("operation") == "equalize":
            image = plan["imageops_api"].equalize(image)
        elif plan.get("operation") == "invert":
            image = plan["imageops_api"].invert(image)
        elif plan.get("operation") == "grayscale":
            image = plan["imageops_api"].grayscale(image)
        elif plan.get("operation") == "convert":
            image = image.convert(result_mode(plan))
        elif plan.get("operation") == "putpixel":
            value = {"L": 173, "LA": (173, 127), "RGB": (17, 83, 149),
                     "RGBA": (17, 83, 149, 211)}[plan["mode"]]
            image.putpixel((plan["size"][0] // 2, plan["size"][1] // 2), value)
        elif plan.get("operation") == "solarize":
            image = plan["imageops_api"].solarize(image, 128)
        elif plan.get("operation") in ("contrast", "color"):
            enhancer = plan["imageenhance_api"].Contrast if plan["operation"] == "contrast" else plan["imageenhance_api"].Color
            image = enhancer(image).enhance(0.3)
        elif plan.get("operation") == "transform":
            fill = 173 if plan["mode"] == "L" else (17, 83, 149)
            image = image.transform(tuple(plan["size"]), 0, TRANSFORM_DATA,
                                    resample=0, fillcolor=fill)
        elif plan.get("operation") in ("blend", "image-blend", "add", "subtract", "multiply", "screen", "alpha-composite"):
            other_data = plan["pair_inputs"][(frame_id + 1) % len(plan["pair_inputs"])]
            other = image_api.frombytes(plan["mode"], tuple(plan["size"]), other_data)
            image = (image_api.alpha_composite(image, other)
                     if plan["operation"] == "alpha-composite"
                     else image_api.blend(image, other, 0.3)
                     if plan["operation"] == "image-blend"
                     else plan["imagechops_api"].blend(image, other, 0.3)
                     if plan["operation"] == "blend"
                     else plan["imagechops_api"].add(image, other)
                     if plan["operation"] == "add"
                     else plan["imagechops_api"].subtract(image, other)
                     if plan["operation"] == "subtract"
                     else plan["imagechops_api"].screen(image, other)
                     if plan["operation"] == "screen"
                     else plan["imagechops_api"].multiply(image, other))
        else:
            image = image.transpose(0).transpose(2)
        if core is not None:
            parity.lock_target_image_pipeline(image)
        output = image.tobytes()
        ended = time.perf_counter_ns()
        receipt = core.take_pipeline_telemetry() if core is not None else None
        return {"request_id": request_id, "frame_id": frame_id,
                "thread_id": threading.get_native_id(), "started_ns": started,
                "ended_ns": ended, "mode": image.mode, "size": list(image.size),
                "byte_count": len(output), "receipt": receipt, "output": output,
                "status": "completed"}
    except Exception as error:
        ended = time.perf_counter_ns()
        receipt = core.take_pipeline_telemetry() if core is not None else None
        return {"request_id": request_id, "frame_id": frame_id,
                "thread_id": threading.get_native_id(), "started_ns": started,
                "ended_ns": ended, "receipt": receipt, "status": "failed",
                "error": f"{type(error).__name__}: {error}"}


def receipt_error(subject: str, receipt: Any, byte_count: int, operation: str = "transpose",
                  input_byte_count: int | None = None) -> str | None:
    if subject == "Pillow":
        return None
    backend = subject.removeprefix("python-")
    if not isinstance(receipt, dict):
        return "missing native backend receipt (cached results are not admitted)"
    if receipt.get("requested_backend") != backend or receipt.get("actual_backend") != backend:
        return "requested/actual backend does not match isolated subject"
    if receipt.get("fallback_reason") is not None:
        return "unexpected backend fallback"
    expected_operations = 2 if operation == "transpose" else 1
    if receipt.get("operation_count") != expected_operations:
        return f"receipt does not contain {expected_operations} public {operation} operation(s)"
    if backend == "gpu":
        resource = receipt.get("resource") or {}
        expected_dispatches = 4 if operation == "equalize" else 1
        if receipt.get("dispatch_count") != expected_dispatches:
            return f"GPU receipt does not contain {expected_dispatches} {operation} dispatch(es)"
        upload_bytes = byte_count if input_byte_count is None else input_byte_count
        if resource.get("upload_bytes", 0) < upload_bytes or resource.get("readback_bytes", 0) < byte_count:
            return "GPU receipt does not account for a complete upload and readback"
        if operation in ("blend", "image-blend", "add", "subtract", "multiply", "screen", "alpha-composite", "contrast", "color") and resource.get("auxiliary_bytes", 0) < byte_count:
            return "GPU binary-operation receipt does not account for the second image"
    return None


def observed_overlap(records: list[dict[str, Any]]) -> int:
    events = [(record["started_ns"], 1) for record in records]
    events += [(record["ended_ns"], -1) for record in records]
    active = peak = 0
    for _, change in sorted(events):
        active += change
        peak = max(peak, active)
    return peak


def window(executor: ThreadPoolExecutor, depth: int, image_api: Any, core: Any,
           plan: dict[str, Any], inputs: list[bytes], references: list[bytes],
           reference_metadata: list[dict[str, Any]], ordinal: int,
           warmup: bool, subject: str) -> dict[str, Any]:
    ready = threading.Event()

    def lane(lane_id: int) -> list[dict[str, Any]]:
        ready.wait()
        return [request(image_api, core, plan, inputs[index], index,
                        ordinal * FRAMES + index)
                for index in range(lane_id, FRAMES, depth)]

    # Include worker submission, complete public requests and collection of
    # their TLS receipts. No byte comparison occurs until every lane returns.
    started = time.perf_counter_ns()
    futures = [executor.submit(lane, lane_id) for lane_id in range(depth)]
    ready.set()
    records = [record for future in futures for record in future.result()]
    ended = time.perf_counter_ns()
    records.sort(key=lambda record: record["request_id"])
    completed = sum(record["status"] == "completed" for record in records)
    verification_started = time.perf_counter_ns()
    for record in records:
        output = record.pop("output", None)
        if record["status"] != "completed":
            record["exact_match"] = False
            continue
        frame_id = record["frame_id"]
        expected = references[frame_id]
        same = (record["mode"] == result_mode(plan)
                and record["size"] == result_size(plan)
                and isinstance(output, bytes) and output == expected)
        record["exact_match"] = same
        record["reference_sha256"] = reference_metadata[frame_id]["sha256"]
        error = receipt_error(subject, record["receipt"], len(expected),
                              plan.get("operation", "transpose"), plan["frames"][frame_id]["length"])
        if not same:
            error = "output bytes, mode or dimensions differ from live Pillow"
            if isinstance(output, bytes):
                mismatch = next((i for i, (a, b) in enumerate(zip(output, expected)) if a != b), None)
                record["first_mismatching_byte"] = mismatch
                record["actual_sha256"] = digest(output)
        if error:
            record["status"] = "failed"
            record["error"] = error
    verification_ns = time.perf_counter_ns() - verification_started
    valid = (len(records) == FRAMES and completed == FRAMES
             and all(record["status"] == "completed" for record in records))
    return {"ordinal": ordinal, "warmup": warmup, "started_ns": started,
            "ended_ns": ended, "elapsed_ns": ended - started,
            "completed_frames": completed, "verified_frames": sum(r["exact_match"] for r in records),
            "status": "completed" if valid else "failed",
            "host_request_overlap_peak": observed_overlap(records),
            "verification_ns_outside_timing": verification_ns, "requests": records}


def summarize_queue(windows: list[dict[str, Any]], check_only: bool) -> dict[str, Any] | None:
    measured = [item for item in windows if not item["warmup"]]
    if check_only or len(measured) != ITERATIONS * SAMPLES or any(item["status"] != "completed" for item in windows):
        return None
    samples = []
    for index in range(SAMPLES):
        group = measured[index * ITERATIONS:(index + 1) * ITERATIONS]
        elapsed = sum(item["elapsed_ns"] for item in group)
        count = sum(item["completed_frames"] for item in group)
        samples.append({"sample": index, "completed_frames": count, "window_elapsed_ns": elapsed,
                        "images_per_second": count * 1_000_000_000 / elapsed})
    total_ns = sum(item["elapsed_ns"] for item in measured)
    total_frames = sum(item["completed_frames"] for item in measured)
    return {"samples": samples, "sample_images_per_second": stats([s["images_per_second"] for s in samples]),
            "window_elapsed_ns": stats([item["elapsed_ns"] for item in measured]),
            "request_elapsed_ns": stats([r["ended_ns"] - r["started_ns"] for item in measured for r in item["requests"]]),
            "measured_completed_frames": total_frames, "measured_window_elapsed_ns": total_ns,
            "aggregate_images_per_second": total_frames * 1_000_000_000 / total_ns}


def child(args: argparse.Namespace) -> int:
    plan = json.loads(args.plan.read_text())
    subject = args.child_subject
    identity = parity.side_identity("source" if subject == "Pillow" else "target")
    image_api = importlib.import_module("PIL.Image")
    if plan.get("operation") in ("equalize", "invert", "grayscale", "solarize"):
        plan["imageops_api"] = importlib.import_module("PIL.ImageOps")
    if plan.get("operation") in ("blend", "image-blend", "add", "subtract", "multiply", "screen", "alpha-composite"):
        plan["imagechops_api"] = importlib.import_module("PIL.ImageChops")
    if plan.get("operation") in ("contrast", "color"):
        plan["imageenhance_api"] = importlib.import_module("PIL.ImageEnhance")
    core = None if subject == "Pillow" else importlib.import_module("pillow_rs._core")
    if core is not None:
        core.set_pipeline_telemetry(True)  # once per process, never toggled by workers
    binaries = runtime_files(subject)
    inputs = [Path(frame["path"]).read_bytes() for frame in plan["frames"]]
    if plan.get("operation") in ("blend", "image-blend", "add", "subtract", "multiply", "screen", "alpha-composite"):
        plan["pair_inputs"] = inputs
    for frame, data in zip(plan["frames"], inputs):
        if len(data) != frame["length"] or digest(data) != frame["sha256"]:
            raise RuntimeError("input bytes differ from the declared stimulus")
    reference_path = Path(plan["reference_manifest"])
    if subject == "Pillow":
        metadata = []
        for frame_id, data in enumerate(inputs):
            result = request(image_api, None, plan, data, frame_id, frame_id)
            if result["status"] != "completed":
                raise RuntimeError(f"live Pillow reference failed: {result}")
            output = result.pop("output")
            if result["mode"] != result_mode(plan) or result["size"] != result_size(plan):
                raise RuntimeError("unexpected live Pillow output mode/dimensions")
            path = reference_path.parent / f"reference-{frame_id:02d}.bin"
            path.write_bytes(output)
            metadata.append({"frame_id": frame_id, "path": str(path), "length": len(output),
                             "differs_from_input": output != data,
                             "mode": result["mode"], "size": result["size"], "sha256": digest(output)})
        write_json(reference_path, metadata)
    metadata = json.loads(reference_path.read_text())
    if (len(metadata) != FRAMES
            or [item["frame_id"] for item in metadata] != list(range(FRAMES))
            or any(item["mode"] != result_mode(plan) or item["size"] != result_size(plan)
                   for item in metadata)):
        raise RuntimeError("live Pillow reference inventory is incomplete or incompatible")
    references = [Path(item["path"]).read_bytes() for item in metadata]
    if any(len(data) != item["length"] or digest(data) != item["sha256"]
           for item, data in zip(metadata, references)):
        raise RuntimeError("live Pillow reference artifact changed")
    result: dict[str, Any] = {"subject": subject, "status": "completed", "identity": identity,
                              "runtime_files": binaries, "argv": sys.argv, "pid": os.getpid(),
                              "started_at": utc_now(), "reference": metadata, "queues": []}
    for depth in DEPTHS:
        windows = []
        with ThreadPoolExecutor(max_workers=depth, thread_name_prefix=f"{plan.get('operation', 'transpose')}-stream") as executor:
            barrier = threading.Barrier(depth + 1)
            warm_threads = [executor.submit(barrier.wait) for _ in range(depth)]
            barrier.wait()
            for future in warm_threads:
                future.result()
            count = 1 if args.check_only else WARMUPS + ITERATIONS * SAMPLES
            for ordinal in range(count):
                item = window(executor, depth, image_api, core, plan, inputs, references,
                              metadata, ordinal, args.check_only or ordinal < WARMUPS, subject)
                windows.append(item)
                if item["status"] != "completed":
                    result["status"] = "failed"
                    break
        result["queues"].append({"host_queue_depth": depth, "windows": windows,
                                  "summary": summarize_queue(windows, args.check_only)})
        if result["status"] != "completed":
            break
    result["finished_at"] = utc_now()
    result["runtime_files_unchanged"] = runtime_files(subject) == binaries
    if not result["runtime_files_unchanged"]:
        result["status"] = "failed"
    write_json(args.child_output, result)
    return 0 if result["status"] == "completed" else 1


def child_environment(subject: str) -> dict[str, str]:
    environment = os.environ.copy()
    target = str((ROOT / "pillow-rs-py/python").resolve())
    paths = [value for value in environment.get("PYTHONPATH", "").split(os.pathsep)
             if value and str(Path(value).resolve()) != target]
    if subject != "Pillow":
        paths.insert(0, target)
    if paths:
        environment["PYTHONPATH"] = os.pathsep.join(paths)
    else:
        environment.pop("PYTHONPATH", None)
    environment["MIGRATION_TARGET_BACKEND"] = subject.removeprefix("python-") if subject != "Pillow" else "cpu"
    environment["MIGRATION_STRICT_TARGET_BACKEND"] = "1"
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    for name in ("MIGRATION_GPU_WGSL_COVERAGE_OUTPUT", "LLVM_PROFILE_FILE", "COVERAGE_PROCESS_START"):
        environment.pop(name, None)
    return environment


def comparisons(subjects: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rows = []
    for depth in DEPTHS:
        values = {}
        for subject in subjects:
            if subject.get("status") != "completed":
                continue
            queue = next((q for q in subject["queues"] if q["host_queue_depth"] == depth), None)
            if queue and queue["summary"]:
                values[subject["subject"]] = queue["summary"]["aggregate_images_per_second"]
        for baseline, target in (("Pillow", "python-cpu"), ("Pillow", "python-simd"),
                                 ("Pillow", "python-gpu"), ("python-simd", "python-gpu")):
            if baseline in values and target in values:
                rows.append({"host_queue_depth": depth, "baseline": baseline, "target": target,
                             "completed_frame_throughput_ratio": values[target] / values[baseline]})
    return rows


def run(args: argparse.Namespace) -> int:
    if min(args.size) < 1:
        raise ValueError("--size requires positive dimensions")
    operation = args.operation
    defaults = (["RGB", "RGBA"] if operation == "transpose" else
                ["LA", "RGBA"] if operation == "alpha-composite" else ["L", "RGB"])
    modes = list(dict.fromkeys(args.mode or defaults))
    if operation in ("equalize", "invert", "grayscale", "convert", "transform") and any(mode not in ("L", "RGB") for mode in modes):
        raise ValueError(f"{operation} throughput supports L/RGB input")
    if operation == "alpha-composite" and any(mode not in ("LA", "RGBA") for mode in modes):
        raise ValueError("alpha-composite throughput supports LA/RGBA input")
    before = source_identity()
    result: dict[str, Any] = {
        "schema": SCHEMA if operation == "transpose" else f"pillow-rs/{operation}-throughput-diagnostic@1", "status": "completed", "started_at": utc_now(), "argv": sys.argv,
        "environment": {"platform": platform.platform(), "machine": platform.machine(),
                        "python": sys.version, "cpu_count": os.cpu_count(),
                        "RAYON_NUM_THREADS": os.environ.get("RAYON_NUM_THREADS")},
        "source_before": before, "check_only": args.check_only,
        "policy": {"host_queue_depths": list(DEPTHS), "frames_per_window": FRAMES,
                   "warmup_windows": WARMUPS, "measurement_iterations_per_sample": ITERATIONS,
                   "samples": SAMPLES, "operation": operation, "methods": [0, 2] if operation == "transpose" else [],
                   "contrast_factor": 0.3 if operation in ("contrast", "color") else None,
                   "solarize_threshold": 128 if operation == "solarize" else None,
                   "affine_coefficients": TRANSFORM_DATA if operation == "transform" else None,
                   "boundary": ("two fresh frombytes images, blend(alpha=0.3), terminal bytes, worker scheduling and receipt capture"
                                if operation in ("blend", "image-blend") else "two fresh frombytes images, add(scale=1, offset=0), terminal bytes, worker scheduling and receipt capture"
                                if operation == "add" else "two fresh frombytes images, subtract(scale=1, offset=0), terminal bytes, worker scheduling and receipt capture"
                                if operation == "subtract" else "two fresh frombytes images, multiply, terminal bytes, worker scheduling and receipt capture"
                                if operation == "multiply" else "two fresh frombytes images, screen, terminal bytes, worker scheduling and receipt capture"
                                if operation == "screen" else "two fresh frombytes images, alpha_composite, terminal bytes, worker scheduling and receipt capture"
                                if operation == "alpha-composite" else "fresh frombytes image, Contrast constructor including host mean and base allocation, enhance(0.3), terminal bytes, worker scheduling and receipt capture"
                                if operation == "contrast" else "fresh frombytes image, Color constructor and saved base, enhance(0.3), terminal bytes, worker scheduling and receipt capture"
                                if operation == "color" else "fresh frombytes through terminal bytes, worker scheduling and receipt capture"),
                   "comparison": "every output exactly matches live Pillow outside measured window",
                   "concurrency_claim": "host worker requests; simultaneous GPU kernels are not asserted",
                   "output_retention": "all outputs retained until window completion",
                   "input_generator": "8192-byte tile (73*i+11*(i//17)+29)%256; "
                       + ("divide tile values by 4; " if operation == "equalize" else "")
                       + "frame j adds 41*j modulo 256"
                       + ("; second image uses frame (j+1) modulo 16" if operation in ("blend", "image-blend", "add", "subtract", "multiply", "screen", "alpha-composite") else ""),
                   "build_profile": "expected release via build-parity; binary identity recorded separately",
                   "check_only_policy": "one verification window per depth; no performance summary",
                   "cache_state": "warm workers/backend; fresh image and graph per request"},
        "cases": [], "errors": [],
    }
    with tempfile.TemporaryDirectory(prefix=f"pillow-{operation}-throughput-") as temporary:
        root = Path(temporary)
        for mode in modes:
            directory = root / mode
            directory.mkdir()
            frames = patterned_frames(mode, tuple(args.size), directory, operation)
            plan = {"operation": operation, "mode": mode, "size": args.size, "frames": frames,
                    "reference_manifest": str(directory / "references.json")}
            plan_path = directory / "plan.json"
            write_json(plan_path, plan)
            case = {"mode": mode, "size": args.size, "input_frames": frames, "subjects": []}
            for subject in SUBJECTS:
                output = directory / f"{subject}.json"
                command = [sys.executable, str(Path(__file__).resolve()), "--child-subject", subject,
                           "--plan", str(plan_path), "--child-output", str(output)]
                if args.check_only:
                    command.append("--check-only")
                process = subprocess.Popen(command, cwd=ROOT, env=child_environment(subject),
                                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                                           **parity.process_group_options())
                try:
                    stdout, stderr = process.communicate(timeout=args.timeout)
                except subprocess.TimeoutExpired:
                    stdout, stderr = parity.reap_timed_out_process(process)
                    stderr += f"\nsubject exceeded {args.timeout}s timeout"
                child_result = json.loads(output.read_text()) if output.exists() else {
                    "subject": subject, "status": "failed", "queues": []}
                child_result["process"] = {"argv": command, "returncode": process.returncode,
                                             "stdout": stdout, "stderr": stderr}
                if process.returncode != 0:
                    child_result["status"] = "failed"
                case["subjects"].append(child_result)
                if child_result["status"] != "completed":
                    result["status"] = "failed"
                    if subject == "Pillow":
                        break  # no target may run without a complete live oracle
            case["comparisons"] = comparisons(case["subjects"])
            result["cases"].append(case)
    after = source_identity()
    result["source_after"] = after
    result["source_unchanged"] = before == after
    runtime_sets: dict[str, set[str]] = {}
    for case in result["cases"]:
        for subject in case["subjects"]:
            files = subject.get("runtime_files")
            if files:
                implementation = "Pillow" if subject["subject"] == "Pillow" else "pillow-rs"
                runtime_sets.setdefault(implementation, set()).add(
                    digest(json.dumps(files, sort_keys=True).encode()))
    result["runtime_consistent_across_processes"] = all(len(values) == 1 for values in runtime_sets.values())
    if not result["runtime_consistent_across_processes"]:
        result["status"] = "failed"
        result["errors"].append("runtime/binary files differed between isolated subject processes")
    if before != after:
        result["status"] = "failed"
        result["errors"].append("source changed during diagnostic execution")
    if result["errors"]:
        for case in result["cases"]:
            case["comparisons"] = []
    result["finished_at"] = utc_now()
    write_json(args.output.resolve(), result)
    print(f"{result['status']}: {args.output.resolve()}")
    return 0 if result["status"] == "completed" else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--operation", choices=("transpose", "equalize", "invert", "grayscale", "convert", "putpixel", "blend", "image-blend", "add", "subtract", "multiply", "screen", "transform", "alpha-composite", "contrast", "color", "solarize"), default="transpose")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--mode", action="append", choices=("L", "LA", "RGB", "RGBA"), help="select input mode(s); defaults depend on operation")
    parser.add_argument("--size", nargs=2, type=int, default=[1024, 1024], metavar=("WIDTH", "HEIGHT"))
    parser.add_argument("--timeout", type=int, default=900, help="deadline per isolated subject process")
    parser.add_argument("--check-only", action="store_true", help="verify one complete window per queue depth; emit no timing summary")
    parser.add_argument("--child-subject", choices=SUBJECTS, help=argparse.SUPPRESS)
    parser.add_argument("--plan", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--child-output", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.output is None:
        args.output = ROOT / f"build/migration-parity/{args.operation}-throughput.json"
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    try:
        return child(args) if args.child_subject else run(args)
    except Exception:
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

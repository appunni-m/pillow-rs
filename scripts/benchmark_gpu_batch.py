#!/usr/bin/env python3
"""Completed-work GPU batching diagnostic, isolated live Pillow baseline.

Fresh inputs/graphs every request; warm shader caches, no result cache reuse.
Includes construction, upload, dispatch, readback, Image and terminal byte export.
Reference construction, timing aggregation and exact byte comparisons are outside
measured windows. No host thread pools or JSON evidence files are created. The
BoxBlur lane measures an explicit one-operation GPU batch against strict serial
CPU/SIMD execution and Pillow. The grayscale lane measures
ImageOps.grayscale as a one-operation graph. The alpha-composite-mirror lane
measures the fused RGBA public pipeline against all five subjects. JSON evidence
is emitted to stdout only when requested with --json. The F Resize lane measures
one native F bicubic resize with exact output comparison and CPU materialization.
"""
from __future__ import annotations
import argparse
from array import array
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SUBJECTS = ("Pillow", "CPU", "SIMD", "GPU eager", "GPU queued")


def child(subject, size, height, mode, images, samples, operation="median", max_jobs=16, max_in_flight=4, alpha_input_pattern="uniform"):
    from importlib.metadata import version as distribution_version
    import PIL
    from PIL import Image, ImageOps, ImageFilter
    channels = {"L": 1, "RGB": 3, "CMYK": 4, "RGBA": 4, "F": 4}[mode]
    length = size * height * channels
    import random
    if operation == "alpha-composite-mirror" and alpha_input_pattern == "noise":
        sources = [
            (
                random.Random(index + size * 31 + height * 17 + 1).randbytes(length),
                random.Random(index + size * 31 + height * 17 + 2).randbytes(length),
            )
            for index in range(images)
        ]
    elif operation == "alpha-composite-mirror":
        sources = []
    elif operation == "resize":
        sources = []
        for index in range(images):
            rng = random.Random(index + size * 31 + height * 17 + 4)
            values = array("f", (rng.uniform(-1.0, 1.0) for _ in range(size * height)))
            if values.itemsize != 4:
                raise RuntimeError("F resize diagnostic requires 32-bit float storage")
            if sys.byteorder != "little":
                values.byteswap()
            sources.append(values.tobytes())
    else:
        sources = [
            random.Random(index + size * 31 + height * 17 + channels).randbytes(length)
            for index in range(images)
        ]

    def graph(index):
        if operation == "alpha-composite-mirror":
            if alpha_input_pattern == "uniform":
                destination = Image.new("RGBA", (size, height), (30, 60, 90, 64))
                source = Image.new("RGBA", (size, height), (200, 150, 100, 192))
            else:
                destination = Image.frombytes("RGBA", (size, height), sources[index][0])
                source = Image.frombytes("RGBA", (size, height), sources[index][1])
            result = Image.alpha_composite(destination, source)
            result = ImageOps.mirror(result)
        else:
            result = Image.frombytes(mode, (size, height), sources[index])
        if operation == "boxblur":
            result = result.filter(ImageFilter.BoxBlur(1))
        elif operation == "grayscale":
            result = ImageOps.grayscale(result)
        elif operation == "resize":
            result = result.resize((max(1, size // 2), max(1, height // 2)), Image.Resampling.BICUBIC)
        elif operation == "median":
            result = ImageOps.invert(result).filter(ImageFilter.MedianFilter(3))
        if subject in ("CPU", "SIMD"):
            # A single active backend plus a per-image lock makes unsupported
            # contexts fail instead of silently entering CPU fallback.
            rust = __import__("pillow_rs")
            result._rust_image.lock_active_backend()
        return result

    core = None
    if subject in ("CPU", "SIMD"):
        import pillow_rs
        core = pillow_rs._core
        for backend in ("cpu", "simd", "gpu"):
            pillow_rs.disable_backend(backend)
        pillow_rs.enable_backend(subject.lower())

    executor = None
    if subject.startswith("GPU"):
        from PIL import GpuBatchExecutor
        executor = GpuBatchExecutor(queue=subject == "GPU queued", max_jobs=max_jobs, max_in_flight=max_in_flight)

    def window():
        if executor is None:
            return [graph(i).tobytes() for i in range(images)]
        output = {}
        for result in executor.run((i, graph(i)) for i in range(images)):
            output[result.input_key] = result.image.tobytes()
        return [output[i] for i in range(images)]
    times, digest = [], None
    receipt = None
    try:
        if core is not None:
            core.take_pipeline_telemetry()
            core.set_pipeline_telemetry(True)
            proof = graph(0).tobytes()
            receipt = core.take_pipeline_telemetry()
            core.set_pipeline_telemetry(False)
            assert receipt and receipt["actual_backend"] == subject.lower(), receipt
            assert receipt["requested_backend"] == subject.lower(), receipt
            assert receipt["fallback_reason"] is None, receipt
            assert proof, "backend proof materialized no bytes"

        for _ in range(2): window()
        for _ in range(samples):
            started = time.perf_counter_ns()
            output = window()
            elapsed = time.perf_counter_ns() - started
            actual = [hashlib.sha256(image).hexdigest() for image in output]
            if digest is not None: assert actual == digest, "fresh request outputs changed"
            digest = actual
            times.append(elapsed / 1e9)
        stats = executor.stats if executor else None
        if stats:
            assert stats["gpu_bytes"] == stats["host_bytes"] == stats["live_jobs"] == 0, stats
            assert stats["submitted_jobs"] == (samples + 2) * images, stats
            dispatches_per_job = 1 if operation in ("boxblur", "grayscale", "alpha-composite-mirror") else 2
            assert stats["dispatches"] == dispatches_per_job * stats["submitted_jobs"], stats
            inputs_per_job = 2 if operation == "alpha-composite-mirror" else 1
            upload_bytes_per_job = length * inputs_per_job
            expected_bytes = stats["submitted_jobs"] * upload_bytes_per_job
            assert stats["upload_bytes"] == expected_bytes, stats
            output_channels = 1 if operation == "grayscale" else channels
            output_size = (
                (max(1, size // 2), max(1, height // 2))
                if operation == "resize"
                else (size, height)
            )
            expected_output_bytes = stats["submitted_jobs"] * output_size[0] * output_size[1] * output_channels
            assert stats["readback_bytes"] == expected_output_bytes, stats
            if subject == "GPU queued":
                assert stats["largest_submission"] > 1, stats
                assert stats["submissions"] < stats["submitted_jobs"], stats
            else:
                assert stats["largest_submission"] == 1, stats
        return {"seconds": statistics.median(times), "sample_seconds": times,
                "digests": digest, "stats": stats,
                "backend_receipt": receipt,
                "pillow_version": PIL.__version__, "pillow_module": PIL.__file__,
                "pillow_distribution_version": distribution_version("Pillow"),
                "workload": {"operation": operation, "mode": mode, "size": [size, height],
                             "images_per_window": images, "samples": samples,
                             "parameters": (
                                 {"filter": "BICUBIC",
                                  "destination_size": [max(1, size // 2), max(1, height // 2)],
                                  "input_pattern": "deterministic finite f32 noise"}
                                 if operation == "resize" else None
                             ),
                             "alpha_input_pattern": (
                                 alpha_input_pattern if operation == "alpha-composite-mirror" else None
                             ),
                             "subject": subject}}
    finally:
        if executor: executor.close()


def isolated(subject, size, height, mode, images, samples, operation, max_jobs=16, max_in_flight=4, alpha_input_pattern="uniform"):
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    if subject != "Pillow": env["PYTHONPATH"] = str(ROOT / "pillow-rs-py/python")
    process = subprocess.run([sys.executable, str(Path(__file__).resolve()), "--subject", subject,
        "--operation", operation, "--size", str(size), "--height", str(height), "--mode", mode, "--images", str(images), "--samples", str(samples), "--max-jobs", str(max_jobs), "--max-in-flight", str(max_in_flight), "--alpha-input-pattern", alpha_input_pattern],
        env=env, capture_output=True, text=True)
    if process.returncode: raise RuntimeError(process.stderr)
    return json.loads(process.stdout)


def pillow_identity(subject):
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    if subject != "Pillow": env["PYTHONPATH"] = str(ROOT / "pillow-rs-py/python")
    process = subprocess.run(
        [sys.executable, "-c", "from importlib.metadata import version; import json, PIL; print(json.dumps({'distribution_version': version('Pillow'), 'module_version': PIL.__version__, 'module': PIL.__file__}))"],
        env=env, capture_output=True, text=True,
    )
    if process.returncode: raise RuntimeError(process.stderr)
    return json.loads(process.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--subject", choices=SUBJECTS)
    parser.add_argument(
        "--operation",
        choices=("median", "boxblur", "grayscale", "alpha-composite-mirror", "resize"),
        default="median",
    )
    parser.add_argument("--size", type=int, default=256)
    parser.add_argument("--height", type=int)
    parser.add_argument("--mode", choices=("L", "RGB", "CMYK", "RGBA", "F"), default="L")
    parser.add_argument("--images", type=int, default=32)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--modes", nargs="+", choices=("L", "RGB", "CMYK", "RGBA", "F"))
    parser.add_argument("--sizes", nargs="+", type=int, default=(256, 1024))
    parser.add_argument("--max-jobs", type=int, default=16)
    parser.add_argument("--max-in-flight", type=int, default=4)
    parser.add_argument(
        "--alpha-input-pattern",
        choices=("uniform", "noise"),
        default="uniform",
        help="input content for alpha-composite-mirror (uniform or deterministic noise)",
    )
    parser.add_argument("--json", action="store_true",
                        help="emit machine-readable timing, parity, backend, and batch receipts")
    args = parser.parse_args()
    if args.images < 1 or args.samples < 1: parser.error("images/samples must be positive")
    default_modes = (
        ("RGBA",) if args.operation == "alpha-composite-mirror"
        else ("F",) if args.operation == "resize"
        else ("L", "RGB")
    )
    selected_modes = (
        (args.mode,)
        if args.subject
        else tuple(args.modes) if args.modes else default_modes
    )
    if args.operation == "alpha-composite-mirror" and selected_modes != ("RGBA",):
        parser.error("alpha-composite-mirror requires RGBA")
    if "RGBA" in selected_modes and args.operation != "alpha-composite-mirror":
        parser.error("RGBA is reserved for --operation alpha-composite-mirror")
    if args.operation == "resize" and selected_modes != ("F",):
        parser.error("F resize batching currently measures mode F only")
    if "F" in selected_modes and args.operation != "resize":
        parser.error("mode F is currently supported only by --operation resize")
    if args.operation != "grayscale" and "CMYK" in selected_modes:
        parser.error("CMYK batching is currently supported only for grayscale")
    if args.subject:
        print(json.dumps(child(args.subject, args.size, args.height or args.size, args.mode, args.images, args.samples, args.operation, args.max_jobs, args.max_in_flight, args.alpha_input_pattern)))
        return
    subjects = (
        ("Pillow", "GPU eager", "GPU queued")
        if args.operation == "median"
        else SUBJECTS
    )
    identities = {subject: pillow_identity(subject) for subject in subjects}
    versions = {identity["distribution_version"] for identity in identities.values()}
    if len(versions) != 1:
        parser.error(f"installed Pillow distribution mismatch across subjects: {identities}")
    if identities["Pillow"]["module_version"] != identities["Pillow"]["distribution_version"]:
        parser.error(f"oracle module does not match its installed Pillow distribution: {identities['Pillow']}")
    replacement_module = (ROOT / "pillow-rs-py/python/PIL/__init__.py").resolve()
    for subject in subjects:
        if subject != "Pillow" and Path(identities[subject]["module"]).resolve() != replacement_module:
            parser.error(f"{subject} did not import the checkout's replacement PIL namespace: {identities[subject]}")
    if args.json:
        started_at = time.time_ns()
        source_revision = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True,
            check=True, text=True,
        ).stdout.strip()
        source_dirty = bool(subprocess.run(
            ["git", "status", "--porcelain"], cwd=ROOT, capture_output=True,
            check=True, text=True,
        ).stdout.strip())
        report = {
            "schema": "pillow-rs/gpu-batch-benchmark@1",
            "run_id": f"gpu-batch-{started_at}",
            "started_at_unix_ns": started_at,
            "source_revision": source_revision,
            "source_dirty": source_dirty,
            "environment": {
                "host": __import__("platform").node(),
                "os": __import__("platform").platform(),
                "architecture": __import__("platform").machine(),
                "python": sys.version.split()[0],
                "pillow_identities": identities,
            },
            "operation": args.operation,
            "measurement_policy": {
                "boundary": "completed_batch_window",
                "steps": ["construct-image-and-graph", "submit", "execute", "materialize", "export-bytes"],
                "warmup_windows": 2,
                "sample_windows": args.samples,
                "images_per_window": args.images,
                "max_jobs": args.max_jobs,
                "max_in_flight": args.max_in_flight,
                "gpu_batch_submission_proves_concurrent_kernels": False,
                "alpha_composite_mirror_input": (
                    f"two {args.alpha_input_pattern} native RGBA images"
                    if args.operation == "alpha-composite-mirror"
                    else None
                ),
            },
            "workloads": [],
        }
    else:
        print(f"Warm completed-work diagnostic: {args.operation}; two warmup windows; "
              f"{args.samples} median samples; no host threads. GPU batch submission is not proof of concurrent kernels.")
    if not args.json:
        if args.operation in ("boxblur", "grayscale", "alpha-composite-mirror"):
            print("| Mode/size | Pillow img/s | CPU img/s | SIMD img/s | Eager GPU img/s | Queued GPU img/s | Queued / SIMD | Queued / Pillow | Jobs/submission | Dispatches/job |")
            print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
        else:
            print("| Mode/size | Pillow images/s | Eager GPU images/s | Queued GPU images/s | Queued / Pillow | Queued / eager | Max jobs/submission |")
            print("| --- | ---: | ---: | ---: | ---: | ---: | ---: |")
    for mode in selected_modes:
        for size in args.sizes:
            height = args.height or size
            runs = [isolated(subject, size, height, mode, args.images, args.samples, args.operation, args.max_jobs, args.max_in_flight, args.alpha_input_pattern) for subject in subjects]
            for run in runs:
                identity = identities[run["workload"]["subject"]]
                assert run["pillow_version"] == identity["module_version"], identity
                assert run["pillow_module"] == identity["module"], identity
                assert run["pillow_distribution_version"] == identity["distribution_version"], identity
            assert all(run["digests"] == runs[0]["digests"] for run in runs), f"exact parity failed: {mode} {size}x{height}"
            rates = [args.images / run["seconds"] for run in runs]
            if args.json:
                operation_count = args.images * (args.samples + 2)
                report["workloads"].append({
                    "workload_id": (
                        f"pipeline-batch.{args.operation}.{mode.lower()}-{size}x{height}"
                        + (f".{args.alpha_input_pattern}" if args.operation == "alpha-composite-mirror" else "")
                    ),
                    "mode": mode,
                    "size": [size, height],
                    "parameters": (
                        {"filter": "BICUBIC",
                         "destination_size": [max(1, size // 2), max(1, height // 2)],
                         "input_pattern": "deterministic finite f32 noise"}
                        if args.operation == "resize" else None
                    ),
                    "parity_status": "exact_bytes_pass",
                    "parity_comparison_count": args.images,
                    "subjects": [
                        {
                            "subject": run["workload"]["subject"],
                            "pillow_version": run["pillow_version"],
                            "pillow_module": run["pillow_module"],
                            "pillow_distribution_version": run["pillow_distribution_version"],
                            "requested_backend": (
                                run["backend_receipt"]["requested_backend"]
                                if run["backend_receipt"]
                                else "gpu" if run["workload"]["subject"].startswith("GPU")
                                else "pillow"
                            ),
                            "actual_backend": (
                                run["backend_receipt"]["actual_backend"]
                                if run["backend_receipt"]
                                else "gpu" if run["workload"]["subject"].startswith("GPU")
                                else "pillow"
                            ),
                            "actual_backend_counts": (
                                {run["backend_receipt"]["actual_backend"]: operation_count}
                                if run["backend_receipt"]
                                else {"gpu": run["stats"]["submitted_jobs"]}
                                if run["stats"]
                                else {"pillow": operation_count}
                            ),
                            "fallback_reason_counts": (
                                run["backend_receipt"].get("fallback_reason_counts", {})
                                if run["backend_receipt"] else {}
                            ),
                            "terminal_complete": (
                                bool(run["digests"]) and (
                                    run["stats"] is None or run["stats"]["live_jobs"] == 0
                                )
                            ),
                            "window_seconds_median": run["seconds"],
                            "window_seconds_samples": run["sample_seconds"],
                            "throughput_images_per_s": rates[index],
                            "operation_count": (
                                run["stats"]["submitted_jobs"]
                                if run["stats"] else operation_count
                            ),
                            "dispatch_count": (
                                run["stats"]["dispatches"]
                                if run["stats"] else None
                            ),
                            "upload_bytes": (
                                run["stats"]["upload_bytes"]
                                if run["stats"] else None
                            ),
                            "readback_bytes": (
                                run["stats"]["readback_bytes"]
                                if run["stats"] else None
                            ),
                            "output_sha256": run["digests"],
                            "backend_receipt": run["backend_receipt"],
                            "batch_stats": run["stats"],
                        }
                        for index, run in enumerate(runs)
                    ],
                })
                continue
            if args.operation in ("boxblur", "grayscale"):
                queued_stats = runs[4]["stats"]
                print(f"| {mode} {size}×{height} | {rates[0]:.1f} | {rates[1]:.1f} | {rates[2]:.1f} | {rates[3]:.1f} | {rates[4]:.1f} | "
                      f"{rates[4]/rates[2]:.2f}× | {rates[4]/rates[0]:.2f}× | {queued_stats['largest_submission']} | "
                      f"{queued_stats['dispatches']/queued_stats['submitted_jobs']:.2f} |", flush=True)
            else:
                print(f"| {mode} {size}² | {rates[0]:.1f} | {rates[1]:.1f} | {rates[2]:.1f} | "
                      f"{rates[2]/rates[0]:.2f}× | {rates[2]/rates[1]:.2f}× | {runs[2]['stats']['largest_submission']} |", flush=True)
    if args.json:
        report["finished_at_unix_ns"] = time.time_ns()
        print(json.dumps(report, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__": main()

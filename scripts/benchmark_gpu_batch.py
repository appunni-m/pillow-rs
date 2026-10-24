#!/usr/bin/env python3
"""Completed-work GPU batching diagnostic, isolated live Pillow baseline.

Fresh inputs/graphs every request; warm shader caches, no result cache reuse.
Includes construction, upload, dispatch, readback, Image and terminal byte export.
Reference construction, timing aggregation and exact byte comparisons are outside
measured windows. No host thread pools or JSON evidence files are created. The
BoxBlur lane measures an explicit one-operation GPU batch against strict serial
CPU/SIMD execution and Pillow. The grayscale lane measures
ImageOps.grayscale as a one-operation graph. The GetChannel lane measures
materialized L and RGB extraction as a one-operation graph. The
alpha-composite-mirror lane measures the fused RGBA public pipeline against all
five subjects. JSON evidence
is emitted to stdout only when requested with --json. The F Resize lane measures
one native F bicubic resize with exact output comparison and CPU materialization.
The Filter lane measures the exact native-LA binomial 3x3 Kernel operation.
The Screen lane measures ImageChops.screen over two deterministic native images.
The Merge lane measures three materialized L bands composed into RGB. Its
direct GPU lane includes per-image synchronization and materialization; the
current GpuBatchExecutor admission contract rejects this graph. Use
``--operation median-only`` to measure MedianFilter(3) without the separate
invert operation. The bitmap lane measures ImageDraw.bitmap over deterministic
destination and L-mask inputs. Its GPU subjects cover both eager dispatch and
the explicit queued stream's native masked-Paste shader.
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
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
SUBJECTS = ("Pillow", "CPU", "SIMD", "GPU eager", "GPU queued")
FILTER_3X3_BINOMIAL = (1, 2, 1, 2, 4, 2, 1, 2, 1)
DEFAULT_BATCH_SIZES = (256, 1024)


def default_batch_modes(operation):
    if operation == "alpha-composite-mirror":
        return ("RGBA",)
    if operation == "resize":
        return ("F",)
    if operation == "filter":
        return ("L", "LA")
    if operation == "merge":
        return ("RGB",)
    if operation == "screen":
        return ("RGB",)
    if operation == "bitmap":
        return ("L", "RGB", "CMYK")
    return ("L", "RGB")


def resolve_selection(operation, subject, mode, modes, size, sizes):
    """Resolve explicit selectors while preserving the default cohort matrix."""
    if subject is not None:
        if modes is not None or sizes is not None:
            raise ValueError("--modes/--sizes are cohort selectors; use --mode/--size with --subject")
        return (mode or "L",), (size if size is not None else 256,)
    if mode is not None and modes is not None:
        raise ValueError("choose either --mode or --modes, not both")
    if size is not None and sizes is not None:
        raise ValueError("choose either --size or --sizes, not both")
    selected_modes = (mode,) if mode is not None else tuple(modes) if modes else default_batch_modes(operation)
    selected_sizes = (size,) if size is not None else tuple(sizes) if sizes else DEFAULT_BATCH_SIZES
    return selected_modes, selected_sizes


def validate_queued_capacity(subjects, images, max_jobs, max_in_flight):
    """Keep queued throughput assertions meaningful for every cohort window."""
    if "GPU queued" in subjects and min(images, max_jobs, max_in_flight) < 2:
        raise ValueError(
            "GPU queued throughput requires at least 2 images, --max-jobs >= 2, "
            "and --max-in-flight >= 2; use GPU eager for a one-image latency run"
        )


def batch_workload_id(operation, mode, size, height, alpha_input_pattern, draw_context="native"):
    workload_id = f"pipeline-batch.{operation}.{mode.lower()}-{size}x{height}"
    if operation == "alpha-composite-mirror":
        workload_id += f".{alpha_input_pattern}"
    if operation == "bitmap" and draw_context == "rgba":
        workload_id += ".rgba-context"
    return workload_id


def expected_workload_ids(operation, modes, sizes, height, alpha_input_pattern, draw_context="native"):
    return {
        batch_workload_id(
            operation, mode, size, height or size, alpha_input_pattern, draw_context
        )
        for mode in modes
        for size in sizes
    }


def extension_identity(module):
    path = Path(module.__file__).resolve()
    return {
        "path": str(path),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def child(subject, size, height, mode, images, samples, operation="median", max_jobs=16, max_in_flight=4, alpha_input_pattern="uniform", capture_output_path=None, draw_context="native"):
    from importlib.metadata import version as distribution_version
    import PIL
    from PIL import Image, ImageOps, ImageFilter, ImageChops, ImageDraw
    loaded_core_identity = None
    if subject != "Pillow":
        import pillow_rs
        loaded_core_identity = extension_identity(pillow_rs._core)
    channels = {"L": 1, "LA": 2, "RGB": 3, "CMYK": 4, "RGBA": 4, "F": 4}[mode]
    source_channels = 1 if operation == "merge" else channels
    length = size * height * source_channels
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
    elif operation == "merge":
        sources = [
            tuple(
                random.Random(index + size * 31 + height * 17 + band * 101).randbytes(length)
                for band in range(3)
            )
            for index in range(images)
        ]
    elif operation == "screen":
        sources = [
            (
                random.Random(index + size * 31 + height * 17 + channels).randbytes(length),
                random.Random(index + size * 31 + height * 17 + channels + 1).randbytes(length),
            )
            for index in range(images)
        ]
    elif operation == "bitmap":
        sources = [
            (
                random.Random(index + size * 31 + height * 17 + channels).randbytes(length),
                random.Random(index + size * 31 + height * 17 + channels + 1).randbytes(size * height),
            )
            for index in range(images)
        ]
    else:
        sources = [
            random.Random(index + size * 31 + height * 17 + channels).randbytes(length)
            for index in range(images)
        ]

    def graph(index):
        if operation == "merge":
            bands = tuple(
                Image.frombytes("L", (size, height), pixels)
                for pixels in sources[index]
            )
            result = Image.merge("RGB", bands)
        elif operation == "alpha-composite-mirror":
            if alpha_input_pattern == "uniform":
                destination = Image.new("RGBA", (size, height), (30, 60, 90, 64))
                source = Image.new("RGBA", (size, height), (200, 150, 100, 192))
            else:
                destination = Image.frombytes("RGBA", (size, height), sources[index][0])
                source = Image.frombytes("RGBA", (size, height), sources[index][1])
            result = Image.alpha_composite(destination, source)
            result = ImageOps.mirror(result)
        elif operation == "screen":
            left = Image.frombytes(mode, (size, height), sources[index][0])
            right = Image.frombytes(mode, (size, height), sources[index][1])
            result = ImageChops.screen(left, right)
        elif operation == "bitmap":
            result = Image.frombytes(mode, (size, height), sources[index][0])
            mask = Image.frombytes("L", (size, height), sources[index][1])
            draw = ImageDraw.Draw(result, "RGBA" if draw_context == "rgba" else None)
            if draw_context == "rgba":
                fill = (211, 37, 89, 96)
            elif mode == "L":
                fill = 211
            elif mode == "CMYK":
                fill = (21, 37, 89, 173)
            else:
                fill = (211, 37, 89)
            draw.bitmap((0, 0), mask, fill=fill)
        else:
            result = Image.frombytes(mode, (size, height), sources[index])
        if operation == "boxblur":
            result = result.filter(ImageFilter.BoxBlur(1))
        elif operation == "grayscale":
            result = ImageOps.grayscale(result)
        elif operation == "getchannel":
            result = result.getchannel(0 if mode == "L" else "G")
        elif operation == "resize":
            result = result.resize((max(1, size // 2), max(1, height // 2)), Image.Resampling.BICUBIC)
        elif operation == "median":
            result = ImageOps.invert(result).filter(ImageFilter.MedianFilter(3))
        elif operation == "median-only":
            result = result.filter(ImageFilter.MedianFilter(3))
        elif operation == "filter":
            result = result.filter(
                ImageFilter.Kernel((3, 3), FILTER_3X3_BINOMIAL, scale=16, offset=0)
            )
        if subject in ("CPU", "SIMD") or (
            operation == "bitmap" and subject.startswith("GPU")
        ) or (operation == "merge" and subject == "GPU eager"):
            # A single active backend plus a per-image lock makes unsupported
            # contexts fail instead of silently entering CPU fallback.
            rust = __import__("pillow_rs")
            result._rust_image.lock_active_backend()
        return result

    core = None
    gpu_core = None
    bitmap_gpu_batch = operation == "bitmap" and subject == "GPU queued"
    if subject in ("CPU", "SIMD") or (
        operation == "bitmap" and subject == "GPU eager"
    ) or (operation == "merge" and subject == "GPU eager") or bitmap_gpu_batch:
        import pillow_rs
        if bitmap_gpu_batch:
            gpu_core = pillow_rs._core
        else:
            core = pillow_rs._core
        for backend in ("cpu", "simd", "gpu"):
            pillow_rs.disable_backend(backend)
        active_backend = "gpu" if subject.startswith("GPU") else subject.lower()
        pillow_rs.enable_backend(active_backend)

    executor = None
    direct_gpu_bitmap = operation == "bitmap" and subject == "GPU eager"
    if subject.startswith("GPU") and not direct_gpu_bitmap and not (
        operation == "merge" and subject == "GPU eager"
    ):
        from PIL import GpuBatchExecutor
        executor = GpuBatchExecutor(queue=subject == "GPU queued", max_jobs=max_jobs, max_in_flight=max_in_flight)

    def window():
        if executor is None:
            return [graph(i).tobytes() for i in range(images)]
        output = {}
        for result in executor.run((i, graph(i)) for i in range(images)):
            output[result.input_key] = result.image.tobytes()
        return [output[i] for i in range(images)]
    times, digest, reference_output = [], None, None
    receipt = None
    gpu_shader_receipt = None
    try:
        if core is not None:
            core.take_pipeline_telemetry()
            if operation == "bitmap" and subject == "GPU eager":
                core.take_gpu_shader_coverage()
                core.set_gpu_shader_coverage(True)
            core.set_pipeline_telemetry(True)
            proof = graph(0).tobytes()
            receipt = core.take_pipeline_telemetry()
            core.set_pipeline_telemetry(False)
            if operation == "bitmap" and subject == "GPU eager":
                gpu_shader_receipt = core.take_gpu_shader_coverage()
                core.set_gpu_shader_coverage(False)
            expected_backend = "gpu" if subject == "GPU eager" else subject.lower()
            assert receipt and receipt["actual_backend"] == expected_backend, receipt
            assert receipt["requested_backend"] == expected_backend, receipt
            assert receipt["fallback_reason"] is None, receipt
            assert proof, "backend proof materialized no bytes"
            if operation == "bitmap" and subject == "GPU eager":
                assert receipt["dispatch_count"] == 1, receipt
                resource = receipt.get("resource")
                assert resource and resource["upload_bytes"] > 0, receipt
                assert resource["readback_bytes"] == size * height * channels, receipt
                expected_shader = {
                    "L": "paste_native_masked_l.wgsl",
                    "RGB": "paste_native_masked_rgb.wgsl",
                    "CMYK": "paste_native_masked_rgba.wgsl",
                }[mode]
                assert len(gpu_shader_receipt) == 1, gpu_shader_receipt
                assert gpu_shader_receipt[0]["shader_file"] == expected_shader, gpu_shader_receipt
                assert gpu_shader_receipt[0]["dispatches"] == 1, gpu_shader_receipt
            if operation == "bitmap" and subject == "SIMD":
                paste_paths = [
                    item for item in receipt.get("operation_telemetry", [])
                    if item.get("operation") == "Paste"
                    and item.get("path") in (
                        "vector",
                        "neon-blend",
                        "neon-rgb-l-blend",
                        "neon-cmyk-l-blend",
                    )
                ]
                assert paste_paths, receipt

        for warmup_index in range(2):
            if bitmap_gpu_batch and warmup_index == 0:
                gpu_core.take_gpu_shader_coverage()
                gpu_core.set_gpu_shader_coverage(True)
            warm_output = window()
            if bitmap_gpu_batch and warmup_index == 0:
                gpu_shader_receipt = gpu_core.take_gpu_shader_coverage()
                gpu_core.set_gpu_shader_coverage(False)
                assert len(warm_output) == images and all(warm_output), "GPU bitmap warmup did not materialize every image"
                assert all(
                    item["shader_file"] == "paste_native_masked_bytes.wgsl"
                    for item in gpu_shader_receipt
                ), gpu_shader_receipt
                assert sum(item["dispatches"] for item in gpu_shader_receipt) == images, gpu_shader_receipt
        for _ in range(samples):
            started = time.perf_counter_ns()
            output = window()
            elapsed = time.perf_counter_ns() - started
            if reference_output is None:
                reference_output = output
                if capture_output_path is not None:
                    with open(capture_output_path, "wb") as capture:
                        for image in output:
                            capture.write(len(image).to_bytes(8, "little"))
                            capture.write(image)
            else:
                assert output == reference_output, "fresh request output bytes changed"
            actual = [hashlib.sha256(image).hexdigest() for image in output]
            if digest is not None: assert actual == digest, "fresh request output digests changed"
            digest = actual
            times.append(elapsed / 1e9)
        stats = executor.stats if executor else None
        if stats:
            assert stats["gpu_bytes"] == stats["host_bytes"] == stats["live_jobs"] == 0, stats
            assert stats["submitted_jobs"] == (samples + 2) * images, stats
            dispatches_per_job = 1 if operation in ("median-only", "screen", "boxblur", "grayscale", "getchannel", "alpha-composite-mirror", "filter", "merge", "bitmap") else 2
            assert stats["dispatches"] == dispatches_per_job * stats["submitted_jobs"], stats
            inputs_per_job = 3 if operation == "merge" else 2 if operation in ("alpha-composite-mirror", "screen") else 1
            upload_bytes_per_job = (
                length + channels + size * height
                if operation == "bitmap" else length * inputs_per_job
            )
            expected_bytes = stats["submitted_jobs"] * upload_bytes_per_job
            assert stats["upload_bytes"] == expected_bytes, stats
            output_channels = 1 if operation in ("grayscale", "getchannel") else channels
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
                "gpu_shader_receipt": gpu_shader_receipt,
                "pillow_version": PIL.__version__, "pillow_module": PIL.__file__,
                "pillow_distribution_version": distribution_version("Pillow"),
                "loaded_core_identity": loaded_core_identity,
                "workload": {"operation": operation, "mode": mode, "size": [size, height],
                             "images_per_window": images, "samples": samples,
                             "parameters": (
                                 {"output_mode": "RGB", "band_mode": "L", "bands": 3}
                                 if operation == "merge" else
                                 {
                                     "mask_mode": "L",
                                     "fill": (
                                         [211, 37, 89, 96]
                                         if draw_context == "rgba" else
                                         211 if mode == "L" else
                                         [21, 37, 89, 173] if mode == "CMYK" else
                                         [211, 37, 89]
                                     ),
                                     "input_pattern": "deterministic native destination bytes and L-mask bytes",
                                 }
                                 if operation == "bitmap" else
                                 {"filter": "BICUBIC",
                                  "destination_size": [max(1, size // 2), max(1, height // 2)],
                                  "input_pattern": "deterministic finite f32 noise"}
                                 if operation == "resize" else
                                 {"filter": "Kernel",
                                  "kernel_size": [3, 3],
                                  "kernel": FILTER_3X3_BINOMIAL,
                                  "scale": 16,
                                  "offset": 0,
                                  "input_pattern": "deterministic native-LA noise"}
                                 if operation == "filter" else
                                 {"filter": "MedianFilter(3)",
                                  "input_pattern": f"deterministic native-{mode} noise"}
                                 if operation == "median-only" else
                                 {"input_pattern": f"two deterministic native-{mode} noise images"}
                                 if operation == "screen" else
                                 {"channel": 0 if mode == "L" else "G"}
                                 if operation == "getchannel" else None
                             ),
                             "alpha_input_pattern": (
                                 alpha_input_pattern if operation == "alpha-composite-mirror" else None
                             ),
                             "draw_context": (
                                 draw_context if operation == "bitmap" else None
                             ),
                             "subject": subject}}
    finally:
        if executor: executor.close()


def isolated(subject, size, height, mode, images, samples, operation, max_jobs=16, max_in_flight=4, alpha_input_pattern="uniform", capture_output_path=None, draw_context="native"):
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    if subject != "Pillow": env["PYTHONPATH"] = str(ROOT / "pillow-rs-py/python")
    command = [sys.executable, str(Path(__file__).resolve()), "--subject", subject,
        "--operation", operation, "--size", str(size), "--height", str(height), "--mode", mode, "--images", str(images), "--samples", str(samples), "--max-jobs", str(max_jobs), "--max-in-flight", str(max_in_flight), "--alpha-input-pattern", alpha_input_pattern, "--draw-context", draw_context]
    if capture_output_path is not None:
        command.extend(("--capture-output", str(capture_output_path)))
    process = subprocess.run(command, env=env, capture_output=True, text=True)
    if process.returncode: raise RuntimeError(process.stderr)
    return json.loads(process.stdout)


def exact_output_files_equal(left_path, right_path):
    with open(left_path, "rb") as left, open(right_path, "rb") as right:
        while True:
            left_chunk = left.read(1024 * 1024)
            right_chunk = right.read(1024 * 1024)
            if left_chunk != right_chunk:
                return False
            if not left_chunk:
                return True


def pillow_identity(subject):
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    if subject != "Pillow": env["PYTHONPATH"] = str(ROOT / "pillow-rs-py/python")
    identity_script = """
from importlib.metadata import version
import hashlib
import json
from pathlib import Path
import sys
import PIL

identity = {
    "distribution_version": version("Pillow"),
    "module_version": PIL.__version__,
    "module": PIL.__file__,
    "core": None,
}
if sys.argv[1] != "Pillow":
    import pillow_rs
    core_path = Path(pillow_rs._core.__file__).resolve()
    identity["core"] = {
        "path": str(core_path),
        "sha256": hashlib.sha256(core_path.read_bytes()).hexdigest(),
    }
print(json.dumps(identity))
"""
    process = subprocess.run(
        [sys.executable, "-c", identity_script, subject],
        env=env, capture_output=True, text=True,
    )
    if process.returncode: raise RuntimeError(process.stderr)
    return json.loads(process.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--subject", choices=SUBJECTS)
    parser.add_argument(
        "--operation",
        choices=("median", "median-only", "screen", "boxblur", "grayscale", "getchannel", "alpha-composite-mirror", "resize", "filter", "merge", "bitmap"),
        default="median",
    )
    parser.add_argument("--size", type=int,
                        help="select one size in cohort mode; defaults to the standard matrix")
    parser.add_argument("--height", type=int)
    parser.add_argument("--mode", choices=("L", "LA", "RGB", "CMYK", "RGBA", "F"),
                        help="select one mode in cohort mode; defaults to the operation matrix")
    parser.add_argument("--images", type=int, default=32)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--modes", nargs="+", choices=("L", "LA", "RGB", "CMYK", "RGBA", "F"))
    parser.add_argument("--sizes", nargs="+", type=int,
                        help="select multiple sizes; conflicts with --size")
    parser.add_argument("--max-jobs", type=int, default=16)
    parser.add_argument("--max-in-flight", type=int, default=4)
    parser.add_argument(
        "--draw-context",
        choices=("native", "rgba"),
        default="native",
        help="ImageDraw context for bitmap (native or RGBA over an RGB image)",
    )
    parser.add_argument("--capture-output", type=Path,
                        help=argparse.SUPPRESS)
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
    try:
        selected_modes, selected_sizes = resolve_selection(
            args.operation, args.subject, args.mode, args.modes, args.size, args.sizes
        )
    except ValueError as error:
        parser.error(str(error))
    if args.operation == "alpha-composite-mirror" and selected_modes != ("RGBA",):
        parser.error("alpha-composite-mirror requires RGBA")
    if args.operation == "merge" and selected_modes != ("RGB",):
        parser.error("merge batching currently measures three L bands into RGB")
    if "RGBA" in selected_modes and args.operation != "alpha-composite-mirror":
        parser.error("RGBA is reserved for --operation alpha-composite-mirror")
    if args.operation == "resize" and selected_modes != ("F",):
        parser.error("F resize batching currently measures mode F only")
    if args.operation == "bitmap" and any(mode not in ("L", "RGB", "CMYK") for mode in selected_modes):
        parser.error("bitmap benchmarking supports native L/RGB/CMYK destinations")
    if args.operation == "bitmap" and args.draw_context == "rgba" and selected_modes != ("RGB",):
        parser.error("RGBA draw context is supported only over an RGB bitmap destination")
    if args.operation != "bitmap" and args.draw_context != "native":
        parser.error("--draw-context applies only to --operation bitmap")
    if args.operation == "filter" and any(mode not in ("L", "LA") for mode in selected_modes):
        parser.error("Kernel filter batching currently supports native L/LA inputs")
    if args.operation != "filter" and "LA" in selected_modes:
        parser.error("LA is currently supported only by Kernel filter batching")
    if "F" in selected_modes and args.operation != "resize":
        parser.error("mode F is currently supported only by --operation resize")
    if args.operation not in ("grayscale", "getchannel", "bitmap") and "CMYK" in selected_modes:
        parser.error("CMYK batching is currently supported only for grayscale")
    if args.operation == "getchannel" and any(mode not in ("L", "RGB") for mode in selected_modes):
        parser.error("GetChannel batching currently supports L/RGB inputs")
    if args.operation == "screen" and any(mode not in ("L", "RGB") for mode in selected_modes):
        parser.error("Screen batching currently supports native L/RGB inputs")
    if args.subject:
        try:
            validate_queued_capacity(
                (args.subject,), args.images, args.max_jobs, args.max_in_flight
            )
        except ValueError as error:
            parser.error(str(error))
        mode = selected_modes[0]
        size = selected_sizes[0]
        print(json.dumps(child(
            args.subject, size, args.height or size, mode, args.images, args.samples,
            args.operation, args.max_jobs, args.max_in_flight,
            args.alpha_input_pattern, args.capture_output, args.draw_context,
        )))
        return
    subjects = (
        ("Pillow", "GPU eager", "GPU queued")
        if args.operation == "median"
        else ("Pillow", "SIMD", "GPU queued")
        if args.operation == "median-only"
        else ("Pillow", "CPU", "SIMD", "GPU eager")
        if args.operation == "merge"
        else ("Pillow", "CPU", "SIMD", "GPU eager", "GPU queued")
        if args.operation == "bitmap"
        else SUBJECTS
    )
    try:
        validate_queued_capacity(subjects, args.images, args.max_jobs, args.max_in_flight)
    except ValueError as error:
        parser.error(str(error))
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
            "benchmark_script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "environment": {
                "host": __import__("platform").node(),
                "os": __import__("platform").platform(),
                "architecture": __import__("platform").machine(),
                "python": sys.version.split()[0],
                "pillow_identities": identities,
            },
            "operation": args.operation,
            "measurement_policy": {
                "boundary": (
                    "completed_direct_gpu_window"
                    if args.operation == "merge"
                    else "completed_work_window"
                    if args.operation == "bitmap"
                    else "completed_batch_window"
                ),
                "parity_method": "length-prefixed output bytes compared exactly across subjects and all measured windows",
                "steps": (
                    ["construct-image-and-graph", "execute", "synchronize", "materialize", "export-bytes"]
                    if args.operation == "merge"
                    else ["construct-image-and-graph", "submit-or-dispatch", "execute", "synchronize", "materialize", "export-bytes"]
                    if args.operation == "bitmap"
                    else ["construct-image-and-graph", "submit", "execute", "materialize", "export-bytes"]
                ),
                "warmup_windows": 2,
                "sample_windows": args.samples,
                "images_per_window": args.images,
                "max_jobs": args.max_jobs if args.operation != "merge" else None,
                "max_in_flight": args.max_in_flight if args.operation != "merge" else None,
                "gpu_batch_submission_proves_concurrent_kernels": False,
                "gpu_batch_merge_admission": (
                    "unsupported: operation Merge has no composed native L layout"
                    if args.operation == "merge"
                    else None
                ),
                "merge_input": (
                    "three materialized native L images composed to RGB"
                    if args.operation == "merge"
                    else None
                ),
                "bitmap_gpu_execution_modes": (
                    {
                        "GPU eager": "direct per-image GPU dispatch and readback",
                        "GPU queued": "GpuBatchExecutor submission with one native masked-Paste dispatch per image",
                    }
                    if args.operation == "bitmap"
                    else None
                ),
                "bitmap_draw_context": (
                    args.draw_context if args.operation == "bitmap" else None
                ),
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
        if args.operation == "bitmap":
            print("| Mode/size | Pillow img/s | CPU img/s | SIMD img/s | Eager GPU img/s | Queued GPU img/s | Queued / SIMD |")
            print("| --- | ---: | ---: | ---: | ---: | ---: | ---: |")
        elif args.operation in ("screen", "boxblur", "grayscale", "getchannel", "alpha-composite-mirror", "filter"):
            print("| Mode/size | Pillow img/s | CPU img/s | SIMD img/s | Eager GPU img/s | Queued GPU img/s | Queued / SIMD | Queued / Pillow | Jobs/submission | Dispatches/job |")
            print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
        elif args.operation == "merge":
            print("| Mode/size | Pillow img/s | CPU img/s | SIMD img/s | Direct GPU img/s | GPU / SIMD | GPU batch |")
            print("| --- | ---: | ---: | ---: | ---: | ---: | --- |")
        else:
            print("| Mode/size | Pillow images/s | Eager GPU images/s | Queued GPU images/s | Queued / Pillow | Queued / eager | Max jobs/submission |")
            print("| --- | ---: | ---: | ---: | ---: | ---: | ---: |")
    capture_directory = tempfile.TemporaryDirectory(prefix="pillow-rs-gpu-batch-parity-")
    capture_root = Path(capture_directory.name)
    for mode in selected_modes:
        for size in selected_sizes:
            height = args.height or size
            workload_id = batch_workload_id(
                args.operation, mode, size, height, args.alpha_input_pattern,
                args.draw_context,
            )
            capture_paths = {
                subject: capture_root / f"{mode}-{size}-{subject.replace(' ', '-')}.bin"
                for subject in subjects
            }
            runs = [isolated(
                subject, size, height, mode, args.images, args.samples,
                args.operation, args.max_jobs, args.max_in_flight,
                args.alpha_input_pattern, capture_paths[subject], args.draw_context,
            ) for subject in subjects]
            for run in runs:
                identity = identities[run["workload"]["subject"]]
                assert run["pillow_version"] == identity["module_version"], identity
                assert run["pillow_module"] == identity["module"], identity
                assert run["pillow_distribution_version"] == identity["distribution_version"], identity
                expected_core_identity = identity["core"]
                actual_core_identity = run["loaded_core_identity"]
                if actual_core_identity != expected_core_identity:
                    raise AssertionError(
                        f"loaded extension identity changed for {run['workload']['subject']}: "
                        f"expected={expected_core_identity}, actual={actual_core_identity}"
                    )
            assert all(run["digests"] == runs[0]["digests"] for run in runs), f"output digest mismatch: {mode} {size}x{height}"
            assert all(path.is_file() and path.stat().st_size > 0 for path in capture_paths.values()), (
                f"missing exact output capture: {mode} {size}x{height}"
            )
            assert all(
                exact_output_files_equal(capture_paths[subjects[0]], capture_paths[subject])
                for subject in subjects[1:]
            ), f"byte-for-byte output parity failed: {mode} {size}x{height}"
            rates = [args.images / run["seconds"] for run in runs]
            operation_count = args.images * (args.samples + 2)

            def total_receipt_resource(run, key):
                receipt = run["backend_receipt"]
                resource = receipt.get("resource") if receipt else None
                value = resource.get(key) if resource else None
                if value is None:
                    return None
                return value if key == "peak_live_host_bytes" else value * operation_count

            if args.json:
                report["workloads"].append({
                    "workload_id": workload_id,
                    "mode": mode,
                    "size": [size, height],
                    "parameters": (
                        {"output_mode": "RGB", "band_mode": "L", "bands": 3}
                        if args.operation == "merge" else
                        {
                            "mask_mode": "L",
                            "fill": (
                                [211, 37, 89, 96]
                                if args.draw_context == "rgba" else
                                211 if mode == "L" else
                                [21, 37, 89, 173] if mode == "CMYK" else
                                [211, 37, 89]
                            ),
                            "draw_context": args.draw_context,
                            "input_pattern": "deterministic native destination bytes and L-mask bytes",
                            "GPU_execution_modes": {
                                "GPU eager": "direct per-image dispatch",
                                "GPU queued": "GpuBatchExecutor, one masked-Paste dispatch per image",
                            },
                        }
                        if args.operation == "bitmap" else
                        {"filter": "BICUBIC",
                         "destination_size": [max(1, size // 2), max(1, height // 2)],
                         "input_pattern": "deterministic finite f32 noise"}
                        if args.operation == "resize" else
                        {"filter": "Kernel",
                         "kernel_size": [3, 3],
                         "kernel": FILTER_3X3_BINOMIAL,
                         "scale": 16,
                         "offset": 0,
                         "input_pattern": "deterministic native-LA noise"}
                        if args.operation == "filter" else
                        {"filter": "MedianFilter(3)",
                         "input_pattern": f"deterministic native-{mode} noise"}
                        if args.operation == "median-only" else
                        {"input_pattern": f"two deterministic native-{mode} noise images"}
                        if args.operation == "screen" else
                        {"channel": 0 if mode == "L" else "G"}
                        if args.operation == "getchannel" else None
                    ),
                    "parity_status": "exact_bytes_pass",
                    "parity_method": "length-prefixed output bytes compared exactly across subjects and all measured windows",
                    "parity_comparison_count": args.images,
                    "subjects": [
                        {
                            "subject": run["workload"]["subject"],
                            "pillow_version": run["pillow_version"],
                            "pillow_module": run["pillow_module"],
                            "pillow_distribution_version": run["pillow_distribution_version"],
                            "loaded_core_identity": run["loaded_core_identity"],
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
                                if run["backend_receipt"] is None
                                else (
                                    run["backend_receipt"]["dispatch_count"] * operation_count
                                    if run["backend_receipt"]["dispatch_count"] is not None
                                    else None
                                )
                            ),
                            "upload_bytes": (
                                run["stats"]["upload_bytes"]
                                if run["stats"] else total_receipt_resource(run, "upload_bytes")
                            ),
                            "readback_bytes": (
                                run["stats"]["readback_bytes"]
                                if run["stats"] else total_receipt_resource(run, "readback_bytes")
                            ),
                            "parameter_bytes": total_receipt_resource(run, "parameter_bytes"),
                            "host_allocation_count": total_receipt_resource(run, "host_allocation_count"),
                            "host_allocated_bytes": total_receipt_resource(run, "host_allocated_bytes"),
                            "peak_live_host_bytes": total_receipt_resource(run, "peak_live_host_bytes"),
                            "host_buffer_count": total_receipt_resource(run, "host_buffer_count"),
                            "host_buffer_bytes": total_receipt_resource(run, "host_buffer_bytes"),
                            "full_frame_copy_count": total_receipt_resource(run, "full_frame_copy_count"),
                            "mode_conversion_count": total_receipt_resource(run, "mode_conversion_count"),
                            "fused_operation_count": total_receipt_resource(run, "fused_operation_count"),
                            "output_sha256": run["digests"],
                            "backend_receipt": run["backend_receipt"],
                            "gpu_shader_receipt": run["gpu_shader_receipt"],
                            "batch_stats": run["stats"],
                        }
                        for index, run in enumerate(runs)
                    ],
                })
                continue
            if args.operation == "bitmap":
                suffix = " RGBA context" if args.draw_context == "rgba" else ""
                print(f"| {mode}{suffix} {size}×{height} | {rates[0]:.1f} | {rates[1]:.1f} | {rates[2]:.1f} | {rates[3]:.1f} | {rates[4]:.1f} | {rates[4]/rates[2]:.2f}× |", flush=True)
                continue
            if args.operation == "merge":
                print(f"| {mode} {size}×{height} | {rates[0]:.1f} | {rates[1]:.1f} | {rates[2]:.1f} | {rates[3]:.1f} | "
                      f"{rates[3]/rates[2]:.2f}× | unsupported at admission |", flush=True)
                continue
            if args.operation in ("screen", "boxblur", "grayscale", "getchannel", "filter"):
                queued_stats = runs[4]["stats"]
                print(f"| {mode} {size}×{height} | {rates[0]:.1f} | {rates[1]:.1f} | {rates[2]:.1f} | {rates[3]:.1f} | {rates[4]:.1f} | "
                      f"{rates[4]/rates[2]:.2f}× | {rates[4]/rates[0]:.2f}× | {queued_stats['largest_submission']} | "
                      f"{queued_stats['dispatches']/queued_stats['submitted_jobs']:.2f} |", flush=True)
            else:
                print(f"| {mode} {size}² | {rates[0]:.1f} | {rates[1]:.1f} | {rates[2]:.1f} | "
                      f"{rates[2]/rates[0]:.2f}× | {rates[2]/rates[1]:.2f}× | {runs[2]['stats']['largest_submission']} |", flush=True)
    capture_directory.cleanup()
    if args.json:
        actual_workload_ids = {item["workload_id"] for item in report["workloads"]}
        wanted_workload_ids = expected_workload_ids(
            args.operation, selected_modes, selected_sizes, args.height,
            args.alpha_input_pattern, args.draw_context,
        )
        if actual_workload_ids != wanted_workload_ids:
            raise AssertionError(
                f"emitted workload IDs differ from selected scope: "
                f"expected={sorted(wanted_workload_ids)}, actual={sorted(actual_workload_ids)}"
            )
        report["finished_at_unix_ns"] = time.time_ns()
        print(json.dumps(report, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__": main()

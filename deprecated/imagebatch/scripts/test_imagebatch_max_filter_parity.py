#!/usr/bin/env python3
"""Check explicit queued MaxFilter(3) against isolated Pillow output bytes.

Run after ``make build-parity``. Pillow and pillow-rs run in separate
processes so the replacement ``PIL`` namespace never shadows the oracle.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MODES = {"L": 1, "LA": 2, "RGB": 3, "RGBA": 4}
SMALL = ((7, 5, 11), (7, 5, 229), (1, 1, 73))
IMAGE_COUNT = 64
IMAGE_SIZE = (64, 64)
LARGE_IMAGE_COUNT = 16
LARGE_IMAGE_SIZE = (256, 256)


def pixels(mode: str, size: tuple[int, int], seed: int) -> bytes:
    if seed in (11, 229):
        return bytes((seed,)) * size[0] * size[1] * MODES[mode]
    count = size[0] * size[1] * MODES[mode]
    return bytes((index * 31 + seed * 47 + (index // 9) * 13) & 255 for index in range(count))


def benchmark_pixels(
    mode: str, seed: int, size: tuple[int, int] = IMAGE_SIZE
) -> bytes:
    count = size[0] * size[1] * MODES[mode]
    return bytes((index * 73 + (index // 11) * 19 + seed * 47 + (seed >> 2)) & 255 for index in range(count))


def run_oracle(output: Path) -> None:
    import PIL
    from PIL import Image, ImageFilter

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"unexpected Pillow oracle version: {PIL.__version__}")
    expected = {
        "pillow_version": PIL.__version__,
        "small": {},
        "benchmark": {},
        "large": {},
    }
    for mode in MODES:
        small_outputs = []
        for width, height, seed in SMALL:
            image = Image.frombytes(
                mode, (width, height), pixels(mode, (width, height), seed)
            )
            image.info["batch-seed"] = seed
            result = image.filter(ImageFilter.MaxFilter(3))
            small_outputs.append(
                {
                    "bytes": result.tobytes().hex(),
                    "size": list(result.size),
                    "mode": result.mode,
                    "info": result.info.get("batch-seed"),
                }
            )
        expected["small"][mode] = small_outputs
        expected["benchmark"][mode] = [
            Image.frombytes(
                mode, IMAGE_SIZE, benchmark_pixels(mode, seed)
            ).filter(ImageFilter.MaxFilter(3)).tobytes().hex()
            for seed in range(IMAGE_COUNT)
        ]
        expected["large"][mode] = [
            Image.frombytes(
                mode,
                LARGE_IMAGE_SIZE,
                benchmark_pixels(mode, seed, LARGE_IMAGE_SIZE),
            )
            .filter(ImageFilter.MaxFilter(3))
            .tobytes()
            .hex()
            for seed in range(LARGE_IMAGE_COUNT)
        ]
    output.write_text(json.dumps(expected))


def require_exact(actual, expected, label: str) -> None:
    if len(actual) != len(expected):
        raise AssertionError(f"{label} returned {len(actual)} images; expected {len(expected)}")
    for index, (image, reference) in enumerate(zip(actual, expected, strict=True)):
        if (
            image.mode != reference["mode"]
            or list(image.size) != reference["size"]
            or image.tobytes().hex() != reference["bytes"]
            or image.info.get("batch-seed") != reference["info"]
        ):
            raise AssertionError(f"{label} output {index} differs from Pillow")


def require_gpu_group(core, label: str, expected_dispatches: int = 1) -> None:
    receipt = core.take_pipeline_telemetry()
    if (
        receipt is None
        or receipt.get("actual_backend") != "gpu"
        or receipt.get("fallback_reason")
        or receipt.get("dispatch_count") != 1
    ):
        raise AssertionError(f"{label} did not execute as GPU work: {receipt}")
    resource = receipt.get("resource")
    if not isinstance(resource, dict) or resource.get("mode_conversion_count") != 0:
        raise AssertionError(f"{label} converted image modes: {receipt}")
    records = core.take_gpu_shader_coverage()
    dispatches = sum(
        record["dispatches"]
        for record in records
        if "max_filter" in record["shader_file"]
    )
    if dispatches != expected_dispatches:
        raise AssertionError(
            f"{label} issued {dispatches} MaxFilter shader dispatches; "
            f"expected {expected_dispatches}: {records}"
        )


def run_target(expected_path: Path) -> None:
    from PIL import Image, ImageBatch, ImageFilter
    import pillow_rs._core as core

    expected = json.loads(expected_path.read_text())
    if expected.get("pillow_version") != "12.2.0":
        raise RuntimeError("oracle artifact version mismatch")

    for backend in ("cpu", "simd", "gpu"):
        for selected in ("cpu", "simd", "gpu"):
            core.disable_backend(selected)
        if not core.enable_backend(backend):
            raise RuntimeError(f"{backend} backend unavailable for MaxFilter parity")
        core.set_pipeline_telemetry(True)
        core.set_gpu_shader_coverage(True)

        for mode in MODES:
            small = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for width, height, seed in SMALL:
                image = Image.frombytes(
                    mode, (width, height), pixels(mode, (width, height), seed)
                )
                image.info["batch-seed"] = seed
                small.submit(ImageBatch.PipelineOp(image, ImageFilter.MaxFilter(3)))
            actual = small.join()
            require_exact(actual, expected["small"][mode], f"queued {backend}/{mode}")
            if backend == "gpu":
                # Two equal-sized images share one stacked dispatch. The 1x1
                # image has no compatible peer and uses one ordinary dispatch.
                require_gpu_group(core, f"{mode} small mixed-size MaxFilter", 2)
            else:
                receipt = core.take_pipeline_telemetry()
                if receipt is None or receipt.get("actual_backend") != backend:
                    raise AssertionError(f"queued {backend}/{mode} left its backend: {receipt}")
                core.take_gpu_shader_coverage()

            eager = ImageBatch.BatchExecutor(queue=False, backend=backend)
            image = Image.frombytes(mode, (7, 5), pixels(mode, (7, 5), SMALL[0][2]))
            image.info["batch-seed"] = SMALL[0][2]
            eager.submit(ImageBatch.PipelineOp(image, ImageFilter.MaxFilter(3)))
            eager_result = eager.join()
            require_exact(eager_result, expected["small"][mode][:1], f"eager {backend}/{mode}")
            if backend == "gpu":
                require_gpu_group(core, f"{mode} queue=False MaxFilter")
            else:
                receipt = core.take_pipeline_telemetry()
                if receipt is None or receipt.get("actual_backend") != backend:
                    raise AssertionError(f"eager {backend}/{mode} left its backend: {receipt}")
                core.take_gpu_shader_coverage()

            batch = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for seed in range(IMAGE_COUNT):
                batch.submit(ImageBatch.PipelineOp(Image.frombytes(mode, IMAGE_SIZE, benchmark_pixels(mode, seed)), ImageFilter.MaxFilter(3)))
            result = batch.join()
            if [image.tobytes().hex() for image in result] != expected["benchmark"][mode]:
                raise AssertionError(f"{backend}/{mode} 64x64x64 MaxFilter differs from Pillow")
            if any(image.mode != mode or image.size != IMAGE_SIZE for image in result):
                raise AssertionError(f"{backend}/{mode} MaxFilter changed mode or size")
            if backend == "gpu":
                require_gpu_group(core, f"{mode} MaxFilter 64x64 × {IMAGE_COUNT}")
            else:
                receipt = core.take_pipeline_telemetry()
                if receipt is None or receipt.get("actual_backend") != backend:
                    raise AssertionError(f"{backend}/{mode} benchmark cohort left its backend: {receipt}")
                core.take_gpu_shader_coverage()
            print(f"{backend}/{mode} MaxFilter: Pillow parity PASS; 64x64 × {IMAGE_COUNT}")

            batch = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for seed in range(LARGE_IMAGE_COUNT):
                batch.submit(ImageBatch.PipelineOp(Image.frombytes(
                        mode,
                        LARGE_IMAGE_SIZE,
                        benchmark_pixels(mode, seed, LARGE_IMAGE_SIZE),
                    ), ImageFilter.MaxFilter(3)))
            large_result = batch.join()
            if [image.tobytes().hex() for image in large_result] != expected["large"][mode]:
                raise AssertionError(
                    f"{backend}/{mode} 256x256x{LARGE_IMAGE_COUNT} MaxFilter differs from Pillow"
                )
            if any(
                image.mode != mode or image.size != LARGE_IMAGE_SIZE
                for image in large_result
            ):
                raise AssertionError(f"{backend}/{mode} large MaxFilter changed mode or size")
            if backend == "gpu":
                require_gpu_group(
                    core, f"{mode} MaxFilter 256x256 × {LARGE_IMAGE_COUNT}"
                )
            else:
                receipt = core.take_pipeline_telemetry()
                if receipt is None or receipt.get("actual_backend") != backend:
                    raise AssertionError(
                        f"{backend}/{mode} large cohort left its backend: {receipt}"
                    )
                core.take_gpu_shader_coverage()
            print(
                f"{backend}/{mode} MaxFilter: Pillow parity PASS; "
                f"256x256 × {LARGE_IMAGE_COUNT}"
            )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="pillow-rs-max-filter-batch-") as directory:
        expected = Path(directory) / "pillow-expected.json"
        oracle_env = os.environ.copy()
        oracle_env.pop("PYTHONPATH", None)
        oracle_env["MAXFILTER_BATCH_PARITY_MODE"] = "oracle"
        oracle_env["MAXFILTER_BATCH_PARITY_OUTPUT"] = str(expected)
        subprocess.run([sys.executable, str(Path(__file__).resolve())], cwd=ROOT, env=oracle_env, check=True)

        target_env = os.environ.copy()
        target_env["PYTHONPATH"] = os.pathsep.join(
            (str(ROOT / "scripts"), str(ROOT / "pillow-rs-py/python"))
        )
        target_env["MAXFILTER_BATCH_PARITY_MODE"] = "target"
        target_env["MAXFILTER_BATCH_PARITY_EXPECTED"] = str(expected)
        subprocess.run([sys.executable, str(Path(__file__).resolve())], cwd=ROOT, env=target_env, check=True)
    return 0


if __name__ == "__main__":
    mode = os.environ.get("MAXFILTER_BATCH_PARITY_MODE")
    if mode == "oracle":
        run_oracle(Path(os.environ["MAXFILTER_BATCH_PARITY_OUTPUT"]))
        raise SystemExit(0)
    if mode == "target":
        run_target(Path(os.environ["MAXFILTER_BATCH_PARITY_EXPECTED"]))
        raise SystemExit(0)
    raise SystemExit(main())

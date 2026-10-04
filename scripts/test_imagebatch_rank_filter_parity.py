#!/usr/bin/env python3
"""Check explicit native-L RankFilter batches against isolated Pillow bytes.

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
SMALL = ((33, 35, 9), (33, 35, 241), (1, 11, 73))
LARGE_SIZE = (256, 256)
LARGE_IMAGE_COUNT = 16
FALLBACK = (
    ("LA", (7, 5), 31, 3, 1),
    ("L", (7, 5), 67, 3, 0),
    ("L", (7, 5), 149, 5, 24),
)


def pixels(size: tuple[int, int], seed: int) -> bytes:
    width, height = size
    if seed in (9, 241):
        return bytes((seed,)) * width * height
    return bytes(
        (index * 37 + index // width * 11 + seed) % 5
        for index in range(width * height)
    )


def material_pixels(seed: int) -> bytes:
    width, height = LARGE_SIZE
    return bytes(
        (index * 73 + index // 13 * 31 + seed * 47 + (seed >> 2)) & 255
        for index in range(width * height)
    )


def fallback_pixels(mode: str, size: tuple[int, int], seed: int) -> bytes:
    channels = {"L": 1, "LA": 2}[mode]
    count = size[0] * size[1] * channels
    return bytes((index * 29 + seed * 41 + index // 7) & 255 for index in range(count))


def run_oracle(output: Path) -> None:
    import PIL
    from PIL import Image, ImageFilter

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"unexpected Pillow oracle version: {PIL.__version__}")

    expected = {
        "pillow_version": PIL.__version__,
        "small": [],
        "eager": None,
        "large": [],
        "fallback": [],
    }
    for width, height, seed in SMALL:
        image = Image.frombytes("L", (width, height), pixels((width, height), seed))
        image.info["batch-seed"] = seed
        result = image.filter(ImageFilter.RankFilter(3, rank=1))
        expected["small"].append(
            {
                "bytes": result.tobytes().hex(),
                "mode": result.mode,
                "size": list(result.size),
                "info": result.info.get("batch-seed"),
            }
        )

    eager_image = Image.frombytes("L", (33, 35), pixels((33, 35), 73))
    eager_result = eager_image.filter(ImageFilter.RankFilter(3, rank=1))
    expected["eager"] = {
        "bytes": eager_result.tobytes().hex(),
        "mode": eager_result.mode,
        "size": list(eager_result.size),
    }

    expected["large"] = [
        Image.frombytes("L", LARGE_SIZE, material_pixels(seed))
        .filter(ImageFilter.RankFilter(3, rank=1))
        .tobytes()
        .hex()
        for seed in range(LARGE_IMAGE_COUNT)
    ]

    for mode, size, seed, filter_size, rank in FALLBACK:
        image = Image.frombytes(mode, size, fallback_pixels(mode, size, seed))
        result = image.filter(ImageFilter.RankFilter(filter_size, rank=rank))
        expected["fallback"].append(
            {
                "bytes": result.tobytes().hex(),
                "mode": result.mode,
                "size": list(result.size),
            }
        )
    output.write_text(json.dumps(expected))


def require_exact(actual, expected, label: str, check_info: bool = False) -> None:
    if len(actual) != len(expected):
        raise AssertionError(f"{label} returned {len(actual)} images; expected {len(expected)}")
    for index, (image, reference) in enumerate(zip(actual, expected, strict=True)):
        if (
            image.mode != reference["mode"]
            or list(image.size) != reference["size"]
            or image.tobytes().hex() != reference["bytes"]
            or (check_info and image.info.get("batch-seed") != reference["info"])
        ):
            raise AssertionError(f"{label} output {index} differs from Pillow")


def require_backend(core, backend: str, label: str, expected_shader_dispatches: int) -> None:
    receipt = core.take_pipeline_telemetry()
    if (
        receipt is None
        or receipt.get("actual_backend") != backend
        or receipt.get("fallback_reason")
        or (backend == "gpu" and receipt.get("dispatch_count") != 1)
    ):
        raise AssertionError(f"{label} did not use {backend} as expected: {receipt}")
    if backend == "gpu":
        resource = receipt.get("resource")
        if not isinstance(resource, dict) or resource.get("mode_conversion_count") != 0:
            raise AssertionError(f"{label} converted L mode: {receipt}")
        records = core.take_gpu_shader_coverage()
        dispatches = sum(
            record["dispatches"]
            for record in records
            if "rank_filter_3x3_luma_packed" in record["shader_file"]
        )
        if dispatches != expected_shader_dispatches:
            raise AssertionError(
                f"{label} issued {dispatches} packed-L RankFilter dispatches; "
                f"expected {expected_shader_dispatches}: {records}"
            )
    else:
        core.take_gpu_shader_coverage()


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
            raise RuntimeError(f"{backend} backend unavailable for RankFilter batch parity")
        core.set_pipeline_telemetry(True)
        core.set_gpu_shader_coverage(True)

        queued = ImageBatch.BatchExecutor(queue=True, backend=backend)
        for width, height, seed in SMALL:
            image = Image.frombytes("L", (width, height), pixels((width, height), seed))
            image.info["batch-seed"] = seed
            queued.submit(image, ImageFilter.RankFilter(3, rank=1))
        small_result = queued.join()
        require_exact(small_result, expected["small"], f"queued {backend}/L", check_info=True)
        require_backend(core, backend, f"queued {backend}/L mixed shapes", 2 if backend == "gpu" else 0)
        print(f"{backend}/L queued RankFilter: Pillow parity PASS; odd tails and isolated fallback")

        eager = ImageBatch.BatchExecutor(queue=False, backend=backend)
        image = Image.frombytes("L", (33, 35), pixels((33, 35), 73))
        eager.submit(image, ImageFilter.RankFilter(3, rank=1))
        eager_result = eager.join()
        require_exact(eager_result, [expected["eager"]], f"eager {backend}/L")
        require_backend(core, backend, f"eager {backend}/L", 1 if backend == "gpu" else 0)

        large = ImageBatch.BatchExecutor(queue=True, backend=backend)
        for seed in range(LARGE_IMAGE_COUNT):
            large.submit(
                Image.frombytes("L", LARGE_SIZE, material_pixels(seed)),
                ImageFilter.RankFilter(3, rank=1),
            )
        large_result = large.join()
        require_exact(
            large_result,
            [
                {"bytes": data, "mode": "L", "size": list(LARGE_SIZE)}
                for data in expected["large"]
            ],
            f"queued {backend}/L 256x256x{LARGE_IMAGE_COUNT}",
        )
        require_backend(
            core,
            backend,
            f"queued {backend}/L large group",
            1 if backend == "gpu" else 0,
        )

    core.disable_backend("simd")
    core.disable_backend("gpu")
    core.enable_backend("cpu")
    fallback = ImageBatch.BatchExecutor(queue=True, backend="cpu")
    for mode, size, seed, filter_size, rank in FALLBACK:
        fallback.submit(
            Image.frombytes(mode, size, fallback_pixels(mode, size, seed)),
            ImageFilter.RankFilter(filter_size, rank=rank),
        )
    fallback_result = fallback.join()
    require_exact(fallback_result, expected["fallback"], "queued CPU non-groupable RankFilter")
    receipt = core.take_pipeline_telemetry()
    if receipt is None or receipt.get("actual_backend") != "cpu":
        raise AssertionError(f"non-groupable RankFilter did not use CPU: {receipt}")
    core.take_gpu_shader_coverage()
    print("CPU non-groupable RankFilter settings: Pillow parity PASS")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="pillow-rs-rank-filter-batch-") as directory:
        expected = Path(directory) / "pillow-expected.json"
        oracle_env = os.environ.copy()
        oracle_env.pop("PYTHONPATH", None)
        oracle_env["RANKFILTER_BATCH_PARITY_MODE"] = "oracle"
        oracle_env["RANKFILTER_BATCH_PARITY_OUTPUT"] = str(expected)
        subprocess.run(
            [sys.executable, str(Path(__file__).resolve())],
            cwd=ROOT,
            env=oracle_env,
            check=True,
        )

        target_env = os.environ.copy()
        target_env["PYTHONPATH"] = os.pathsep.join(
            (str(ROOT / "scripts"), str(ROOT / "pillow-rs-py/python"))
        )
        target_env["RANKFILTER_BATCH_PARITY_MODE"] = "target"
        target_env["RANKFILTER_BATCH_PARITY_EXPECTED"] = str(expected)
        subprocess.run(
            [sys.executable, str(Path(__file__).resolve())],
            cwd=ROOT,
            env=target_env,
            check=True,
        )
    return 0


if __name__ == "__main__":
    mode = os.environ.get("RANKFILTER_BATCH_PARITY_MODE")
    if mode == "oracle":
        run_oracle(Path(os.environ["RANKFILTER_BATCH_PARITY_OUTPUT"]))
        raise SystemExit(0)
    if mode == "target":
        run_target(Path(os.environ["RANKFILTER_BATCH_PARITY_EXPECTED"]))
        raise SystemExit(0)
    raise SystemExit(main())

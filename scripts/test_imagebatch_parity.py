#!/usr/bin/env python3
"""Compare the explicit ImageBatch API with isolated live Pillow outputs.

Run after ``make build-parity``. The oracle and target execute in separate
processes so the replacement ``PIL`` namespace cannot shadow the Pillow oracle.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODES = {"L": 1, "LA": 2, "RGB": 3, "RGBA": 4}
SIZES = ((7, 5), (7, 5), (1, 1))
SEEDS = (3, 41, 97)
LARGE_SIZE = (256, 256)
LARGE_SEEDS = {"L": (131, 132), "RGB": (173, 174)}
MULTIPLY_LARGE_IMAGE_COUNT = 16
BOUNDARY_SIZE = (1024, 768)
BOUNDARY_IMAGE_COUNT = 22
BENCHMARK_CHANNELS = {"L": 0, "LA": 1, "RGB": 1, "RGBA": 3}
COLOR3DLUT_WORKLOADS = ((64, 64, 64), (256, 256, 16), (1024, 768, 4))
PASTE_BATCH_SEEDS = (11, 73)
PASTE_LARGE_SIZE = (256, 256)
PASTE_LARGE_IMAGE_COUNT = 16
PASTE_PARALLEL_SIZE = (512, 512)
INVERT_LARGE_SIZE = (1024, 768)
INVERT_LARGE_IMAGE_COUNT = 4
PASTE_FAULT_CONTRACT_REQUIREMENTS = {
    "imagebatch.paste.group-fallback": (
        "A compatible queued native-mode masked Paste group recovers exact ordered "
        "outputs after a dimension or allocation failure, leaves submitted inputs "
        "unchanged, and keeps the executor usable."
    ),
}
PASTE_FAULT_CONTRACT_CASES = (
    {
        "case_id": "imagebatch.paste.group-dimension-failure.fallback",
        "verification": "fault-contract",
        "operation": "ImageBatch.Paste",
        "target_profile": "python-gpu",
        "oracle": "not_applicable",
        "requirements": ("imagebatch.paste.group-fallback",),
        "fault": {
            "point": "image_batch.paste.group_dimension_failure",
            "contract": "grouped-paste-error-falls-back-and-recovers",
        },
        "mode": "RGBA",
        "input_seeds": PASTE_BATCH_SEEDS,
    },
    {
        "case_id": "imagebatch.paste.group-memory-failure.fallback",
        "verification": "fault-contract",
        "operation": "ImageBatch.Paste",
        "target_profile": "python-gpu",
        "oracle": "not_applicable",
        "requirements": ("imagebatch.paste.group-fallback",),
        "fault": {
            "point": "image_batch.paste.group_memory_failure",
            "contract": "grouped-paste-error-falls-back-and-recovers",
        },
        "mode": "RGBA",
        "input_seeds": PASTE_BATCH_SEEDS,
    },
)


def pixels(mode: str, size: tuple[int, int], seed: int) -> bytes:
    count = size[0] * size[1] * MODES[mode]
    return bytes((i * 31 + seed * 47 + (i // 9) * 13) & 255 for i in range(count))


def benchmark_pixels(
    mode: str, seed: int, size: tuple[int, int] = (64, 64)
) -> bytes:
    count = size[0] * size[1] * MODES[mode]
    return bytes(
        (i * 73 + (i // 11) * 19 + seed * 47 + (seed >> 2)) & 255
        for i in range(count)
    )


def multiply_other_pixels(mode: str, size: tuple[int, int], seed: int) -> bytes:
    return pixels(mode, size, seed ^ 0xA5)


def multiply_benchmark_other_pixels(
    mode: str, seed: int, size: tuple[int, int] = (64, 64)
) -> bytes:
    return benchmark_pixels(mode, seed + 137, size)


def boundary_rgba_pixels(seed: int) -> bytes:
    count = BOUNDARY_SIZE[0] * BOUNDARY_SIZE[1] * 4
    return bytes(
        173 if index % 4 == 3 else (index * 31 + seed * 47 + (index // 9) * 13) & 255
        for index in range(count)
    )


def make_batch_color3dlut(ImageFilter):
    def transform(red: float, green: float, blue: float) -> tuple[float, float, float, float]:
        return (
            0.02 + 0.78 * red + 0.12 * green * blue,
            0.04 + 0.75 * green + 0.12 * red * (1.0 - blue),
            0.03 + 0.74 * blue + 0.18 * red * green,
            0.02
            + 0.20 * red
            + 0.28 * green
            + 0.40 * blue
            + 0.10 * red * green * blue,
        )

    return ImageFilter.Color3DLUT.generate(
        17, transform, channels=4, target_mode="RGBA"
    )


def require_gpu_execution(
    core,
    receipt: dict | None,
    label: str,
    expected_shader: str = "median_filter_3x3",
    expected_shader_dispatches: int = 1,
) -> None:
    if (
        receipt is None
        or receipt.get("actual_backend") != "gpu"
        or receipt.get("fallback_reason")
        or receipt.get("operation_count") != 1
        or receipt.get("dispatch_count") != 1
    ):
        raise AssertionError(f"invalid GPU receipt for {label}: {receipt}")
    resource = receipt.get("resource")
    if not isinstance(resource, dict) or resource.get("mode_conversion_count") != 0:
        raise AssertionError(f"{label} changed pixel mode: {receipt}")
    shader_records = core.take_gpu_shader_coverage()
    matching_dispatches = sum(
        record["dispatches"]
        for record in shader_records
        if expected_shader in record["shader_file"]
    )
    if matching_dispatches != expected_shader_dispatches:
        raise AssertionError(
            f"{label} used {matching_dispatches} {expected_shader} shader dispatches; "
            f"expected {expected_shader_dispatches}: {shader_records}"
        )


def run_oracle(output: Path) -> None:
    import PIL
    from PIL import Image, ImageChops, ImageEnhance, ImageFilter, ImageOps

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"unexpected Pillow oracle version: {PIL.__version__}")
    expected: dict[str, list[str]] = {}
    metadata: dict[str, list[int | None]] = {}
    benchmark_outputs: dict[str, list[str]] = {}
    brightness_outputs: dict[str, list[str]] = {}
    brightness_metadata: dict[str, list[int | None]] = {}
    brightness_benchmark_outputs: dict[str, list[str]] = {}
    extract_outputs: dict[str, list[str]] = {}
    extract_metadata: dict[str, list[int | None]] = {}
    extract_benchmark_outputs: dict[str, list[str]] = {}
    multiply_outputs: dict[str, list[str]] = {}
    multiply_metadata: dict[str, list[int | None]] = {}
    multiply_benchmark_outputs: dict[str, list[str]] = {}
    multiply_large_outputs: dict[str, list[str]] = {}
    color3dlut_outputs: list[str] = []
    color3dlut_metadata: list[int | None] = []
    color3dlut_benchmark_outputs: list[str] = []
    color3dlut_workload_outputs: dict[str, list[str]] = {}
    paste_batch_outputs: dict[str, list[str]] = {}
    paste_batch_metadata: dict[str, list[int | None]] = {}
    paste_benchmark_outputs: dict[str, list[str]] = {}
    paste_large_outputs: dict[str, list[str]] = {}
    paste_parallel_outputs: dict[str, list[str]] = {}
    invert_outputs: dict[str, list[str]] = {}
    invert_metadata: dict[str, list[int | None]] = {}
    invert_large_rgb_outputs: list[str] = []
    large_outputs: dict[str, list[str]] = {}
    large_metadata: dict[str, list[int | None]] = {}
    for mode in MODES:
        expected[mode] = []
        metadata[mode] = []
        brightness_outputs[mode] = []
        brightness_metadata[mode] = []
        multiply_outputs[mode] = []
        multiply_metadata[mode] = []
        multiply_large_outputs[mode] = []
        for size, seed in zip(SIZES, SEEDS, strict=True):
            image = Image.frombytes(mode, size, pixels(mode, size, seed))
            image.info["batch-seed"] = seed
            result = image.filter(ImageFilter.MedianFilter(3))
            expected[mode].append(result.tobytes().hex())
            metadata[mode].append(result.info.get("batch-seed"))
            brightened = ImageEnhance.Brightness(image).enhance(0.5)
            brightness_outputs[mode].append(brightened.tobytes().hex())
            brightness_metadata[mode].append(brightened.info.get("batch-seed"))
            other = Image.frombytes(
                mode, size, multiply_other_pixels(mode, size, seed)
            )
            multiplied = ImageChops.multiply(image, other)
            multiply_outputs[mode].append(multiplied.tobytes().hex())
            multiply_metadata[mode].append(multiplied.info.get("batch-seed"))
        benchmark_outputs[mode] = []
        brightness_benchmark_outputs[mode] = []
        multiply_benchmark_outputs[mode] = []
        for seed in range(64):
            image = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed))
            benchmark_outputs[mode].append(
                image.filter(ImageFilter.MedianFilter(3)).tobytes().hex()
            )
            brightness_benchmark_outputs[mode].append(
                ImageEnhance.Brightness(image).enhance(0.5).tobytes().hex()
            )
            other = Image.frombytes(
                mode,
                (64, 64),
                multiply_benchmark_other_pixels(mode, seed),
            )
            multiply_benchmark_outputs[mode].append(
                ImageChops.multiply(image, other).tobytes().hex()
            )
        for seed in range(MULTIPLY_LARGE_IMAGE_COUNT):
            image = Image.frombytes(
                mode,
                LARGE_SIZE,
                benchmark_pixels(mode, seed, LARGE_SIZE),
            )
            other = Image.frombytes(
                mode,
                LARGE_SIZE,
                multiply_benchmark_other_pixels(mode, seed, LARGE_SIZE),
            )
            multiply_large_outputs[mode].append(
                ImageChops.multiply(image, other).tobytes().hex()
            )
        for channel in range(MODES[mode]):
            key = f"{mode}:{channel}"
            extract_outputs[key] = []
            extract_metadata[key] = []
            for size, seed in zip(SIZES[:2], SEEDS[:2], strict=True):
                image = Image.frombytes(mode, size, pixels(mode, size, seed))
                image.info["batch-seed"] = seed
                result = image.getchannel(channel)
                extract_outputs[key].append(result.tobytes().hex())
                extract_metadata[key].append(result.info.get("batch-seed"))
        benchmark_channel = BENCHMARK_CHANNELS[mode]
        key = f"{mode}:{benchmark_channel}"
        extract_benchmark_outputs[key] = []
        for seed in range(64):
            image = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed))
            extract_benchmark_outputs[key].append(
                image.getchannel(benchmark_channel).tobytes().hex()
            )
    for mode in ("L", "RGB"):
        invert_outputs[mode] = []
        invert_metadata[mode] = []
        for size, seed in zip(SIZES[:2], SEEDS[:2], strict=True):
            image = Image.frombytes(mode, size, pixels(mode, size, seed))
            image.info["batch-seed"] = seed
            result = ImageOps.invert(image)
            invert_outputs[mode].append(result.tobytes().hex())
            invert_metadata[mode].append(result.info.get("batch-seed"))
    for seed in range(INVERT_LARGE_IMAGE_COUNT):
        image = Image.frombytes(
            "RGB",
            INVERT_LARGE_SIZE,
            benchmark_pixels("RGB", seed + 201, INVERT_LARGE_SIZE),
        )
        image.info["batch-seed"] = seed
        result = ImageOps.invert(image)
        invert_large_rgb_outputs.append(result.tobytes().hex())
    for mode, seeds in LARGE_SEEDS.items():
        large_outputs[mode] = []
        large_metadata[mode] = []
        for seed in seeds:
            image = Image.frombytes(mode, LARGE_SIZE, pixels(mode, LARGE_SIZE, seed))
            image.info["batch-seed"] = seed
            result = image.filter(ImageFilter.MedianFilter(3))
            large_outputs[mode].append(result.tobytes().hex())
            large_metadata[mode].append(result.info.get("batch-seed"))
    for mode in MODES:
        paste_batch_outputs[mode] = []
        paste_batch_metadata[mode] = []
        for seed in PASTE_BATCH_SEEDS:
            destination = Image.frombytes(
                mode, (64, 64), benchmark_pixels(mode, seed)
            )
            destination.info["paste-seed"] = seed
            source = Image.frombytes(
                mode, (64, 64), benchmark_pixels(mode, seed + 101)
            )
            mask = Image.frombytes(
                "L", (64, 64), benchmark_pixels("L", seed + 211)
            )
            result = destination.copy()
            result.paste(source, (0, 0), mask)
            paste_batch_outputs[mode].append(result.tobytes().hex())
            paste_batch_metadata[mode].append(result.info.get("paste-seed"))
        paste_benchmark_outputs[mode] = []
        for seed in range(64):
            destination = Image.frombytes(
                mode, (64, 64), benchmark_pixels(mode, seed)
            )
            source = Image.frombytes(
                mode,
                (64, 64),
                multiply_benchmark_other_pixels(mode, seed),
            )
            mask = Image.frombytes("L", (64, 64), benchmark_pixels("L", seed))
            result = destination.copy()
            result.paste(source, (0, 0), mask)
            paste_benchmark_outputs[mode].append(result.tobytes().hex())
        paste_large_outputs[mode] = []
        for seed in range(PASTE_LARGE_IMAGE_COUNT):
            destination = Image.frombytes(
                mode,
                PASTE_LARGE_SIZE,
                benchmark_pixels(mode, seed, PASTE_LARGE_SIZE),
            )
            source = Image.frombytes(
                mode,
                PASTE_LARGE_SIZE,
                multiply_benchmark_other_pixels(mode, seed, PASTE_LARGE_SIZE),
            )
            mask = Image.frombytes(
                "L",
                PASTE_LARGE_SIZE,
                benchmark_pixels("L", seed, PASTE_LARGE_SIZE),
            )
            result = destination.copy()
            result.paste(source, (0, 0), mask)
            paste_large_outputs[mode].append(result.tobytes().hex())
        paste_parallel_outputs[mode] = []
        for seed in range(1):
            destination = Image.frombytes(
                mode,
                PASTE_PARALLEL_SIZE,
                benchmark_pixels(mode, seed, PASTE_PARALLEL_SIZE),
            )
            destination.info["paste-seed"] = seed
            source = Image.frombytes(
                mode,
                PASTE_PARALLEL_SIZE,
                multiply_benchmark_other_pixels(mode, seed, PASTE_PARALLEL_SIZE),
            )
            mask = Image.frombytes(
                "L",
                PASTE_PARALLEL_SIZE,
                benchmark_pixels("L", seed, PASTE_PARALLEL_SIZE),
            )
            result = destination.copy()
            result.paste(source, (0, 0), mask)
            paste_parallel_outputs[mode].append(result.tobytes().hex())
    color3dlut = make_batch_color3dlut(ImageFilter)
    for size, seed in zip(SIZES, SEEDS, strict=True):
        image = Image.frombytes("RGBA", size, pixels("RGBA", size, seed))
        image.info["batch-seed"] = seed
        result = image.filter(color3dlut)
        color3dlut_outputs.append(result.tobytes().hex())
        color3dlut_metadata.append(result.info.get("batch-seed"))
    for seed in range(64):
        image = Image.frombytes("RGBA", (64, 64), benchmark_pixels("RGBA", seed))
        color3dlut_benchmark_outputs.append(image.filter(color3dlut).tobytes().hex())
    for width, height, image_count in COLOR3DLUT_WORKLOADS[1:]:
        key = f"{width}x{height}x{image_count}"
        color3dlut_workload_outputs[key] = []
        for seed in range(image_count):
            image = Image.frombytes(
                "RGBA",
                (width, height),
                benchmark_pixels("RGBA", seed, (width, height)),
            )
            color3dlut_workload_outputs[key].append(image.filter(color3dlut).tobytes().hex())
    boundary_reference = Image.frombytes(
        "RGBA", BOUNDARY_SIZE, boundary_rgba_pixels(0)
    ).getchannel(3)
    output.write_text(
        json.dumps(
            {
                "pillow_version": PIL.__version__,
                "outputs": expected,
                "metadata": metadata,
                "benchmark_outputs": benchmark_outputs,
                "brightness_outputs": brightness_outputs,
                "brightness_metadata": brightness_metadata,
                "brightness_benchmark_outputs": brightness_benchmark_outputs,
                "extract_outputs": extract_outputs,
                "extract_metadata": extract_metadata,
                "extract_benchmark_outputs": extract_benchmark_outputs,
                "multiply_outputs": multiply_outputs,
                "multiply_metadata": multiply_metadata,
                "multiply_benchmark_outputs": multiply_benchmark_outputs,
                "multiply_large_outputs": multiply_large_outputs,
                "large_outputs": large_outputs,
                "large_metadata": large_metadata,
                "color3dlut_outputs": color3dlut_outputs,
                "color3dlut_metadata": color3dlut_metadata,
                "color3dlut_benchmark_outputs": color3dlut_benchmark_outputs,
                "color3dlut_workload_outputs": color3dlut_workload_outputs,
                "paste_batch_outputs": paste_batch_outputs,
                "paste_batch_metadata": paste_batch_metadata,
                "paste_benchmark_outputs": paste_benchmark_outputs,
                "paste_large_outputs": paste_large_outputs,
                "paste_parallel_outputs": paste_parallel_outputs,
                "invert_outputs": invert_outputs,
                "invert_metadata": invert_metadata,
                "invert_large_rgb_outputs": invert_large_rgb_outputs,
                "boundary_rgba_extract_alpha": boundary_reference.tobytes().hex(),
            }
        )
    )


def run_target(expected_path: Path) -> None:
    from PIL import Image, ImageBatch, ImageFilter
    import pillow_rs._core as core

    expected = json.loads(expected_path.read_text())
    if expected["pillow_version"] != "12.2.0":
        raise RuntimeError("oracle artifact version mismatch")
    for backend in ("cpu", "simd", "gpu"):
        core.disable_backend(backend)
    if not core.enable_backend("gpu"):
        raise RuntimeError("GPU backend unavailable for the required batch parity lane")
    core.set_pipeline_telemetry(True)
    core.set_gpu_shader_coverage(True)

    # ImageBatch.Invert is an explicit wrapper around ImageOps.invert. Its
    # grouping contract is L/RGB only; queue=False and non-GPU queues keep the
    # usual single-image implementation and error behavior.
    for backend in ("cpu", "simd"):
        for selected in ("cpu", "simd", "gpu"):
            core.disable_backend(selected)
        if not core.enable_backend(backend):
            raise AssertionError(f"{backend} backend unavailable for ImageOps.invert parity")
        for mode in ("L", "RGB"):
            order = (1, 0)
            queued = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for input_index in order:
                image = Image.frombytes(
                    mode,
                    SIZES[input_index],
                    pixels(mode, SIZES[input_index], SEEDS[input_index]),
                )
                image.info["batch-seed"] = SEEDS[input_index]
                queued.submit(image, ImageBatch.Invert())
            results = queued.join()
            if [image.tobytes().hex() for image in results] != [
                expected["invert_outputs"][mode][index] for index in order
            ]:
                raise AssertionError(f"queued {backend}/{mode} ImageOps.invert differs from Pillow")
            if any(image.mode != mode for image in results) or [image.size for image in results] != [
                SIZES[index] for index in order
            ]:
                raise AssertionError(f"queued {backend}/{mode} ImageOps.invert changed mode or size")
            if [image.info.get("batch-seed") for image in results] != [
                expected["invert_metadata"][mode][index] for index in order
            ]:
                raise AssertionError(f"queued {backend}/{mode} ImageOps.invert changed info")
        print(f"{backend} queued ImageOps.invert L/RGB: Pillow bytes/mode/size/info parity PASS")

    for selected in ("cpu", "simd", "gpu"):
        core.disable_backend(selected)
    if not core.enable_backend("gpu"):
        raise AssertionError("GPU backend unavailable for ImageOps.invert parity")
    for mode in ("L", "RGB"):
        order = (1, 0)
        eager = ImageBatch.BatchExecutor(queue=False, backend="gpu")
        for input_index in order:
            image = Image.frombytes(
                mode,
                SIZES[input_index],
                pixels(mode, SIZES[input_index], SEEDS[input_index]),
            )
            image.info["batch-seed"] = SEEDS[input_index]
            eager.submit(image, ImageBatch.Invert())
        results = eager.join()
        if [image.tobytes().hex() for image in results] != [
            expected["invert_outputs"][mode][index] for index in order
        ]:
            raise AssertionError(f"queue=False GPU/{mode} ImageOps.invert differs from Pillow")
        if any(image.mode != mode for image in results):
            raise AssertionError(f"queue=False GPU/{mode} ImageOps.invert changed mode")
        print(f"queue=False GPU ImageOps.invert {mode}: Pillow byte/mode parity PASS")

    # Two small L inputs and four material RGB inputs each form one native-mode
    # stack and must execute as exactly one solarize.wgsl invert variant.
    for mode, size, expected_outputs, seeds, metadata_seeds in (
        ("L", (7, 5), expected["invert_outputs"]["L"], (3, 41), (3, 41)),
        (
            "RGB",
            INVERT_LARGE_SIZE,
            expected["invert_large_rgb_outputs"],
            tuple(range(201, 201 + INVERT_LARGE_IMAGE_COUNT)),
            tuple(range(INVERT_LARGE_IMAGE_COUNT)),
        ),
    ):
        core.take_gpu_shader_coverage()
        core.take_pipeline_telemetry()
        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed_index, seed in enumerate(seeds):
            image = Image.frombytes(
                mode,
                size,
                pixels(mode, size, seed) if size == SIZES[0]
                else benchmark_pixels(mode, seed, size),
            )
            image.info["batch-seed"] = metadata_seeds[seed_index]
            batch.submit(image, ImageBatch.Invert())
        results = batch.join()
        actual = [image.tobytes().hex() for image in results]
        wanted = expected_outputs
        if actual != wanted:
            raise AssertionError(f"queued GPU/{mode} ImageOps.invert differs from Pillow")
        if any(image.mode != mode or image.size != size for image in results):
            raise AssertionError(f"queued GPU/{mode} ImageOps.invert changed mode or size")
        if [image.info.get("batch-seed") for image in results] != list(metadata_seeds):
            raise AssertionError(f"queued GPU/{mode} ImageOps.invert changed submission order/info")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"{mode} ImageOps.invert queued group {len(seeds)} images",
            expected_shader="solarize.wgsl",
        )
        print(
            f"{mode} ImageOps.invert queued × {len(seeds)}: Pillow bytes/mode/size/info parity PASS; "
            "one actual GPU dispatch, zero mode conversions"
        )

    for backend in ("cpu", "simd"):
        for selected in ("cpu", "simd", "gpu"):
            core.disable_backend(selected)
        if not core.enable_backend(backend):
            raise AssertionError(f"{backend} backend unavailable for Brightness parity")
        modes = MODES if backend == "cpu" else {mode: MODES[mode] for mode in ("L", "LA", "RGB")}
        for mode in modes:
            submission_order = (2, 0, 1)
            small = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for input_index in submission_order:
                image = Image.frombytes(
                    mode,
                    SIZES[input_index],
                    pixels(mode, SIZES[input_index], SEEDS[input_index]),
                )
                image.info["batch-seed"] = SEEDS[input_index]
                small.submit(image, ImageBatch.Brightness(0.5))
            small_actual = small.join()
            if [image.tobytes().hex() for image in small_actual] != [
                expected["brightness_outputs"][mode][index]
                for index in submission_order
            ]:
                raise AssertionError(f"small Pillow Brightness mismatch for {backend}/{mode}")
            if [image.size for image in small_actual] != [
                SIZES[index] for index in submission_order
            ] or any(image.mode != mode for image in small_actual):
                raise AssertionError(f"small {backend}/{mode} Brightness changed mode or size")
            if [image.info.get("batch-seed") for image in small_actual] != [
                expected["brightness_metadata"][mode][index]
                for index in submission_order
            ]:
                raise AssertionError(f"small {backend}/{mode} Brightness info differs from Pillow")

            batch = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for seed in range(64):
                image = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed))
                image.info["batch-seed"] = seed
                batch.submit(image, ImageBatch.Brightness(0.5))
            actual = batch.join()
            if [image.tobytes().hex() for image in actual] != expected[
                "brightness_benchmark_outputs"
            ][mode]:
                raise AssertionError(f"64x64 × 64 Pillow Brightness mismatch for {backend}/{mode}")
            if any(
                image.mode != mode or image.size != (64, 64)
                for image in actual
            ):
                raise AssertionError(f"{backend}/{mode} Brightness changed mode or size")
            if any(image.info.get("batch-seed") is not None for image in actual):
                raise AssertionError(f"{backend}/{mode} Brightness metadata differs from Pillow")
            print(f"{backend} queued Brightness {mode} 64x64 × 64: Pillow parity PASS")
        if backend == "cpu":
            eager_source = Image.frombytes("LA", SIZES[0], pixels("LA", SIZES[0], SEEDS[0]))
            eager_source.info["batch-seed"] = SEEDS[0]
            eager = ImageBatch.BatchExecutor(queue=False, backend="cpu")
            if eager.submit(eager_source, ImageBatch.Brightness(0.5)) != 0:
                raise AssertionError("queue=False Brightness returned an invalid index")
            eager_result = eager.join()[0]
            if (
                eager_result.tobytes().hex() != expected["brightness_outputs"]["LA"][0]
                or eager_result.mode != "LA"
                or eager_result.size != SIZES[0]
                or eager_result.info.get("batch-seed")
                != expected["brightness_metadata"]["LA"][0]
            ):
                raise AssertionError("queue=False LA Brightness differs from Pillow")
            print("CPU queue=False LA Brightness: Pillow parity PASS; eager route")
        receipt = core.take_pipeline_telemetry()
        if (
            receipt is None
            or receipt.get("actual_backend") != backend
            or receipt.get("fallback_reason")
        ):
            raise AssertionError(f"queued Brightness did not use {backend}: {receipt}")

    for selected in ("cpu", "simd", "gpu"):
        core.disable_backend(selected)
    if not core.enable_backend("gpu"):
        raise AssertionError("GPU backend unavailable after Brightness CPU/SIMD parity")

    for mode in MODES:
        # The first 1x1 image uses the ordinary single-image path. The next
        # two equal-size images share one GPU dispatch in this same join.
        submission_order = (2, 0, 1)
        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for input_index in submission_order:
            size, seed = SIZES[input_index], SEEDS[input_index]
            image = Image.frombytes(mode, size, pixels(mode, size, seed))
            image.info["batch-seed"] = seed
            batch.submit(image, ImageFilter.MedianFilter(3))
        actual = batch.join()
        outputs = [image.tobytes().hex() for image in actual]
        expected_outputs = [expected["outputs"][mode][index] for index in submission_order]
        if outputs != expected_outputs:
            raise AssertionError(f"Pillow byte mismatch for mode {mode}")
        metadata = [image.info.get("batch-seed") for image in actual]
        expected_metadata = [expected["metadata"][mode][index] for index in submission_order]
        if metadata != expected_metadata:
            raise AssertionError(f"Pillow info mismatch for mode {mode}: {metadata}")

        receipt = core.take_pipeline_telemetry()
        require_gpu_execution(
            core,
            receipt,
            mode,
            expected_shader="median_filter_3x3",
            expected_shader_dispatches=2,
        )
        print(
            f"{mode}: Pillow parity PASS; actual GPU; compatible pair grouped in one dispatch; "
            "incompatible job used one single-image dispatch"
        )

        benchmark = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed in range(64):
            benchmark.submit(
                Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed)),
                ImageFilter.MedianFilter(3),
            )
        actual = benchmark.join()
        outputs = [image.tobytes().hex() for image in actual]
        if outputs != expected["benchmark_outputs"][mode]:
            raise AssertionError(f"64x64 Pillow byte mismatch for {mode}")
        receipt = core.take_pipeline_telemetry()
        require_gpu_execution(core, receipt, f"{mode} 64x64 × 64")
        print(f"{mode} 64x64 × 64: Pillow parity PASS; actual GPU; grouped dispatch=1")

    for mode, seeds in LARGE_SEEDS.items():
        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for image_seed in seeds:
            image = Image.frombytes(
                mode, LARGE_SIZE, pixels(mode, LARGE_SIZE, image_seed)
            )
            image.info["batch-seed"] = image_seed
            batch.submit(image, ImageFilter.MedianFilter(3))
        actual = batch.join()
        output = [image.tobytes().hex() for image in actual]
        if output != expected["large_outputs"][mode]:
            raise AssertionError(f"256x256 Pillow byte mismatch for {mode}")
        receipt = core.take_pipeline_telemetry()
        require_gpu_execution(core, receipt, f"{mode} 256x256")
        metadata = [image.info.get("batch-seed") for image in actual]
        if metadata != expected["large_metadata"][mode]:
            raise AssertionError(f"256x256 Pillow info mismatch for {mode}")
        print(f"{mode} 256x256: Pillow parity PASS; actual GPU; grouped dispatch=1")

    shared_lut = ImageBatch.Color3DLUT(make_batch_color3dlut(ImageFilter))
    lut_order = (1, 0, 2)
    lut_batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
    for input_index in lut_order:
        size, seed = SIZES[input_index], SEEDS[input_index]
        image = Image.frombytes("RGBA", size, pixels("RGBA", size, seed))
        image.info["batch-seed"] = seed
        lut_batch.submit(image, shared_lut)
    lut_actual = lut_batch.join()
    if [image.mode for image in lut_actual] != ["RGBA"] * len(lut_order):
        raise AssertionError("Color3DLUT batch changed the RGBA output mode")
    if [image.size for image in lut_actual] != [SIZES[index] for index in lut_order]:
        raise AssertionError("Color3DLUT batch changed output dimensions")
    if [image.tobytes().hex() for image in lut_actual] != [
        expected["color3dlut_outputs"][index] for index in lut_order
    ]:
        raise AssertionError("small Color3DLUT batch differs from Pillow")
    if [image.info.get("batch-seed") for image in lut_actual] != [
        expected["color3dlut_metadata"][index] for index in lut_order
    ]:
        raise AssertionError("Color3DLUT batch changed per-image info")
    require_gpu_execution(
        core,
        core.take_pipeline_telemetry(),
        "RGBA Color3DLUT compatible pair and singleton",
        expected_shader="color_3dlut.wgsl",
        expected_shader_dispatches=2,
    )
    print("RGBA Color3DLUT: Pillow bytes, mode, size, order, and info PASS")

    for width, height, image_count in COLOR3DLUT_WORKLOADS:
        key = f"{width}x{height}x{image_count}"
        expected_outputs = (
            expected["color3dlut_benchmark_outputs"]
            if (width, height, image_count) == COLOR3DLUT_WORKLOADS[0]
            else expected["color3dlut_workload_outputs"][key]
        )
        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed in range(image_count):
            batch.submit(
                Image.frombytes(
                    "RGBA",
                    (width, height),
                    benchmark_pixels("RGBA", seed, (width, height)),
                ),
                shared_lut,
            )
        actual = batch.join()
        if len(actual) != image_count or any(image.mode != "RGBA" for image in actual):
            raise AssertionError(f"{key} Color3DLUT batch returned an invalid mode/count")
        if [image.tobytes().hex() for image in actual] != expected_outputs:
            raise AssertionError(f"{key} RGBA Color3DLUT bytes differ from Pillow")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"RGBA Color3DLUT {key}",
            expected_shader="color_3dlut.wgsl",
        )
        print(f"RGBA Color3DLUT {key}: Pillow parity PASS; one actual GPU dispatch")

    other_lut = ImageBatch.Color3DLUT(make_batch_color3dlut(ImageFilter))
    distinct_lut_batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
    for operation in (shared_lut, other_lut):
        distinct_lut_batch.submit(
            Image.frombytes("RGBA", (7, 5), pixels("RGBA", (7, 5), 41)),
            operation,
        )
    distinct_lut_results = distinct_lut_batch.join()
    if [image.tobytes().hex() for image in distinct_lut_results] != [
        expected["color3dlut_outputs"][1],
        expected["color3dlut_outputs"][1],
    ]:
        raise AssertionError("distinct Color3DLUT instances changed per-image results")
    require_gpu_execution(
        core,
        core.take_pipeline_telemetry(),
        "distinct Color3DLUT instances",
        expected_shader="color_3dlut.wgsl",
        expected_shader_dispatches=2,
    )
    print("distinct Color3DLUT instances: kept separate; two actual GPU dispatches")

    for mode, channels in MODES.items():
        submission_order = (1, 0)
        for channel in range(channels):
            key = f"{mode}:{channel}"
            batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
            for input_index in submission_order:
                image = Image.frombytes(
                    mode,
                    SIZES[input_index],
                    pixels(mode, SIZES[input_index], SEEDS[input_index]),
                )
                image.info["batch-seed"] = SEEDS[input_index]
                batch.submit(image, ImageBatch.ExtractBand(channel))
            actual = batch.join()
            outputs = [image.tobytes().hex() for image in actual]
            expected_outputs = [
                expected["extract_outputs"][key][index] for index in submission_order
            ]
            if outputs != expected_outputs:
                raise AssertionError(f"Pillow ExtractBand mismatch for {mode} channel {channel}")
            actual_metadata = [image.info.get("batch-seed") for image in actual]
            expected_metadata = [
                expected["extract_metadata"][key][index] for index in submission_order
            ]
            if actual_metadata != expected_metadata:
                raise AssertionError(
                    f"ExtractBand info mismatch for {mode} channel {channel}: {actual_metadata}"
                )
            require_gpu_execution(
                core,
                core.take_pipeline_telemetry(),
                f"{mode} ExtractBand({channel})",
                expected_shader="extract_band.wgsl",
            )
            print(
                f"{mode} ExtractBand({channel}): Pillow parity PASS; actual GPU; grouped dispatch=1"
            )

        channel = BENCHMARK_CHANNELS[mode]
        key = f"{mode}:{channel}"
        benchmark = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed in range(64):
            benchmark.submit(
                Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed)),
                ImageBatch.ExtractBand(channel),
            )
        actual = benchmark.join()
        outputs = [image.tobytes().hex() for image in actual]
        if outputs != expected["extract_benchmark_outputs"][key]:
            raise AssertionError(f"64x64 Pillow ExtractBand mismatch for {mode} channel {channel}")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"{mode} ExtractBand({channel}) 64x64 × 64",
            expected_shader="extract_band.wgsl",
        )
        print(
            f"{mode} ExtractBand({channel}) 64x64 × 64: Pillow parity PASS; grouped dispatch=1"
        )

    for mode in MODES:
        core.take_gpu_shader_coverage()
        submission_order = (2, 0, 1)
        small = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for input_index in submission_order:
            image = Image.frombytes(
                mode,
                SIZES[input_index],
                pixels(mode, SIZES[input_index], SEEDS[input_index]),
            )
            image.info["batch-seed"] = SEEDS[input_index]
            small.submit(image, ImageBatch.Brightness(0.5))
        small_actual = small.join()
        if [image.tobytes().hex() for image in small_actual] != [
            expected["brightness_outputs"][mode][index]
            for index in submission_order
        ]:
            raise AssertionError(f"small Pillow Brightness mismatch for GPU/{mode}")
        if [image.size for image in small_actual] != [
            SIZES[index] for index in submission_order
        ] or any(image.mode != mode for image in small_actual):
            raise AssertionError(f"small GPU/{mode} Brightness changed mode or size")
        if [image.info.get("batch-seed") for image in small_actual] != [
            expected["brightness_metadata"][mode][index]
            for index in submission_order
        ]:
            raise AssertionError(f"small GPU/{mode} Brightness info differs from Pillow")
        receipt = core.take_pipeline_telemetry()
        if mode in ("L", "LA", "RGB"):
            require_gpu_execution(
                core,
                receipt,
                f"{mode} Brightness mixed-size small batch",
                expected_shader="brightness_native.wgsl",
                expected_shader_dispatches=2,
            )
        else:
            core.take_gpu_shader_coverage()

        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed in range(64):
            image = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed))
            image.info["batch-seed"] = seed
            batch.submit(image, ImageBatch.Brightness(0.5))
        actual = batch.join()
        if [image.tobytes().hex() for image in actual] != expected[
            "brightness_benchmark_outputs"
        ][mode]:
            raise AssertionError(f"64x64 × 64 Pillow Brightness mismatch for GPU/{mode}")
        if any(
            image.mode != mode or image.size != (64, 64)
            for image in actual
        ):
            raise AssertionError(f"GPU/{mode} Brightness changed mode or size")
        if any(image.info.get("batch-seed") is not None for image in actual):
            raise AssertionError("GPU Brightness metadata differs from Pillow")
        receipt = core.take_pipeline_telemetry()
        if mode in ("L", "LA", "RGB"):
            require_gpu_execution(
                core,
                receipt,
                f"{mode} Brightness 64x64 × 64",
                expected_shader="brightness_native.wgsl",
            )
            print(
                f"GPU queued Brightness {mode} 64x64 × 64: Pillow parity PASS; "
                "one native-mode dispatch"
            )
        else:
            if (
                receipt is None
                or receipt.get("actual_backend") != "gpu"
                or receipt.get("fallback_reason")
            ):
                raise AssertionError(
                    f"RGBA Brightness should retain its ordinary GPU single-image path: {receipt}"
                )
            print("GPU queued Brightness RGBA: Pillow parity PASS; ordinary single-image route retained")

    # Verify that queue=False executes eagerly on the normal single-image path.
    image = Image.frombytes("L", SIZES[0], pixels("L", SIZES[0], SEEDS[0]))
    eager = ImageBatch.BatchExecutor(queue=False, backend="gpu")
    eager.submit(image, ImageFilter.MedianFilter(3))
    result = eager.join()[0]
    if result.tobytes().hex() != expected["outputs"]["L"][0]:
        raise AssertionError("eager queue=False output differs from Pillow")
    require_gpu_execution(core, core.take_pipeline_telemetry(), "queue=False")
    print("queue=False: Pillow parity PASS; eager single-image execution")

    for mode, channel in BENCHMARK_CHANNELS.items():
        image = Image.frombytes(mode, SIZES[0], pixels(mode, SIZES[0], SEEDS[0]))
        eager = ImageBatch.BatchExecutor(queue=False, backend="gpu")
        eager.submit(image, ImageBatch.ExtractBand(channel))
        result = eager.join()[0]
        if result.tobytes().hex() != expected["extract_outputs"][f"{mode}:{channel}"][0]:
            raise AssertionError(f"queue=False ExtractBand mismatch for {mode} channel {channel}")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"queue=False {mode} ExtractBand({channel})",
            expected_shader="extract_band.wgsl",
        )
        print(f"queue=False {mode} ExtractBand({channel}): Pillow parity PASS; single-image path")

    eager_lut = ImageBatch.BatchExecutor(queue=False, backend="gpu")
    eager_lut.submit(
        Image.frombytes("RGBA", SIZES[0], pixels("RGBA", SIZES[0], SEEDS[0])),
        shared_lut,
    )
    eager_lut_result = eager_lut.join()[0]
    if eager_lut_result.mode != "RGBA" or eager_lut_result.tobytes().hex() != expected[
        "color3dlut_outputs"
    ][0]:
        raise AssertionError("queue=False Color3DLUT differs from Pillow or changed mode")
    require_gpu_execution(
        core,
        core.take_pipeline_telemetry(),
        "queue=False RGBA Color3DLUT",
        expected_shader="color_3dlut.wgsl",
    )
    print("queue=False Color3DLUT: Pillow parity PASS; eager single-image execution")

    for backend in ("cpu", "simd"):
        for selected in ("cpu", "simd", "gpu"):
            core.disable_backend(selected)
        if not core.enable_backend(backend):
            raise AssertionError(f"{backend} backend unavailable for Color3DLUT parity")
        backend_indices = (
            lut_order
            if backend == "cpu"
            else tuple(index for index in lut_order if SIZES[index][0] * SIZES[index][1] >= 8)
        )
        sequential = ImageBatch.BatchExecutor(queue=False, backend=backend)
        for input_index in backend_indices:
            size, seed = SIZES[input_index], SEEDS[input_index]
            image = Image.frombytes("RGBA", size, pixels("RGBA", size, seed))
            image.info["batch-seed"] = seed
            sequential.submit(image, shared_lut)
        actual = sequential.join()
        if [image.tobytes().hex() for image in actual] != [
            expected["color3dlut_outputs"][index] for index in backend_indices
        ]:
            raise AssertionError(f"{backend} eager Color3DLUT differs from Pillow")
        if any(image.mode != "RGBA" for image in actual):
            raise AssertionError(f"{backend} eager Color3DLUT changed the output mode")
        receipt = core.take_pipeline_telemetry()
        if (
            receipt is None
            or receipt.get("actual_backend") != backend
            or receipt.get("fallback_reason")
        ):
            raise AssertionError(f"{backend} Color3DLUT did not use the requested backend: {receipt}")
        resource = receipt.get("resource")
        if isinstance(resource, dict) and resource.get("mode_conversion_count") != 0:
            raise AssertionError(f"{backend} Color3DLUT converted RGBA: {receipt}")
        detail = "; SIMD checked only vector-eligible images" if backend == "simd" else ""
        print(f"queue=False Color3DLUT on {backend}: Pillow parity PASS; no fallback{detail}")

    for selected in ("cpu", "simd", "gpu"):
        core.disable_backend(selected)
    if not core.enable_backend("gpu"):
        raise AssertionError("GPU backend unavailable after Color3DLUT serial parity")

    for mode in MODES:
        submission_order = (1, 0)
        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for input_index in submission_order:
            size, seed = SIZES[input_index], SEEDS[input_index]
            image = Image.frombytes(mode, size, pixels(mode, size, seed))
            image.info["batch-seed"] = seed
            other = Image.frombytes(
                mode,
                size,
                multiply_other_pixels(mode, size, seed),
            )
            batch.submit(image, ImageBatch.Multiply(other))
        actual = batch.join()
        outputs = [image.tobytes().hex() for image in actual]
        expected_outputs = [
            expected["multiply_outputs"][mode][index]
            for index in submission_order
        ]
        if outputs != expected_outputs:
            raise AssertionError(f"Pillow Multiply mismatch for {mode}")
        actual_metadata = [image.info.get("batch-seed") for image in actual]
        expected_metadata = [
            expected["multiply_metadata"][mode][index]
            for index in submission_order
        ]
        if actual_metadata != expected_metadata:
            raise AssertionError(f"Multiply info mismatch for {mode}: {actual_metadata}")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"{mode} Multiply queued pair",
            expected_shader="multiply.wgsl",
        )
        print(f"{mode} Multiply: Pillow byte/info parity PASS; actual GPU; grouped dispatch=1")

        benchmark = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed in range(64):
            image = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed))
            other = Image.frombytes(
                mode,
                (64, 64),
                multiply_benchmark_other_pixels(mode, seed),
            )
            benchmark.submit(image, ImageBatch.Multiply(other))
        actual = benchmark.join()
        outputs = [image.tobytes().hex() for image in actual]
        if outputs != expected["multiply_benchmark_outputs"][mode]:
            raise AssertionError(f"64x64 × 64 Pillow Multiply mismatch for {mode}")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"{mode} Multiply 64x64 × 64",
            expected_shader="multiply.wgsl",
        )
        print(f"{mode} Multiply 64x64 × 64: Pillow parity PASS; grouped dispatch=1")

        large = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed in range(MULTIPLY_LARGE_IMAGE_COUNT):
            image = Image.frombytes(
                mode, LARGE_SIZE, benchmark_pixels(mode, seed, LARGE_SIZE)
            )
            other = Image.frombytes(
                mode,
                LARGE_SIZE,
                multiply_benchmark_other_pixels(mode, seed, LARGE_SIZE),
            )
            large.submit(image, ImageBatch.Multiply(other))
        actual = large.join()
        outputs = [image.tobytes().hex() for image in actual]
        if outputs != expected["multiply_large_outputs"][mode]:
            raise AssertionError(f"256x256 × {MULTIPLY_LARGE_IMAGE_COUNT} Pillow Multiply mismatch for {mode}")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"{mode} Multiply 256x256 × {MULTIPLY_LARGE_IMAGE_COUNT}",
            expected_shader="multiply.wgsl",
        )
        print(
            f"{mode} Multiply 256x256 × {MULTIPLY_LARGE_IMAGE_COUNT}: "
            "Pillow parity PASS; grouped dispatch=1"
        )

        # Eager submission is still a single-image Multiply through its regular
        # path; the batch API only queues/group schedules when explicitly asked.
        size, seed = SIZES[0], SEEDS[0]
        image = Image.frombytes(mode, size, pixels(mode, size, seed))
        other = Image.frombytes(
            mode, size, multiply_other_pixels(mode, size, seed)
        )
        eager = ImageBatch.BatchExecutor(queue=False, backend="gpu")
        eager.submit(image, ImageBatch.Multiply(other))
        result = eager.join()[0]
        if result.tobytes().hex() != expected["multiply_outputs"][mode][0]:
            raise AssertionError(f"queue=False Multiply mismatch for {mode}")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"queue=False {mode} Multiply",
            expected_shader="multiply.wgsl",
        )
        print(f"queue=False {mode} Multiply: Pillow parity PASS; ordinary single-image path")

    # The first 21 compatible images fit the checked GPU image-buffer cap;
    # the 22nd must become its own safe GPU call. RGB bytes vary per image
    # while alpha remains fixed, so exact output plus metadata checks cover
    # both chunk boundaries and returned submission order.
    boundary_expected = bytes.fromhex(expected["boundary_rgba_extract_alpha"])
    boundary_batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
    boundary_metadata = list(range(BOUNDARY_IMAGE_COUNT))
    for seed in boundary_metadata:
        image = Image.frombytes("RGBA", BOUNDARY_SIZE, boundary_rgba_pixels(seed))
        image.info["batch-seed"] = seed
        boundary_batch.submit(image, ImageBatch.ExtractBand(3))
    boundary_results = boundary_batch.join()
    if len(boundary_results) != BOUNDARY_IMAGE_COUNT:
        raise AssertionError("resource-boundary batch omitted an output")
    for index, image in enumerate(boundary_results):
        if image.tobytes() != boundary_expected:
            raise AssertionError(f"resource-boundary alpha mismatch at output {index}")
        if image.info.get("batch-seed") != boundary_metadata[index]:
            raise AssertionError(f"resource-boundary metadata order mismatch at output {index}")
    require_gpu_execution(
        core,
        core.take_pipeline_telemetry(),
        "RGBA ExtractBand(3) 1024x768 × 22 safe resource-boundary chunks",
        expected_shader="extract_band.wgsl",
        expected_shader_dispatches=2,
    )
    print(
        "RGBA ExtractBand(3) 1024x768 × 22: Pillow parity PASS; "
        "two actual GPU dispatches across bounded groups"
    )

    for mode in MODES:
        submission_order = (1, 2, 0)
        eager = ImageBatch.BatchExecutor(queue=False, backend="gpu")
        for input_index in submission_order:
            size, seed = SIZES[input_index], SEEDS[input_index]
            image = Image.frombytes(mode, size, pixels(mode, size, seed))
            image.info["batch-seed"] = seed
            eager.submit(image, ImageFilter.MedianFilter(3))
        actual = eager.join()
        actual_outputs = [image.tobytes().hex() for image in actual]
        expected_outputs = [expected["outputs"][mode][index] for index in submission_order]
        if actual_outputs != expected_outputs:
            raise AssertionError(f"queue=False Pillow byte mismatch for mode {mode}")
        actual_metadata = [image.info.get("batch-seed") for image in actual]
        expected_metadata = [expected["metadata"][mode][index] for index in submission_order]
        if actual_metadata != expected_metadata:
            raise AssertionError(
                f"queue=False Pillow metadata mismatch for mode {mode}: {actual_metadata}"
            )
        print(f"{mode} queue=False × 3: sequential submission order and Pillow parity PASS")

    paste_order = (1, 0)
    for mode in MODES:
        core.take_gpu_shader_coverage()
        core.take_pipeline_telemetry()
        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        submitted_inputs = []
        for input_index in paste_order:
            seed = PASTE_BATCH_SEEDS[input_index]
            destination = Image.frombytes(
                mode, (64, 64), benchmark_pixels(mode, seed)
            )
            destination.info["paste-seed"] = seed
            submitted_inputs.append((destination, destination.tobytes()))
            source = Image.frombytes(
                mode, (64, 64), benchmark_pixels(mode, seed + 101)
            )
            mask = Image.frombytes(
                "L", (64, 64), benchmark_pixels("L", seed + 211)
            )
            batch.submit(destination, ImageBatch.Paste(source, mask))

        actual = batch.join()
        if any(destination.tobytes() != original for destination, original in submitted_inputs):
            raise AssertionError(f"queued {mode} Paste mutated a submitted destination")
        if [image.mode for image in actual] != [mode, mode]:
            raise AssertionError(f"{mode} Paste batch changed output modes")
        if [image.size for image in actual] != [(64, 64), (64, 64)]:
            raise AssertionError(f"{mode} Paste batch changed output sizes")
        if [image.tobytes().hex() for image in actual] != [
            expected["paste_batch_outputs"][mode][index]
            for index in paste_order
        ]:
            raise AssertionError(f"{mode} Paste batch differs from Pillow")
        if [image.info.get("paste-seed") for image in actual] != [
            expected["paste_batch_metadata"][mode][index]
            for index in paste_order
        ]:
            raise AssertionError(f"{mode} Paste batch changed per-image info")

        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"{mode} full-frame masked Paste pair",
            expected_shader=f"paste_native_masked_{mode.lower()}.wgsl",
        )
        print(
            f"{mode} full-frame masked Paste × 2: Pillow bytes/info/order PASS; "
            "one actual native-mode GPU dispatch"
        )

        benchmark = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed in range(64):
            destination = Image.frombytes(
                mode, (64, 64), benchmark_pixels(mode, seed)
            )
            source = Image.frombytes(
                mode,
                (64, 64),
                multiply_benchmark_other_pixels(mode, seed),
            )
            mask = Image.frombytes("L", (64, 64), benchmark_pixels("L", seed))
            benchmark.submit(destination, ImageBatch.Paste(source, mask))
        actual = benchmark.join()
        if [image.tobytes().hex() for image in actual] != expected[
            "paste_benchmark_outputs"
        ][mode]:
            raise AssertionError(f"{mode} 64x64 × 64 Paste differs from Pillow")
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"{mode} 64x64 × 64 masked Paste",
            expected_shader=f"paste_native_masked_{mode.lower()}.wgsl",
        )
        print(f"{mode} 64x64 × 64 masked Paste: Pillow parity PASS; one GPU dispatch")

        large = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        for seed in range(PASTE_LARGE_IMAGE_COUNT):
            destination = Image.frombytes(
                mode,
                PASTE_LARGE_SIZE,
                benchmark_pixels(mode, seed, PASTE_LARGE_SIZE),
            )
            source = Image.frombytes(
                mode,
                PASTE_LARGE_SIZE,
                multiply_benchmark_other_pixels(mode, seed, PASTE_LARGE_SIZE),
            )
            mask = Image.frombytes(
                "L",
                PASTE_LARGE_SIZE,
                benchmark_pixels("L", seed, PASTE_LARGE_SIZE),
            )
            large.submit(destination, ImageBatch.Paste(source, mask))
        actual = large.join()
        if [image.tobytes().hex() for image in actual] != expected[
            "paste_large_outputs"
        ][mode]:
            raise AssertionError(
                f"{mode} 256x256 × {PASTE_LARGE_IMAGE_COUNT} Paste differs from Pillow"
            )
        require_gpu_execution(
            core,
            core.take_pipeline_telemetry(),
            f"{mode} 256x256 × {PASTE_LARGE_IMAGE_COUNT} masked Paste",
            expected_shader=f"paste_native_masked_{mode.lower()}.wgsl",
        )
        print(
            f"{mode} 256x256 × {PASTE_LARGE_IMAGE_COUNT} masked Paste: "
            "Pillow parity PASS; one GPU dispatch"
        )

        for backend in ("cpu", "simd", "gpu"):
            for selected in ("cpu", "simd", "gpu"):
                core.disable_backend(selected)
            if not core.enable_backend(backend):
                raise AssertionError(f"{backend} backend unavailable for {mode} Paste")
            destination = Image.frombytes(
                mode,
                (64, 64),
                benchmark_pixels(mode, PASTE_BATCH_SEEDS[0]),
            )
            destination.info["paste-seed"] = PASTE_BATCH_SEEDS[0]
            original_destination = destination.tobytes()
            source = Image.frombytes(
                mode,
                (64, 64),
                benchmark_pixels(mode, PASTE_BATCH_SEEDS[0] + 101),
            )
            mask = Image.frombytes(
                "L",
                (64, 64),
                benchmark_pixels("L", PASTE_BATCH_SEEDS[0] + 211),
            )
            eager = ImageBatch.BatchExecutor(queue=False, backend=backend)
            eager.submit(destination, ImageBatch.Paste(source, mask))
            result = eager.join()[0]
            if destination.tobytes() != original_destination:
                raise AssertionError(f"queue=False {mode} Paste mutated its destination")
            if (
                result.mode != mode
                or result.size != (64, 64)
                or result.tobytes().hex() != expected["paste_batch_outputs"][mode][0]
                or result.info.get("paste-seed")
                != expected["paste_batch_metadata"][mode][0]
            ):
                raise AssertionError(f"queue=False {mode} Paste differs from Pillow")
            receipt = core.take_pipeline_telemetry()
            if (
                receipt is None
                or receipt.get("actual_backend") != backend
                or receipt.get("fallback_reason")
            ):
                raise AssertionError(
                    f"queue=False {mode} Paste missed {backend}: {receipt}"
                )
            resource = receipt.get("resource")
            if isinstance(resource, dict) and resource.get("mode_conversion_count") != 0:
                raise AssertionError(f"queue=False {mode} Paste converted modes: {receipt}")
            if backend == "gpu":
                require_gpu_execution(
                    core,
                    receipt,
                    f"queue=False {mode} masked Paste",
                    expected_shader=f"paste_native_masked_{mode.lower()}.wgsl",
                )
            print(
                f"queue=False {mode} Paste on {backend}: Pillow bytes/mode/size/info PASS; "
                "requested backend executed without fallback"
            )

        # This exact 512×512 input reaches the feature-gated Rayon threshold
        # in native masked Paste; the default build checks the same contract
        # on its serial CPU path.
        for selected in ("cpu", "simd", "gpu"):
            core.disable_backend(selected)
        if not core.enable_backend("cpu"):
            raise AssertionError(f"CPU backend unavailable for {mode} Paste threshold case")
        destination = Image.frombytes(
            mode,
            PASTE_PARALLEL_SIZE,
            benchmark_pixels(mode, 0, PASTE_PARALLEL_SIZE),
        )
        destination.info["paste-seed"] = 0
        source = Image.frombytes(
            mode,
            PASTE_PARALLEL_SIZE,
            multiply_benchmark_other_pixels(mode, 0, PASTE_PARALLEL_SIZE),
        )
        mask = Image.frombytes(
            "L",
            PASTE_PARALLEL_SIZE,
            benchmark_pixels("L", 0, PASTE_PARALLEL_SIZE),
        )
        threshold = ImageBatch.BatchExecutor(queue=False, backend="cpu")
        threshold.submit(destination, ImageBatch.Paste(source, mask))
        result = threshold.join()[0]
        if (
            result.mode != mode
            or result.size != PASTE_PARALLEL_SIZE
            or result.tobytes().hex() != expected["paste_parallel_outputs"][mode][0]
            or result.info.get("paste-seed") != 0
        ):
            raise AssertionError(f"512×512 native CPU Paste parity mismatch for {mode}")
        receipt = core.take_pipeline_telemetry()
        if (
            receipt is None
            or receipt.get("actual_backend") != "cpu"
            or receipt.get("fallback_reason")
        ):
            raise AssertionError(f"512×512 native CPU Paste fell back for {mode}: {receipt}")
        print(
            f"{mode} CPU Paste 512×512 threshold: Pillow bytes/mode/size/info PASS; "
            "CPU executed without fallback"
        )

    for selected in ("cpu", "simd", "gpu"):
        core.disable_backend(selected)
    if not core.enable_backend("gpu"):
        raise AssertionError("GPU backend unavailable after masked Paste parity")


def assert_grouped_paste_failure_fallback(case: dict, expected: dict) -> None:
    """Check exact public recovery after one injected grouped Paste failure."""

    from PIL import Image, ImageBatch
    import pillow_rs._core as core

    fault_point = case["fault"]["point"]
    if os.environ.get("PILLOW_RS_MIGRATION_FAULT_POINT") != fault_point:
        raise RuntimeError(f"fault point was not selected: {fault_point}")
    if case["mode"] not in MODES or case["input_seeds"] != PASTE_BATCH_SEEDS:
        raise ValueError(f"invalid Paste fault input declaration: {case['case_id']}")

    for backend in ("cpu", "simd", "gpu"):
        core.disable_backend(backend)
    if not core.enable_backend("gpu"):
        raise RuntimeError("GPU backend unavailable for Paste fallback fault contract")
    core.set_pipeline_telemetry(True)
    core.set_gpu_shader_coverage(True)
    core.take_pipeline_telemetry()
    core.take_gpu_shader_coverage()

    mode = case["mode"]
    batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
    submitted_inputs = []
    for seed in case["input_seeds"]:
        destination = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed))
        destination.info["paste-seed"] = seed
        submitted_inputs.append((destination, destination.tobytes()))
        source = Image.frombytes(
            mode, (64, 64), benchmark_pixels(mode, seed + 101)
        )
        mask = Image.frombytes(
            "L", (64, 64), benchmark_pixels("L", seed + 211)
        )
        batch.submit(destination, ImageBatch.Paste(source, mask))

    recovered = batch.join()
    if any(
        destination.tobytes() != original
        for destination, original in submitted_inputs
    ):
        raise AssertionError("Paste fallback mutated a submitted destination")

    expected_by_seed = {
        seed: index for index, seed in enumerate(PASTE_BATCH_SEEDS)
    }
    for image, seed in zip(recovered, case["input_seeds"], strict=True):
        index = expected_by_seed[seed]
        if (
            image.mode != mode
            or image.size != (64, 64)
            or image.tobytes().hex() != expected["paste_batch_outputs"][mode][index]
            or image.info.get("paste-seed")
            != expected["paste_batch_metadata"][mode][index]
        ):
            raise AssertionError(
                f"fault-contract Paste fallback differs from Pillow for seed {seed}"
            )

    shader = f"paste_native_masked_{mode.lower()}.wgsl"
    records = core.take_gpu_shader_coverage()
    dispatches = sum(
        record["dispatches"]
        for record in records
        if shader in record["shader_file"]
    )
    if dispatches != len(case["input_seeds"]):
        raise AssertionError(
            "Paste group failure did not run one GPU fallback per input: "
            f"{records}"
        )

    seed = PASTE_BATCH_SEEDS[0]
    destination = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed))
    destination.info["paste-seed"] = seed
    source = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed + 101))
    mask = Image.frombytes("L", (64, 64), benchmark_pixels("L", seed + 211))
    core.take_pipeline_telemetry()
    core.take_gpu_shader_coverage()
    if batch.submit(destination, ImageBatch.Paste(source, mask)) != 0:
        raise AssertionError("a drained Paste batch did not reset its submission index")
    followup = batch.join()
    index = expected_by_seed[seed]
    if (
        len(followup) != 1
        or followup[0].mode != mode
        or followup[0].size != (64, 64)
        or followup[0].tobytes().hex()
        != expected["paste_batch_outputs"][mode][index]
        or followup[0].info.get("paste-seed")
        != expected["paste_batch_metadata"][mode][index]
    ):
        raise AssertionError("Paste executor follow-up result differs from Pillow")
    require_gpu_execution(
        core,
        core.take_pipeline_telemetry(),
        "fault-contract follow-up Paste join",
        expected_shader=shader,
    )


def run_paste_fault_contracts(expected_path: Path) -> None:
    expected = json.loads(expected_path.read_text())
    if expected.get("pillow_version") != "12.2.0":
        raise RuntimeError("oracle artifact version mismatch")
    contracts = {
        "grouped-paste-error-falls-back-and-recovers":
            assert_grouped_paste_failure_fallback,
    }
    case_id = os.environ.get("PASTE_BATCH_FAULT_CONTRACT_CASE_ID")
    case = next(
        (item for item in PASTE_FAULT_CONTRACT_CASES if item["case_id"] == case_id),
        None,
    )
    if case is None:
        raise ValueError(f"unknown Paste fault-contract case: {case_id!r}")
    if (
        case["verification"] != "fault-contract"
        or case["operation"] != "ImageBatch.Paste"
        or case["target_profile"] != "python-gpu"
        or case["oracle"] != "not_applicable"
        or not case["requirements"]
        or any(
            requirement not in PASTE_FAULT_CONTRACT_REQUIREMENTS
            for requirement in case["requirements"]
        )
    ):
        raise ValueError(f"invalid Paste fault-contract declaration: {case_id}")
    assertion = contracts.get(case["fault"]["contract"])
    if assertion is None:
        raise ValueError(f"unknown Paste fault contract: {case['fault']['contract']}")
    assertion(case, expected)
    print(
        f"fault-contract case={case_id} selected=1 executed=1 passed=1 failed=0 "
        f"requirements={','.join(case['requirements'])} oracle=not_applicable"
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="pillow-rs-imagebatch-") as directory:
        expected = Path(directory) / "pillow-expected.json"

        oracle_env = os.environ.copy()
        oracle_env.pop("PYTHONPATH", None)
        oracle_env["IMAGEBATCH_PARITY_MODE"] = "oracle"
        oracle_env["IMAGEBATCH_PARITY_OUTPUT"] = str(expected)
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
        target_env["IMAGEBATCH_PARITY_MODE"] = "target"
        target_env["IMAGEBATCH_PARITY_EXPECTED"] = str(expected)
        target_env.pop("PILLOW_RS_MIGRATION_FAULT_POINT", None)
        target_env.pop("PASTE_BATCH_FAULT_CONTRACT_CASE_ID", None)
        subprocess.run(
            [sys.executable, str(Path(__file__).resolve())],
            cwd=ROOT,
            env=target_env,
            check=True,
        )
        if os.environ.get("PASTE_BATCH_INCLUDE_FAULT_CONTRACT") == "1":
            for case in PASTE_FAULT_CONTRACT_CASES:
                fault_env = target_env.copy()
                fault_env["IMAGEBATCH_PARITY_MODE"] = "paste-fault-contract"
                fault_env["PILLOW_RS_MIGRATION_FAULT_POINT"] = case["fault"]["point"]
                fault_env["PASTE_BATCH_FAULT_CONTRACT_CASE_ID"] = case["case_id"]
                subprocess.run(
                    [sys.executable, str(Path(__file__).resolve())],
                    cwd=ROOT,
                    env=fault_env,
                    check=True,
                )
    return 0


if __name__ == "__main__":
    mode = os.environ.get("IMAGEBATCH_PARITY_MODE")
    if mode == "oracle":
        run_oracle(Path(os.environ["IMAGEBATCH_PARITY_OUTPUT"]))
        raise SystemExit(0)
    if mode == "target":
        run_target(Path(os.environ["IMAGEBATCH_PARITY_EXPECTED"]))
        raise SystemExit(0)
    if mode == "paste-fault-contract":
        run_paste_fault_contracts(Path(os.environ["IMAGEBATCH_PARITY_EXPECTED"]))
        raise SystemExit(0)
    raise SystemExit(main())

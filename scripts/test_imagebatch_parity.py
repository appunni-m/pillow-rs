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
BENCHMARK_CHANNELS = {"L": 0, "LA": 1, "RGB": 1, "RGBA": 3}


def pixels(mode: str, size: tuple[int, int], seed: int) -> bytes:
    count = size[0] * size[1] * MODES[mode]
    return bytes((i * 31 + seed * 47 + (i // 9) * 13) & 255 for i in range(count))


def benchmark_pixels(mode: str, seed: int) -> bytes:
    count = 64 * 64 * MODES[mode]
    return bytes(
        (i * 73 + (i // 11) * 19 + seed * 47 + (seed >> 2)) & 255
        for i in range(count)
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
    from PIL import Image, ImageFilter

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"unexpected Pillow oracle version: {PIL.__version__}")
    expected: dict[str, list[str]] = {}
    metadata: dict[str, list[int | None]] = {}
    benchmark_outputs: dict[str, list[str]] = {}
    extract_outputs: dict[str, list[str]] = {}
    extract_metadata: dict[str, list[int | None]] = {}
    extract_benchmark_outputs: dict[str, list[str]] = {}
    large_outputs: dict[str, list[str]] = {}
    large_metadata: dict[str, list[int | None]] = {}
    for mode in MODES:
        expected[mode] = []
        metadata[mode] = []
        for size, seed in zip(SIZES, SEEDS, strict=True):
            image = Image.frombytes(mode, size, pixels(mode, size, seed))
            image.info["batch-seed"] = seed
            result = image.filter(ImageFilter.MedianFilter(3))
            expected[mode].append(result.tobytes().hex())
            metadata[mode].append(result.info.get("batch-seed"))
        benchmark_outputs[mode] = []
        for seed in range(64):
            image = Image.frombytes(mode, (64, 64), benchmark_pixels(mode, seed))
            benchmark_outputs[mode].append(
                image.filter(ImageFilter.MedianFilter(3)).tobytes().hex()
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
    for mode, seeds in LARGE_SEEDS.items():
        large_outputs[mode] = []
        large_metadata[mode] = []
        for seed in seeds:
            image = Image.frombytes(mode, LARGE_SIZE, pixels(mode, LARGE_SIZE, seed))
            image.info["batch-seed"] = seed
            result = image.filter(ImageFilter.MedianFilter(3))
            large_outputs[mode].append(result.tobytes().hex())
            large_metadata[mode].append(result.info.get("batch-seed"))
    output.write_text(
        json.dumps(
            {
                "pillow_version": PIL.__version__,
                "outputs": expected,
                "metadata": metadata,
                "benchmark_outputs": benchmark_outputs,
                "extract_outputs": extract_outputs,
                "extract_metadata": extract_metadata,
                "extract_benchmark_outputs": extract_benchmark_outputs,
                "large_outputs": large_outputs,
                "large_metadata": large_metadata,
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
        subprocess.run(
            [sys.executable, str(Path(__file__).resolve())],
            cwd=ROOT,
            env=target_env,
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
    raise SystemExit(main())

#!/usr/bin/env python3
"""Compare explicit native-mode Expand batches with isolated Pillow bytes.

The oracle and replacement PIL namespace run in separate processes. Set
``EXPAND_BATCH_INCLUDE_FAULT_CONTRACT=1`` after building with
``migration-fault-injection`` to run the target-only grouped-failure contracts.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MODES = {"L": 1, "LA": 2, "RGB": 3, "RGBA": 4}
SMALL = ((7, 5, 11), (7, 5, 229))
EAGER = (7, 5, 73)
LARGE_SIZE = (256, 256)
LARGE_IMAGE_COUNT = 16
LARGE_BORDER = 7
XLARGE_SIZE = (1024, 768)
XLARGE_IMAGE_COUNT = 4
FAULT_CONTRACT_REQUIREMENTS = {
    "imagebatch.expand.group-fallback": (
        "A compatible queued native-mode Expand group recovers exact ordered outputs "
        "after a dimension or allocation failure, and the executor remains usable."
    ),
}
FAULT_CONTRACT_CASES = (
    {
        "case_id": "imagebatch.expand.group-dimension-failure.fallback",
        "verification": "fault-contract",
        "operation": "ImageBatch.Expand",
        "target_profile": "python-gpu",
        "oracle": "not_applicable",
        "requirements": ("imagebatch.expand.group-fallback",),
        "fault": {
            "point": "image_batch.expand.group_dimension_failure",
            "contract": "grouped-expand-error-falls-back-and-recovers",
        },
        "mode": "RGBA",
        "input_seeds": (11, 229),
    },
    {
        "case_id": "imagebatch.expand.group-memory-failure.fallback",
        "verification": "fault-contract",
        "operation": "ImageBatch.Expand",
        "target_profile": "python-gpu",
        "oracle": "not_applicable",
        "requirements": ("imagebatch.expand.group-fallback",),
        "fault": {
            "point": "image_batch.expand.group_memory_failure",
            "contract": "grouped-expand-error-falls-back-and-recovers",
        },
        "mode": "RGBA",
        "input_seeds": (11, 229),
    },
)


def fill_for(mode: str):
    return {
        "L": 37,
        "LA": (17, 83),
        "RGB": (11, 23, 37),
        "RGBA": (11, 23, 37, 49),
    }[mode]


def pixels(mode: str, size: tuple[int, int], seed: int) -> bytes:
    count = size[0] * size[1] * MODES[mode]
    if seed in (11, 229):
        return bytes((seed,)) * count
    return bytes((index * 37 + index // 11 * 19 + seed * 47) & 255 for index in range(count))


def material_pixels(
    mode: str,
    seed: int,
    size: tuple[int, int] = LARGE_SIZE,
) -> bytes:
    width, height = size
    count = width * height * MODES[mode]
    return bytes(
        (index * 73 + index // 13 * 31 + seed * 47 + (seed >> 2)) & 255
        for index in range(count)
    )


def image_record(image) -> dict:
    return {
        "bytes": image.tobytes().hex(),
        "mode": image.mode,
        "size": list(image.size),
        "info": image.info.get("batch-seed"),
    }


def run_oracle(output: Path) -> None:
    import PIL
    from PIL import Image, ImageOps

    if PIL.__version__ != "12.2.0":
        raise RuntimeError(f"unexpected Pillow oracle version: {PIL.__version__}")
    expected = {
        "pillow_version": PIL.__version__,
        "small": {},
        "eager": {},
        "large": {},
        "xlarge": {},
    }
    for mode in MODES:
        small = []
        for width, height, seed in SMALL:
            image = Image.frombytes(mode, (width, height), pixels(mode, (width, height), seed))
            image.info["batch-seed"] = seed
            small.append(image_record(ImageOps.expand(image, 2, fill_for(mode))))
        expected["small"][mode] = small

        width, height, seed = EAGER
        image = Image.frombytes(mode, (width, height), pixels(mode, (width, height), seed))
        image.info["batch-seed"] = seed
        expected["eager"][mode] = image_record(ImageOps.expand(image, 2, fill_for(mode)))

        expected["large"][mode] = [
            hashlib.sha256(
                ImageOps.expand(
                    Image.frombytes(mode, LARGE_SIZE, material_pixels(mode, seed)),
                    LARGE_BORDER,
                    fill_for(mode),
                ).tobytes()
            ).hexdigest()
            for seed in range(LARGE_IMAGE_COUNT)
        ]
        expected["xlarge"][mode] = [
            hashlib.sha256(
                ImageOps.expand(
                    Image.frombytes(
                        mode,
                        XLARGE_SIZE,
                        material_pixels(mode, seed, XLARGE_SIZE),
                    ),
                    LARGE_BORDER,
                    fill_for(mode),
                ).tobytes()
            ).hexdigest()
            for seed in range(XLARGE_IMAGE_COUNT)
        ]
    output.write_text(json.dumps(expected))


def require_exact(actual, expected, label: str, check_info: bool = True) -> None:
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


def require_backend(core, backend: str, label: str, expected_expand_dispatches: int) -> None:
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
            raise AssertionError(f"{label} converted native pixels: {receipt}")
        records = core.take_gpu_shader_coverage()
        dispatches = sum(
            record["dispatches"]
            for record in records
            if "expand.wgsl" in record["shader_file"]
        )
        if dispatches != expected_expand_dispatches:
            raise AssertionError(
                f"{label} issued {dispatches} Expand shader dispatches; "
                f"expected {expected_expand_dispatches}: {records}"
            )
    else:
        core.take_gpu_shader_coverage()


def run_target(expected_path: Path) -> None:
    from PIL import Image, ImageBatch
    import pillow_rs._core as core

    expected = json.loads(expected_path.read_text())
    if expected.get("pillow_version") != "12.2.0":
        raise RuntimeError("oracle artifact version mismatch")

    for backend in ("cpu", "simd", "gpu"):
        for selected in ("cpu", "simd", "gpu"):
            core.disable_backend(selected)
        if not core.enable_backend(backend):
            raise RuntimeError(f"{backend} backend unavailable for ImageBatch Expand parity")
        core.set_pipeline_telemetry(True)
        core.set_gpu_shader_coverage(True)

        for mode in MODES:
            small = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for width, height, seed in SMALL:
                image = Image.frombytes(mode, (width, height), pixels(mode, (width, height), seed))
                image.info["batch-seed"] = seed
                small.submit(image, ImageBatch.Expand(2, fill_for(mode)))
            small_result = small.join()
            require_exact(small_result, expected["small"][mode], f"queued {backend}/{mode}")
            require_backend(
                core,
                backend,
                f"queued {backend}/{mode} small pair",
                1 if backend == "gpu" else 0,
            )

            width, height, seed = EAGER
            image = Image.frombytes(mode, (width, height), pixels(mode, (width, height), seed))
            image.info["batch-seed"] = seed
            eager = ImageBatch.BatchExecutor(queue=False, backend=backend)
            eager.submit(image, ImageBatch.Expand(2, fill_for(mode)))
            require_exact(eager.join(), [expected["eager"][mode]], f"eager {backend}/{mode}")
            require_backend(
                core,
                backend,
                f"eager {backend}/{mode}",
                1 if backend == "gpu" else 0,
            )

            large = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for seed in range(LARGE_IMAGE_COUNT):
                large.submit(
                    Image.frombytes(mode, LARGE_SIZE, material_pixels(mode, seed)),
                    ImageBatch.Expand(LARGE_BORDER, fill_for(mode)),
                )
            large_results = large.join()
            digests = [hashlib.sha256(image.tobytes()).hexdigest() for image in large_results]
            if digests != expected["large"][mode]:
                raise AssertionError(f"queued {backend}/{mode} 256x256x16 Expand differs from Pillow")
            if any(
                image.mode != mode or image.size != (270, 270)
                for image in large_results
            ):
                raise AssertionError(f"queued {backend}/{mode} Expand changed native output shape")
            require_backend(
                core,
                backend,
                f"queued {backend}/{mode} large group",
                1 if backend == "gpu" else 0,
            )
            xlarge = ImageBatch.BatchExecutor(queue=True, backend=backend)
            for seed in range(XLARGE_IMAGE_COUNT):
                xlarge.submit(
                    Image.frombytes(
                        mode,
                        XLARGE_SIZE,
                        material_pixels(mode, seed, XLARGE_SIZE),
                    ),
                    ImageBatch.Expand(LARGE_BORDER, fill_for(mode)),
                )
            xlarge_results = xlarge.join()
            xlarge_digests = [
                hashlib.sha256(image.tobytes()).hexdigest()
                for image in xlarge_results
            ]
            if xlarge_digests != expected["xlarge"][mode]:
                raise AssertionError(
                    f"queued {backend}/{mode} 1024x768x4 Expand differs from Pillow"
                )
            if any(
                image.mode != mode or image.size != (1038, 782)
                for image in xlarge_results
            ):
                raise AssertionError(f"queued {backend}/{mode} large Expand changed output shape")
            require_backend(
                core,
                backend,
                f"queued {backend}/{mode} 1024x768 group",
                1 if backend == "gpu" else 0,
            )
            print(
                f"{backend}/{mode} ImageBatch.Expand: Pillow parity PASS; "
                "256x256 × 16 and 1024x768 × 4"
            )

    core.disable_backend("simd")
    core.disable_backend("gpu")
    core.enable_backend("cpu")


def assert_grouped_expand_failure_fallback(case: dict, expected: dict) -> None:
    from PIL import Image, ImageBatch
    import pillow_rs._core as core

    fault_point = case["fault"]["point"]
    if os.environ.get("PILLOW_RS_MIGRATION_FAULT_POINT") != fault_point:
        raise RuntimeError(f"fault point was not selected: {fault_point}")
    for backend in ("cpu", "simd", "gpu"):
        core.disable_backend(backend)
    if not core.enable_backend("gpu"):
        raise RuntimeError("GPU backend unavailable for Expand fallback fault contract")
    core.set_pipeline_telemetry(True)
    core.set_gpu_shader_coverage(True)
    core.take_gpu_shader_coverage()

    batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
    references = {
        seed: reference
        for (_, _, seed), reference in zip(
            SMALL,
            expected["small"][case["mode"]],
            strict=True,
        )
    }
    for seed in case["input_seeds"]:
        width, height, _ = next(item for item in SMALL if item[2] == seed)
        image = Image.frombytes(
            case["mode"], (width, height), pixels(case["mode"], (width, height), seed)
        )
        image.info["batch-seed"] = seed
        batch.submit(image, ImageBatch.Expand(2, fill_for(case["mode"])))

    recovered = batch.join()
    require_exact(
        recovered,
        [references[seed] for seed in case["input_seeds"]],
        "fault-contract grouped Expand fallback",
    )
    records = core.take_gpu_shader_coverage()
    dispatches = sum(
        record["dispatches"]
        for record in records
        if "expand.wgsl" in record["shader_file"]
    )
    if dispatches != len(case["input_seeds"]):
        raise AssertionError(
            "fault injection did not produce one GPU single-image fallback per input: "
            f"{records}"
        )

    width, height, seed = EAGER
    image = Image.frombytes(
        case["mode"], (width, height), pixels(case["mode"], (width, height), seed)
    )
    image.info["batch-seed"] = seed
    if batch.submit(image, ImageBatch.Expand(2, fill_for(case["mode"]))) != 0:
        raise AssertionError("a drained batch did not reset its submission index")
    followup = batch.join()
    require_exact(
        followup,
        [expected["eager"][case["mode"]]],
        "fault-contract follow-up Expand join",
    )
    require_backend(core, "gpu", "fault-contract follow-up Expand join", 1)


def run_fault_contracts(expected_path: Path) -> None:
    expected = json.loads(expected_path.read_text())
    if expected.get("pillow_version") != "12.2.0":
        raise RuntimeError("oracle artifact version mismatch")
    contracts = {"grouped-expand-error-falls-back-and-recovers": assert_grouped_expand_failure_fallback}
    case_id = os.environ.get("EXPAND_FAULT_CONTRACT_CASE_ID")
    case = next((item for item in FAULT_CONTRACT_CASES if item["case_id"] == case_id), None)
    if case is None:
        raise ValueError(f"unknown fault-contract case: {case_id!r}")
    if (
        case["verification"] != "fault-contract"
        or case["operation"] != "ImageBatch.Expand"
        or case["target_profile"] != "python-gpu"
        or case["oracle"] != "not_applicable"
        or not case["requirements"]
        or any(requirement not in FAULT_CONTRACT_REQUIREMENTS for requirement in case["requirements"])
    ):
        raise ValueError(f"invalid fault-contract case declaration: {case_id}")
    assertion = contracts.get(case["fault"]["contract"])
    if assertion is None:
        raise ValueError(f"unknown fault contract: {case['fault']['contract']}")
    assertion(case, expected)
    print(
        f"fault-contract case={case_id} selected=1 executed=1 passed=1 failed=0 "
        f"requirements={','.join(case['requirements'])} oracle=not_applicable"
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="pillow-rs-expand-batch-") as directory:
        expected = Path(directory) / "pillow-expected.json"
        oracle_env = os.environ.copy()
        oracle_env.pop("PYTHONPATH", None)
        oracle_env["EXPAND_BATCH_PARITY_MODE"] = "oracle"
        oracle_env["EXPAND_BATCH_PARITY_OUTPUT"] = str(expected)
        subprocess.run([sys.executable, str(Path(__file__).resolve())], cwd=ROOT, env=oracle_env, check=True)

        target_env = os.environ.copy()
        target_env["PYTHONPATH"] = os.pathsep.join(
            (str(ROOT / "scripts"), str(ROOT / "pillow-rs-py/python"))
        )
        target_env["EXPAND_BATCH_PARITY_MODE"] = "target"
        target_env["EXPAND_BATCH_PARITY_EXPECTED"] = str(expected)
        target_env.pop("PILLOW_RS_MIGRATION_FAULT_POINT", None)
        target_env.pop("EXPAND_FAULT_CONTRACT_CASE_ID", None)
        subprocess.run([sys.executable, str(Path(__file__).resolve())], cwd=ROOT, env=target_env, check=True)

        if os.environ.get("EXPAND_BATCH_INCLUDE_FAULT_CONTRACT") == "1":
            for case in FAULT_CONTRACT_CASES:
                fault_env = target_env.copy()
                fault_env["EXPAND_BATCH_PARITY_MODE"] = "fault-contract"
                fault_env["PILLOW_RS_MIGRATION_FAULT_POINT"] = case["fault"]["point"]
                fault_env["EXPAND_FAULT_CONTRACT_CASE_ID"] = case["case_id"]
                subprocess.run([sys.executable, str(Path(__file__).resolve())], cwd=ROOT, env=fault_env, check=True)
    return 0


if __name__ == "__main__":
    mode = os.environ.get("EXPAND_BATCH_PARITY_MODE")
    if mode == "oracle":
        run_oracle(Path(os.environ["EXPAND_BATCH_PARITY_OUTPUT"]))
        raise SystemExit(0)
    if mode == "target":
        run_target(Path(os.environ["EXPAND_BATCH_PARITY_EXPECTED"]))
        raise SystemExit(0)
    if mode == "fault-contract":
        run_fault_contracts(Path(os.environ["EXPAND_BATCH_PARITY_EXPECTED"]))
        raise SystemExit(0)
    raise SystemExit(main())

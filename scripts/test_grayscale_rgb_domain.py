"""Exhaust RGB-to-L bytes against isolated live Pillow and native backends.

Run after make build-parity. Each fresh 256-square image fixes R and enumerates
every G/B pair, covering all 16,777,216 RGB triples without a giant GPU buffer.
This is a parity diagnostic, not a benchmark or coverage collector.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "pillow-rs-py/python"
SUBJECTS = ("pillow", "cpu", "simd", "gpu")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def worker(subject, directory):
    import PIL
    from PIL import Image, ImageOps
    import run_migration_parity as parity

    target = subject != "pillow"
    if Path(PIL.__file__).resolve().is_relative_to(TARGET) != target:
        raise RuntimeError(f"wrong PIL import for {subject}: {PIL.__file__}")
    identity = parity.side_identity("target" if target else "source")
    core = None
    if target:
        from pillow_rs import _core as core
        core.set_pipeline_telemetry(True)
    inputs = (directory / "inputs.rgb").read_bytes()
    counts = Counter()
    receipts = []
    output = bytearray()
    for red in range(256):
        image = Image.frombytes("RGB", (256, 256), inputs[red * 196608:(red + 1) * 196608])
        if target:
            parity.lock_target_image_pipeline(image)
            core.take_pipeline_telemetry()
        result = ImageOps.grayscale(image)
        if target:
            parity.lock_target_image_pipeline(result)
        pixels = result.tobytes()
        assert result.mode == "L" and result.size == (256, 256)
        assert len(pixels) == 65536
        output.extend(pixels)
        if target:
            receipt = core.take_pipeline_telemetry()
            # Raw telemetry has no status field: successful terminal export
            # above establishes completion, and this receipt proves execution.
            if (receipt.get("requested_backend") != subject
                    or receipt.get("actual_backend") != subject
                    or receipt.get("operation_count") != 1
                    or receipt.get("fallback_reason") is not None):
                raise AssertionError(f"non-native {subject} block {red}: {receipt}")
            if subject == "gpu":
                resource = receipt.get("resource") or {}
                if (receipt.get("dispatch_count") != 1
                        or resource.get("upload_bytes", 0) < 196608
                        or resource.get("readback_bytes", 0) < 65536):
                    raise AssertionError(f"incomplete GPU work for block {red}: {receipt}")
            counts[receipt["actual_backend"]] += 1
            receipts.append(receipt)
    data = bytes(output)
    (directory / f"{subject}.l").write_bytes(data)
    record = {"subject": subject, "identity": identity, "bytes": len(data),
              "sha256": digest(data), "native_counts": dict(counts), "receipts": receipts}
    if core is not None:
        record["binary_sha256"] = digest(Path(core.__file__).read_bytes())
    (directory / f"{subject}.json").write_text(json.dumps(record, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--worker", choices=SUBJECTS, help=argparse.SUPPRESS)
    args = parser.parse_args()
    directory = args.output_dir.resolve()
    if args.worker:
        worker(args.worker, directory)
        return
    directory.mkdir(parents=True, exist_ok=True)
    green = np.repeat(np.arange(256, dtype=np.uint8), 256)
    blue = np.tile(np.arange(256, dtype=np.uint8), 256)
    block = np.empty((65536, 3), dtype=np.uint8)
    block[:, 1], block[:, 2] = green, blue
    with (directory / "inputs.rgb").open("wb") as stream:
        for red in range(256):
            block[:, 0] = red
            stream.write(block.tobytes())
    records = []
    for subject in SUBJECTS:
        env = os.environ.copy()
        paths = [p for p in env.get("PYTHONPATH", "").split(os.pathsep)
                 if p and Path(p).resolve() != TARGET]
        if subject != "pillow":
            paths.insert(0, str(TARGET))
        env.update(PYTHONPATH=os.pathsep.join(paths), PYTHONDONTWRITEBYTECODE="1",
                   MIGRATION_TARGET_BACKEND=subject if subject != "pillow" else "cpu",
                   MIGRATION_STRICT_TARGET_BACKEND="1")
        subprocess.run([sys.executable, str(Path(__file__).resolve()),
                        "--output-dir", str(directory), "--worker", subject],
                       env=env, cwd=ROOT, check=True, timeout=240)
        actual = (directory / f"{subject}.l").read_bytes()
        reference = (directory / "pillow.l").read_bytes()
        differences = sum(a != b for a, b in zip(actual, reference)) if actual != reference else 0
        record = {"subject": subject, "bytes": len(actual), "different_bytes": differences,
                  "exact": actual == reference}
        records.append(record)
        print(json.dumps(record), flush=True)
    # Independently check the proposed narrower arithmetic against the live
    # oracle. It is never used to generate backend expected-output fixtures.
    g, b = green.astype(np.int32), blue.astype(np.int32)
    reference = (directory / "pillow.l").read_bytes()
    formula_differences = 0
    for red in range(256):
        base = 77 * red + 150 * g + 29 * b
        correction = 70 * g + 47 * b - 117 * red
        candidate = ((base + ((correction + 32768) >> 8)) >> 8).astype(np.uint8)
        formula_differences += int(np.count_nonzero(candidate != np.frombuffer(
            reference[red * 65536:(red + 1) * 65536], dtype=np.uint8)))
    report = {"schema": "pillow-rs/grayscale-rgb-domain-parity@1",
              "rgb_triples": 16777216, "subjects": records,
              "u16_formula_different_bytes": formula_differences}
    (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    if formula_differences or any(not item["exact"] for item in records):
        raise AssertionError(report)


if __name__ == "__main__":
    main()

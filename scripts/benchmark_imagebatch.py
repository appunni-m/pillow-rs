#!/usr/bin/env python3
"""Measure full-call throughput for the explicit ImageBatch API.

Run after ``make build-parity`` so the checkout facade is used while Pillow
remains installed as the oracle. For exact Pillow comparison and grouped-
dispatch proof, run ``scripts/test_imagebatch_parity.py`` separately.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import statistics
import sys
import time


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", choices=("cpu", "simd", "gpu"), default="gpu")
    parser.add_argument("--queue", action="store_true", help="queue operations until join")
    parser.add_argument("--mode", choices=("L", "LA", "RGB", "RGBA"), default="L")
    parser.add_argument("--width", type=int, default=64)
    parser.add_argument("--height", type=int, default=64)
    parser.add_argument("--images", type=int, default=64)
    parser.add_argument("--samples", type=int, default=12)
    parser.add_argument("--warmups", type=int, default=3)
    args = parser.parse_args()
    if min(args.width, args.height, args.images, args.samples) < 1 or args.warmups < 0:
        parser.error("dimensions, image count, and samples must be positive; warmups cannot be negative")
    return args


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parents[1]
    sys.path.insert(0, str(root / "pillow-rs-py" / "python"))
    sys.path.insert(0, str(root / "scripts"))
    from PIL import Image, ImageBatch, ImageFilter
    import pillow_rs._core as core

    channels = {"L": 1, "LA": 2, "RGB": 3, "RGBA": 4}[args.mode]
    frame_bytes = args.width * args.height * channels
    window_count = args.warmups + args.samples + 1
    inputs = [
        bytes(
            (index * 73 + (index // 11) * 19 + seed * 47 + (seed >> 2)) & 255
            for index in range(frame_bytes)
        )
        for seed in range(args.images * window_count)
    ]

    for backend in ("cpu", "simd", "gpu"):
        core.disable_backend(backend)
    if not core.enable_backend(args.backend):
        raise RuntimeError(f"{args.backend} backend is unavailable")

    def run_window(start: int) -> list[Image.Image]:
        executor = ImageBatch.BatchExecutor(queue=args.queue, backend=args.backend)
        for image_index in range(args.images):
            image = Image.frombytes(
                args.mode,
                (args.width, args.height),
                inputs[start + image_index],
            )
            executor.submit(image, ImageFilter.MedianFilter(3))
        result = executor.join()
        if len(result) != args.images:
            raise RuntimeError(f"expected {args.images} outputs, received {len(result)}")
        return result

    core.set_pipeline_telemetry(True)
    run_window(0)
    receipt = core.take_pipeline_telemetry()
    core.set_pipeline_telemetry(False)
    if (
        receipt is None
        or receipt.get("actual_backend") != args.backend
        or receipt.get("fallback_reason")
    ):
        raise RuntimeError(f"requested {args.backend}, but preflight routed differently: {receipt}")

    for warmup in range(args.warmups):
        run_window((warmup + 1) * args.images)

    sample_ns = []
    for sample in range(args.samples):
        start_index = (args.warmups + sample + 1) * args.images
        start = time.perf_counter_ns()
        run_window(start_index)
        sample_ns.append(time.perf_counter_ns() - start)

    median_ns = statistics.median(sample_ns)
    print(
        json.dumps(
            {
                "mode": args.mode,
                "size": [args.width, args.height],
                "images_per_window": args.images,
                "backend": args.backend,
                "queue": args.queue,
                "samples": args.samples,
                "warmups": args.warmups,
                "median_window_ms": median_ns / 1_000_000,
                "images_per_second": args.images * 1_000_000_000 / median_ns,
                "pixels_per_second": (
                    args.images * args.width * args.height * 1_000_000_000 / median_ns
                ),
                "preflight": {
                    "actual_backend": receipt["actual_backend"],
                    "operation_count": receipt["operation_count"],
                    "dispatch_count": receipt["dispatch_count"],
                    "fallback_reason": receipt["fallback_reason"],
                },
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

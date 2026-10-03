#!/usr/bin/env python3
"""Measure full-call throughput for the explicit ImageBatch API.

Run target profiles after ``make build-parity``. ``--backend pillow`` runs an
ordinary sequential Pillow baseline without adding the checkout facade to
``sys.path``. For exact Pillow comparison and grouped-dispatch proof, run
``scripts/test_imagebatch_parity.py`` separately.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import statistics
import sys
import time


def make_color3dlut(ImageFilter):
    def transform(red, green, blue):
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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", choices=("pillow", "cpu", "simd", "gpu"), default="gpu")
    parser.add_argument("--queue", action="store_true", help="queue operations until join")
    parser.add_argument(
        "--operation",
        choices=("median-filter", "extract-band", "multiply", "color3dlut"),
        default="median-filter",
    )
    parser.add_argument("--mode", choices=("L", "LA", "RGB", "RGBA"), default="L")
    parser.add_argument("--channel", type=int, default=0)
    parser.add_argument("--width", type=int, default=64)
    parser.add_argument("--height", type=int, default=64)
    parser.add_argument("--images", type=int, default=64)
    parser.add_argument("--samples", type=int, default=12)
    parser.add_argument("--warmups", type=int, default=3)
    args = parser.parse_args()
    if min(args.width, args.height, args.images, args.samples) < 1 or args.warmups < 0:
        parser.error("dimensions, image count, and samples must be positive; warmups cannot be negative")
    if args.operation == "extract-band" and not 0 <= args.channel < {
        "L": 1,
        "LA": 2,
        "RGB": 3,
        "RGBA": 4,
    }[args.mode]:
        parser.error("channel is outside the selected image mode")
    if args.operation == "color3dlut" and args.mode != "RGBA":
        parser.error("the explicit Color3DLUT batch currently requires mode RGBA")
    if args.backend == "pillow" and args.queue:
        parser.error("Pillow baseline is ordinary sequential execution; omit --queue")
    return args


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parents[1]
    if args.backend != "pillow":
        sys.path.insert(0, str(root / "pillow-rs-py" / "python"))
        sys.path.insert(0, str(root / "scripts"))
    from PIL import Image, ImageFilter
    if args.backend == "pillow":
        import PIL
        from PIL import ImageChops

        if PIL.__version__ != "12.2.0":
            raise RuntimeError(f"unexpected Pillow baseline version: {PIL.__version__}")
        pillow_version = PIL.__version__
        core = None
        color_lut = make_color3dlut(ImageFilter) if args.operation == "color3dlut" else None
        batch_color_lut = None
    else:
        from PIL import ImageBatch
        import pillow_rs._core as core

        pillow_version = None
        color_lut = make_color3dlut(ImageFilter) if args.operation == "color3dlut" else None
        batch_color_lut = ImageBatch.Color3DLUT(color_lut) if color_lut is not None else None

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
    other_inputs = (
        [
            bytes(
                (
                    index * 73
                    + (index // 11) * 19
                    + (seed + 137) * 47
                    + ((seed + 137) >> 2)
                )
                & 255
                for index in range(frame_bytes)
            )
            for seed in range(args.images * window_count)
        ]
        if args.operation == "multiply"
        else []
    )

    if core is not None:
        for backend in ("cpu", "simd", "gpu"):
            core.disable_backend(backend)
        if not core.enable_backend(args.backend):
            raise RuntimeError(f"{args.backend} backend is unavailable")

    def run_window(start: int) -> list[Image.Image]:
        if args.backend == "pillow":
            for image_index in range(args.images):
                image = Image.frombytes(
                    args.mode,
                    (args.width, args.height),
                    inputs[start + image_index],
                )
                if args.operation == "median-filter":
                    image.filter(ImageFilter.MedianFilter(3)).tobytes()
                elif args.operation == "extract-band":
                    image.getchannel(args.channel).tobytes()
                elif args.operation == "color3dlut":
                    image.filter(color_lut).tobytes()
                else:
                    other = Image.frombytes(
                        args.mode,
                        (args.width, args.height),
                        other_inputs[start + image_index],
                    )
                    ImageChops.multiply(image, other).tobytes()
            return []

        executor = ImageBatch.BatchExecutor(queue=args.queue, backend=args.backend)
        for image_index in range(args.images):
            image = Image.frombytes(
                args.mode,
                (args.width, args.height),
                inputs[start + image_index],
            )
            operation = (
                ImageFilter.MedianFilter(3)
                if args.operation == "median-filter"
                else (
                    ImageBatch.ExtractBand(args.channel)
                    if args.operation == "extract-band"
                    else (
                        batch_color_lut
                        if args.operation == "color3dlut"
                        else ImageBatch.Multiply(
                            Image.frombytes(
                                args.mode,
                                (args.width, args.height),
                                other_inputs[start + image_index],
                            )
                        )
                    )
                )
            )
            executor.submit(image, operation)
        result = executor.join()
        if len(result) != args.images:
            raise RuntimeError(f"expected {args.images} outputs, received {len(result)}")
        # Match Pillow's materialized byte result inside the timing window for
        # every operation, including LUT and filter output images.
        for image in result:
            image.tobytes()
        return result

    if args.backend == "pillow":
        run_window(0)
        receipt = {
            "actual_backend": "pillow",
            "operation_count": args.images,
            "dispatch_count": 0,
            "fallback_reason": None,
        }
    else:
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
                "operation": args.operation,
                "channel": args.channel if args.operation == "extract-band" else None,
                "size": [args.width, args.height],
                "images_per_window": args.images,
                "backend": args.backend,
                "pillow_version": pillow_version,
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

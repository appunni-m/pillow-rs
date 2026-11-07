#!/usr/bin/env python3
"""Live isolated Pillow comparisons for queued and eager GPU streams.

Inputs below contain no expected bytes/hashes. The oracle runs in its own
process, and only terminal images are compared. No evidence files are emitted.
Run after make build-parity: .venv/bin/python scripts/test_gpu_batch.py
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CHANNELS = {"L": 1, "LA": 2, "RGB": 3, "RGBA": 4, "CMYK": 4, "RGBX": 4}
GRAPH_CHANNELS = {**CHANNELS, "F": 4}


def inputs(large=True):
    for mode, channels in CHANNELS.items():
        for size in [(1, 1), (3, 2), (19, 17), (65, 33)]:
            for band in range(channels):
                yield mode, size, ("channel", band), ("invert",), ("median", 3)
        if mode in ("L", "LA", "RGB", "RGBA"):
            for operation in [("median", 3), ("min", 3), ("max", 3), ("box", 1), ("gaussian", 1.7)]:
                yield mode, (19, 17), operation
            for method in range(7):
                yield mode, (19, 17), ("transpose", method)
            yield mode, (19, 17), ("crop", (3, 2, 21, 19))
            yield mode, (19, 17), ("flip",), ("mirror",)
            yield mode, (19, 17), ("expand", 2)
            for size in ((1, 9), (4, 3), (65, 33)):
                yield mode, size, ("gaussian", 0.0)
                yield mode, size, ("gaussian", 3.25)
            if mode == "RGBA":
                yield mode, (19, 17), ("point",)
                yield mode, (19, 17), ("median", 3), ("putpixel", (3, 2), (37, 83, 119, 171))
                for size in ((1, 1), (4, 3), (27, 13)):
                    yield mode, (19, 17), ("resize", size), ("channel", 3), ("invert",)
                    yield mode, (19, 17), ("resize", size)
            for operation in ("multiply", "screen", "overlay", "hard_light", "soft_light", "difference", "darker", "lighter", "add_modulo", "subtract_modulo", "blend"):
                yield mode, (19, 17), (operation,)
            for offset in ((-2, 1), (3, -4), (100, 100)):
                yield mode, (19, 17), ("paste", offset)
        for method in (0, 2, 6):
            yield mode, (19, 17), ("transpose", method)
        yield mode, (19, 17), ("grayscale",), ("invert",)
        if mode in ("L", "RGB"):
            yield mode, (19, 17), ("invert",), ("solarize", 117), ("posterize", 4)
            yield mode, (19, 17), ("point",)
        if mode == "RGB":
            yield mode, (19, 17), ("grayscale",), ("invert",), ("median", 3)
            yield mode, (19, 17), ("convert", "RGBA"), ("channel", 2), ("invert",)
    if large:
        for mode in ("LA", "RGBA"):
            yield mode, (4096, 4096), ("channel", 1 if mode == "LA" else 3)
    # F graphs are admitted only for the statically proven ordered-f64 resize
    # geometry; keep them out of the generic native-byte operation sweep above.
    yield "F", (19, 17), ("resize", (13, 11), "bicubic")
    yield "F", (19, 17), ("resize", (15, 13), "lanczos")
    # This geometry needs more than the generic per-stage arena reservation
    # and exercises input-independent ordered-f64 resource sizing at admission.
    yield "F", (2048, 1536), ("resize", (1024, 768), "bicubic")


def graph(case):
    from PIL import Image, ImageChops, ImageOps, ImageFilter
    mode, size, *ops = case
    length = size[0] * size[1] * GRAPH_CHANNELS[mode]
    seed = bytes((i * 47 + 23) % 256 for i in range(256))
    im = Image.frombytes(mode, size, seed * (length // 256) + seed[:length % 256])
    for name, *args in ops:
        if name == "channel": im = im.getchannel(args[0])
        elif name == "invert": im = ImageOps.invert(im)
        elif name == "median": im = im.filter(ImageFilter.MedianFilter(args[0]))
        elif name == "min": im = im.filter(ImageFilter.MinFilter(args[0]))
        elif name == "max": im = im.filter(ImageFilter.MaxFilter(args[0]))
        elif name == "box": im = im.filter(ImageFilter.BoxBlur(args[0]))
        elif name == "gaussian": im = im.filter(ImageFilter.GaussianBlur(args[0]))
        elif name == "transpose": im = im.transpose(args[0])
        elif name == "resize":
            resampling = getattr(Image.Resampling, args[1].upper()) if len(args) > 1 else Image.Resampling.NEAREST
            im = im.resize(args[0], resampling)
        elif name == "crop": im = im.crop(args[0])
        elif name == "flip": im = ImageOps.flip(im)
        elif name == "mirror": im = ImageOps.mirror(im)
        elif name == "expand": im = ImageOps.expand(im, args[0], fill=0)
        elif name == "solarize": im = ImageOps.solarize(im, args[0])
        elif name == "posterize": im = ImageOps.posterize(im, args[0])
        elif name == "point": im = im.point(lambda x: (x * 3 + 5) % 256)
        elif name == "putpixel": im.putpixel(*args)
        elif name == "grayscale": im = ImageOps.grayscale(im)
        elif name == "convert": im = im.convert(args[0])
        elif name == "paste":
            other = Image.frombytes(mode, (7, 5), bytes([37]) * (7 * 5 * GRAPH_CHANNELS[mode]))
            other = ImageChops.invert(other)  # auxiliary remains a lazy graph on target
            im.paste(other, args[0])
        else:
            other = ImageChops.invert(im)
            im = Image.blend(im, other, 0.375) if name == "blend" else getattr(ImageChops, name)(im, other)
    return im


def observation(image):
    return [image.mode, list(image.size), hashlib.sha256(image.tobytes()).hexdigest()]


def run_side(side, *, large, output="cpu", eager=False):
    cases = inputs(large)
    if side == "oracle":
        return {str(i): observation(graph(case)) for i, case in enumerate(cases)}
    from PIL import GpuBatchExecutor
    with GpuBatchExecutor(queue=not eager, max_jobs=16, max_in_flight=2) as executor:
        results = {}
        for result in executor.run(((i, graph(case)) for i, case in enumerate(cases)), output=output):
            image = result.image.download() if output == "gpu" else result.image
            results[str(result.input_key)] = observation(image)
            del result, image
        stats = executor.stats
        assert stats["live_jobs"] == stats["gpu_bytes"] == stats["host_bytes"] == 0, stats
        assert stats["dispatches"] >= len(results), stats
        assert stats["submitted_jobs"] == len(results), stats
        assert stats["largest_submission"] == 1 if eager else stats["largest_submission"] > 1, stats
        if output == "gpu": assert stats["readback_bytes"] == 0, stats
        print(f"{output} {'eager' if eager else 'queued'} GPU counters: {stats}", file=sys.stderr)
        return results


def isolated(side, *, large=True, output="cpu", eager=False):
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    if side == "target": env["PYTHONPATH"] = str(ROOT / "pillow-rs-py/python")
    command = [sys.executable, str(Path(__file__).resolve()), "--side", side, "--output", output]
    if not large: command.append("--small")
    if eager: command.append("--eager")
    process = subprocess.run(command, env=env, capture_output=True, text=True)
    if process.returncode: raise RuntimeError(process.stderr)
    if process.stderr: print(process.stderr, end="", file=sys.stderr)
    return json.loads(process.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--side", choices=("oracle", "target"))
    parser.add_argument("--output", choices=("cpu", "gpu"), default="cpu")
    parser.add_argument("--small", action="store_true", help="omit 4096x4096 cases explicitly")
    parser.add_argument("--eager", action="store_true")
    args = parser.parse_args()
    if args.side:
        print(json.dumps(run_side(args.side, large=not args.small, output=args.output, eager=args.eager)))
        return
    oracle = isolated("oracle", large=not args.small)
    target = isolated("target", large=not args.small, output=args.output, eager=args.eager)
    descriptions = list(inputs(not args.small))
    failures = [(key, descriptions[int(key)], expected, target.get(key)) for key, expected in oracle.items() if target.get(key) != expected]
    for failure in failures: print(failure)
    if set(oracle) != set(target) or failures: raise AssertionError(f"{len(failures)} parity differences")
    print(f"Live Pillow parity: {len(oracle)}/{len(oracle)} exact mode/size/pixel matches ({args.output}, {'eager' if args.eager else 'queued'}).")


if __name__ == "__main__": main()

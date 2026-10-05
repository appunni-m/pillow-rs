#!/usr/bin/env python3
"""Completed-work GPU batching diagnostic, isolated live Pillow baseline.

Fresh inputs/graphs every request; warm shader caches, no result cache reuse.
Includes construction, upload, dispatch, readback, Image and terminal byte export.
Reference construction, timing aggregation and exact byte comparisons are outside
measured windows. No host thread pools or JSON evidence files are created.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SUBJECTS = ("Pillow", "GPU eager", "GPU queued")


def child(subject, size, mode, images, samples, max_jobs=16, max_in_flight=2):
    from PIL import Image, ImageOps, ImageFilter
    channels = {"L": 1, "RGB": 3}[mode]
    length = size * size * channels
    tile = bytes((i * 47 + 23) % 256 for i in range(256))
    sources = [tile * (length // 256) + tile[:length % 256],
               tile[::-1] * (length // 256) + tile[::-1][:length % 256]]
    def graph(index):
        return ImageOps.invert(Image.frombytes(mode, (size, size), sources[index % 2])).filter(ImageFilter.MedianFilter(3))
    executor = None
    if subject != "Pillow":
        from PIL import GpuBatchExecutor
        executor = GpuBatchExecutor(queue=subject == "GPU queued", max_jobs=max_jobs, max_in_flight=max_in_flight)
    def window():
        if executor is None:
            return [graph(i).tobytes() for i in range(images)]
        output = {}
        for result in executor.run((i, graph(i)) for i in range(images)):
            output[result.input_key] = result.image.tobytes()
        return [output[i] for i in range(images)]
    times, digest = [], None
    try:
        for _ in range(2): window()
        for _ in range(samples):
            started = time.perf_counter_ns()
            output = window()
            elapsed = time.perf_counter_ns() - started
            actual = [hashlib.sha256(image).hexdigest() for image in output]
            if digest is not None: assert actual == digest, "fresh request outputs changed"
            digest = actual
            times.append(elapsed / 1e9)
        stats = executor.stats if executor else None
        if stats:
            assert stats["gpu_bytes"] == stats["host_bytes"] == stats["live_jobs"] == 0, stats
            assert stats["submitted_jobs"] == (samples + 2) * images, stats
            assert stats["dispatches"] == 2 * stats["submitted_jobs"], stats
        return {"seconds": statistics.median(times), "digests": digest, "stats": stats}
    finally:
        if executor: executor.close()


def isolated(subject, size, mode, images, samples, max_jobs=16, max_in_flight=2):
    env = os.environ.copy()
    env.pop("PYTHONPATH", None)
    if subject != "Pillow": env["PYTHONPATH"] = str(ROOT / "pillow-rs-py/python")
    process = subprocess.run([sys.executable, str(Path(__file__).resolve()), "--subject", subject,
        "--size", str(size), "--mode", mode, "--images", str(images), "--samples", str(samples), "--max-jobs", str(max_jobs), "--max-in-flight", str(max_in_flight)],
        env=env, capture_output=True, text=True)
    if process.returncode: raise RuntimeError(process.stderr)
    return json.loads(process.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--subject", choices=SUBJECTS)
    parser.add_argument("--size", type=int, default=256)
    parser.add_argument("--mode", choices=("L", "RGB"), default="L")
    parser.add_argument("--images", type=int, default=32)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--max-jobs", type=int, default=16)
    parser.add_argument("--max-in-flight", type=int, default=2)
    args = parser.parse_args()
    if args.images < 1 or args.samples < 1: parser.error("images/samples must be positive")
    if args.subject:
        print(json.dumps(child(args.subject, args.size, args.mode, args.images, args.samples, args.max_jobs, args.max_in_flight)))
        return
    print("Warm completed-work diagnostic: invert → median(3); two warmup windows; "
          f"{args.samples} median samples; no host threads. GPU batch submission is not proof of concurrent kernels.")
    print("| Mode/size | Pillow images/s | Eager GPU images/s | Queued GPU images/s | Queued / Pillow | Queued / eager | Max jobs/submission |")
    print("| --- | ---: | ---: | ---: | ---: | ---: | ---: |")
    for mode in ("L", "RGB"):
        for size in (256, 1024):
            runs = [isolated(subject, size, mode, args.images, args.samples, args.max_jobs, args.max_in_flight) for subject in SUBJECTS]
            assert all(run["digests"] == runs[0]["digests"] for run in runs), f"exact parity failed: {mode} {size}"
            rates = [args.images / run["seconds"] for run in runs]
            print(f"| {mode} {size}² | {rates[0]:.1f} | {rates[1]:.1f} | {rates[2]:.1f} | "
                  f"{rates[2]/rates[0]:.2f}× | {rates[2]/rates[1]:.2f}× | {runs[2]['stats']['largest_submission']} |", flush=True)


if __name__ == "__main__": main()

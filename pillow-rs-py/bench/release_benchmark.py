#!/usr/bin/env python3
"""Release-only PyO3 boundary benchmark; not a test or parity runner."""

from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
import os
from statistics import median
import time

from pillow_rs import Image, ImageFilter, ImageOps


SAMPLES = max(3, int(os.environ.get("PILLOW_RS_BENCH_SAMPLES", "5")))
SIZE = int(os.environ.get("PILLOW_RS_BENCH_SIZE", "384"))
WORKERS = max(2, int(os.environ.get("PILLOW_RS_BENCH_WORKERS", "2")))


def summarize(samples):
    return {
        "sample_count": len(samples),
        "min_ns": min(samples),
        "median_ns": median(samples),
        "max_ns": max(samples),
    }


def digest(value):
    return hashlib.blake2b(bytes(value), digest_size=8).hexdigest()


def make_pipeline():
    image = Image.new("L", (SIZE, SIZE), 127)
    return ImageOps.invert(image.filter(ImageFilter.GaussianBlur(3)))


def terminal_bytes():
    return make_pipeline().tobytes()


def terminal_encode():
    output = io.BytesIO()
    make_pipeline().save(output, format="PNG")
    return output.getvalue()


def elapsed(function):
    started = time.perf_counter_ns()
    value = function()
    return time.perf_counter_ns() - started, value


def serial_batch(function):
    started = time.perf_counter_ns()
    outputs = [function() for _ in range(WORKERS)]
    return time.perf_counter_ns() - started, outputs


def concurrent_batch(function):
    started = time.perf_counter_ns()
    with ThreadPoolExecutor(max_workers=WORKERS) as executor:
        outputs = list(executor.map(lambda _: function(), range(WORKERS)))
    return time.perf_counter_ns() - started, outputs


def run_workload(name, function):
    samples = []
    output = None
    for iteration in range(SAMPLES + 1):
        duration, output = elapsed(function)
        if iteration:
            samples.append(duration)
    return {
        "workload": name,
        "timing": summarize(samples),
        "output_digest": digest(output),
    }


def run_overlap_workload(name, function):
    serial_samples = []
    concurrent_samples = []
    serial_digest = concurrent_digest = None
    for iteration in range(SAMPLES + 1):
        serial_duration, serial_outputs = serial_batch(function)
        concurrent_duration, concurrent_outputs = concurrent_batch(function)
        if iteration:
            serial_samples.append(serial_duration)
            concurrent_samples.append(concurrent_duration)
            serial_digest = digest(serial_outputs[0])
            concurrent_digest = digest(concurrent_outputs[0])
    serial_median = median(serial_samples)
    concurrent_median = median(concurrent_samples)
    return {
        "workload": name,
        "workers": WORKERS,
        "serial": summarize(serial_samples),
        "concurrent": summarize(concurrent_samples),
        "concurrent_speedup": serial_median / concurrent_median,
        "serial_output_digest": serial_digest,
        "concurrent_output_digest": concurrent_digest,
    }


def main():
    print(json.dumps({
        "schema": "pillow-rs-py/release-binding-benchmark@1",
        "status": "completed",
        "boundary": {
            "build": "make build",
            "parity": False,
            "unit_tests": False,
            "gpu": False,
            "size": [SIZE, SIZE],
        },
        "workloads": [
            run_workload("terminal_tobytes", terminal_bytes),
            run_overlap_workload("terminal_tobytes_gil_overlap", terminal_bytes),
            run_overlap_workload("terminal_encode_gil_overlap", terminal_encode),
        ],
    }, sort_keys=True))


if __name__ == "__main__":
    main()

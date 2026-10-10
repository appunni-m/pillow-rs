#!/usr/bin/env python3
"""Live isolated Pillow comparisons and target-only GPU stream contracts.

Inputs below contain no expected bytes/hashes. The oracle runs in its own
process, and only terminal images are compared. No evidence files are emitted.
Run after make build-parity: .venv/bin/python scripts/test_gpu_batch.py
"""
from __future__ import annotations

import argparse
import gc
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

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


class StreamContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from PIL import GpuBatchExecutor, Image, ImageOps, BatchError, QueueFull, ResultBackpressure
        cls.Executor, cls.Image, cls.ImageOps = GpuBatchExecutor, Image, ImageOps
        cls.BatchError, cls.QueueFull, cls.Backpressure = BatchError, QueueFull, ResultBackpressure

    def pipeline(self, value=42):
        return self.ImageOps.invert(self.Image.new("L", (19, 17), value))

    def test_manual_ids_full_queue_and_reuse(self):
        with self.Executor(max_jobs=2) as executor:
            ids = [executor.submit(self.pipeline(v)) for v in (10, 20)]
            with self.assertRaises(self.QueueFull) as rejected:
                executor.submit(self.pipeline())
            self.assertEqual(rejected.exception.job_id, 2)
            outputs = {r.job_id: r.image.tobytes() for r in executor.join()}
            self.assertEqual(outputs, {ids[0]: bytes([245]) * 323, ids[1]: bytes([235]) * 323})
            self.assertEqual(executor.submit(self.pipeline()), 3)
            self.assertEqual(len(list(executor.join())), 1)
            self.assertEqual(executor.stats["gpu_bytes"], 0)

    def test_eager_submit_waits_for_its_own_job(self):
        with self.Executor(queue=False, max_jobs=3, max_in_flight=1) as executor:
            ids = []
            for index in range(3):
                ids.append(executor.submit(self.pipeline(index)))
                # Earlier undelivered results must not make a later submit
                # return while its own work is still pending or executing.
                self.assertEqual(executor.stats["in_flight"], 0)
                self.assertEqual(executor.stats["submitted_jobs"], index + 1)
            outputs = {r.job_id: r.image.tobytes() for r in executor.join()}
            self.assertEqual(outputs, {ids[i]: bytes([255 - i]) * 323 for i in range(3)})
            self.assertEqual(executor.stats["largest_submission"], 1)

    def test_lazy_long_stream_duplicate_keys_and_bounded_state(self):
        pulled = []
        def jobs():
            for i in range(257):
                pulled.append(i)
                yield "duplicate", self.pipeline(i % 256)
        with self.Executor(max_jobs=7, max_in_flight=2) as executor:
            results = executor.run(jobs())
            self.assertEqual(pulled, [])
            identities = set()
            for result in results:
                self.assertLessEqual(len(pulled) - len(identities), 8)
                self.assertEqual(result.input_key, "duplicate")
                self.assertEqual(result.image.tobytes(), bytes([255 - result.job_id % 256]) * 323)
                self.assertNotIn(result.job_id, identities)
                identities.add(result.job_id)
                stats = executor.stats
                self.assertLessEqual(stats["live_jobs"], 7)
                self.assertLessEqual(stats["in_flight"], 2)
                self.assertLessEqual(stats["gpu_bytes"], 512 << 20)
                self.assertLessEqual(stats["host_bytes"], 256 << 20)
            self.assertEqual(len(identities), 257)
            self.assertGreater(executor.stats["largest_submission"], 1)

    def test_reject_unknown_path_before_submission_and_stop_producer(self):
        pulled = []
        def jobs():
            pulled.append(0)
            yield "unsupported", self.Image.new("P", (3, 2)).transpose(0)
            pulled.append(1)
            yield "later", self.pipeline()
        with self.Executor() as executor:
            with self.assertRaises(self.BatchError) as rejected:
                next(executor.run(jobs()))
            self.assertEqual(rejected.exception.job_id, 0)
            self.assertEqual(rejected.exception.input_key, "unsupported")
            self.assertEqual(pulled, [0])
            self.assertEqual(executor.stats["submitted_jobs"], 0)
            self.assertEqual(executor.stats["live_jobs"], 0)

    def test_abandoned_iterator_cancels_pending_without_image_dispatch(self):
        executor = self.Executor(max_jobs=3)
        results = executor.run((i, self.pipeline()) for i in range(10))
        del results
        gc.collect()
        self.assertEqual(executor.stats["submitted_jobs"], 0)
        with self.assertRaises(self.BatchError): executor.submit(self.pipeline())

    def test_active_driver_and_close_stop_input(self):
        pulled = []
        cleanup = []
        def jobs():
            try:
                for i in range(20):
                    pulled.append(i)
                    yield i, self.pipeline()
            finally:
                cleanup.append(True)
        producer = jobs()
        with self.Executor(max_jobs=3) as executor:
            results = executor.run(producer, output="gpu")
            first = next(results)
            with self.assertRaises(self.BatchError): executor.submit(self.pipeline())
            count = len(pulled)
            results.close()
            self.assertEqual(len(pulled), count)
            self.assertEqual(cleanup, [])  # caller still owns this generator
            producer.close()
            self.assertEqual(cleanup, [True])
            self.assertEqual(executor.stats["live_jobs"], 0)
            self.assertEqual(first.image.download().tobytes(), bytes([213]) * 323)
            del first
            gc.collect()
            self.assertEqual(executor.stats["gpu_bytes"], 0)

    def test_resident_leases_backpressure_retry_and_manual_cpu_reset(self):
        # Measure this small native graph's conservative admission reservation,
        # then use a cap that fits exactly one graph plus held terminal leases.
        with self.Executor(max_jobs=1) as probe:
            probe.submit(self.pipeline())
            gpu_cap = probe.stats["gpu_bytes"]
            list(probe.join())
        with self.Executor(gpu_bytes=gpu_cap, max_jobs=1) as executor:
            results = executor.run(((i, self.pipeline()) for i in range(3)), output="gpu")
            first = next(results)
            self.assertGreater(executor.stats["gpu_bytes"], 0)
            first.image.download()
            held = executor.stats["gpu_bytes"]
            with self.assertRaises(self.Backpressure): next(results)
            self.assertEqual(executor.stats["gpu_bytes"], held)
            del first
            gc.collect()
            second = next(results)
            self.assertEqual(second.input_key, 1)
            del second
            gc.collect()
            third = next(results)
            self.assertEqual(third.input_key, 2)
            del third
            gc.collect()
            with self.assertRaises(StopIteration): next(results)
            executor.submit(self.pipeline(12))
            result = next(executor.join())
            self.assertIsInstance(result.image, self.Image.Image)  # PIL.Image is a module
            self.assertEqual(result.image.tobytes(), bytes([243]) * 323)

    def test_download_metadata_is_independent(self):
        im = self.pipeline()
        im.info["tag"] = b"value"
        im.info["nested"] = {"items": [17]}
        with self.Executor(max_jobs=1) as executor:
            results = executor.run([(0, im)], output="gpu")
            resident = next(results).image
            charged = executor.stats["host_bytes"]
            im.info["nested"]["items"].append(bytes(100_000))
            self.assertEqual(executor.stats["host_bytes"], charged)
            first = resident.download()
            first.info["tag"] = b"changed"
            self.assertEqual(first.info["nested"], {"items": [17]})
            first.info["nested"]["items"].append(42)
            self.assertEqual(resident.download().info["tag"], b"value")
            self.assertEqual(resident.download().info["nested"], {"items": [17]})
            del resident
            list(results)

    def test_hard_limits_and_bad_keys(self):
        with self.Executor(gpu_bytes=1, host_bytes=1) as executor:
            with self.assertRaises(self.BatchError) as rejected: executor.submit(self.pipeline())
            self.assertEqual(rejected.exception.kind, "Limit")
            self.assertEqual(executor.stats["live_jobs"], 0)
            self.assertEqual(executor.stats["gpu_bytes"], 0)
        for key in (True, -1, object()):
            with self.Executor() as executor:
                with self.assertRaises(TypeError): next(executor.run([(key, self.pipeline())]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--side", choices=("oracle", "target"))
    parser.add_argument("--output", choices=("cpu", "gpu"), default="cpu")
    parser.add_argument("--small", action="store_true", help="omit 4096x4096 cases explicitly")
    parser.add_argument("--eager", action="store_true")
    parser.add_argument("--contracts", action="store_true")
    args = parser.parse_args()
    if args.side:
        print(json.dumps(run_side(args.side, large=not args.small, output=args.output, eager=args.eager)))
        return
    if args.contracts:
        sys.path.insert(0, str(ROOT / "pillow-rs-py/python"))
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(StreamContracts)
        if not unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful(): sys.exit(1)
        return
    oracle = isolated("oracle", large=not args.small)
    target = isolated("target", large=not args.small, output=args.output, eager=args.eager)
    descriptions = list(inputs(not args.small))
    failures = [(key, descriptions[int(key)], expected, target.get(key)) for key, expected in oracle.items() if target.get(key) != expected]
    for failure in failures: print(failure)
    if set(oracle) != set(target) or failures: raise AssertionError(f"{len(failures)} parity differences")
    print(f"Live Pillow parity: {len(oracle)}/{len(oracle)} exact mode/size/pixel matches ({args.output}, {'eager' if args.eager else 'queued'}).")


if __name__ == "__main__": main()

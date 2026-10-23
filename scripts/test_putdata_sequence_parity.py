"""Compare Python-sequence Image.putdata behavior with isolated Pillow.

Run after ``make build-parity``. These cases cover callback and protocol
ordering that the declarative migration fixtures cannot express.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
import warnings


ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "pillow-rs-py/python"


def capture(image, call):
    error = None
    try:
        call()
    except Exception as exception:
        error = {"type": type(exception).__name__, "message": str(exception)}
    return {"error": error, "bytes": image.tobytes().hex()}


def exact_and_generic_list_observations(Image):
    records = {}
    image = Image.new("L", (3, 1), 9)
    values = [1, None, 3]

    class MutateLaterItem:
        def __float__(self):
            values[2] = 7
            return 2.0

    values[1] = MutateLaterItem()
    records["exact_list_reads_later_items_live"] = capture(
        image, lambda: image.putdata(values)
    )

    image = Image.new("L", (3, 1), 9)
    values = [1, None, 3]

    class MutateMaterializedSource:
        def __float__(self):
            values[2] = 7
            return 2.0

    values[1] = MutateMaterializedSource()

    class GenericSequence:
        def __len__(self):
            return 3

        def __getitem__(self, index):
            return values[index]

    records["generic_sequence_is_materialized_first"] = capture(
        image, lambda: image.putdata(GenericSequence())
    )
    class GetOnly:
        def __getitem__(self, index):
            if index == 0:
                return 7
            raise IndexError

    image = Image.frombytes("L", (2, 1), bytes([91, 92]))
    records["nested_getitem_only_object_is_a_sequence"] = capture(
        image, lambda: image.putdata([1, GetOnly()])
    )
    return records


def outer_sequence_observations(Image):
    records = {}
    image = Image.new("RGB", (1, 1), (9, 9, 9))
    materialization_reads = []

    class DeclaredOne:
        def __len__(self):
            return 1

        def __getitem__(self, index):
            materialization_reads.append((index, image.tobytes().hex()))
            if index == 0:
                return (1, 2, 3)
            if index == 1:
                return "not coerced"
            raise IndexError

    records["declared_length_materializes_before_writes"] = {
        **capture(image, lambda: image.putdata(DeclaredOne())),
        "reads": materialization_reads,
    }

    image = Image.frombytes("L", (2, 1), bytes([91, 92]))

    class BrokenSequence:
        def __len__(self):
            return 2

        def __getitem__(self, index):
            if index == 0:
                return 1
            raise RuntimeError("not propagated")

    records["outer_materialization_error_is_atomic"] = capture(
        image, lambda: image.putdata(BrokenSequence())
    )
    return records


def callback_observations(Image):
    records = {}
    image = Image.frombytes("RGB", (2, 1), bytes([9] * 6))
    observed_pixels = []

    class ObserveIndex:
        def __index__(self):
            observed_pixels.append(list(image.getpixel((0, 0))))
            return 4

    records["rgb_coercion_observes_prior_write"] = {
        **capture(image, lambda: image.putdata([(1, 2, 3), (ObserveIndex(), 5, 6)])),
        "observed_pixels": observed_pixels,
    }

    image = Image.frombytes("L", (2, 1), bytes([9, 9]))
    observed_pixels = []

    class ObserveFloat:
        def __float__(self):
            observed_pixels.append(image.getpixel((0, 0)))
            return 4.0

    records["l_coercion_observes_prior_write"] = {
        **capture(image, lambda: image.putdata([1, ObserveFloat()])),
        "observed_pixels": observed_pixels,
    }
    return records


def bytes_observations(Image):
    records = {}
    class WeirdBytes(bytes):
        def __iter__(self):
            return iter([9, 9])

    image = Image.new("L", (2, 1))
    records["bytes_subclass_uses_raw_storage"] = capture(
        image, lambda: image.putdata(WeirdBytes([1, 2]), scale=2)
    )

    class DeclaredBytes(bytes):
        def __new__(cls, values, declared_length):
            instance = super().__new__(cls, values)
            instance.declared_length = declared_length
            return instance

        def __init__(self, values, declared_length):
            del values
            del declared_length

        def __len__(self):
            return self.declared_length

    for label, declared_length, scale, offset in (
        ("long", 4, 1, 0),
        ("short", 2, 1, 0),
        ("scaled", 4, 2, 1),
    ):
        image = Image.new("L", (4, 1), 9)
        payload = DeclaredBytes([1, 2, 3], declared_length)
        records[f"bytes_subclass_declared_length_{label}"] = capture(
            image,
            lambda payload=payload, scale=scale, offset=offset: image.putdata(
                payload, scale, offset
            ),
        )
    return records


def mixed_pixel_observations(Image):
    records = {}
    image = Image.frombytes("RGB", (2, 1), bytes([9] * 6))
    records["mixed_packed_integer_then_float_preserves_prefix"] = capture(
        image, lambda: image.putdata([0x00030201, 1.5])
    )

    for mode in ("RGB", "LA"):
        image = Image.new(mode, (1, 1))
        records[f"{mode.lower()}_noninteger_singleton_tuple_error"] = capture(
            image, lambda image=image: image.putdata([(1.5,)])
        )

    palette_image = Image.frombytes("P", (1, 1), bytes([7]))
    palette_image.putpalette([255, 0, 0, 0, 255, 0, 0, 0, 255], "RGB")
    palette_image.putalpha(77)
    records["pa_noninteger_singleton_tuple_error"] = capture(
        palette_image, lambda: palette_image.putdata([(1.5,)])
    )

    source = Image.frombytes("L", (2, 1), bytes([1, 2]))
    destination = Image.frombytes("L", (2, 1), bytes([91, 92]))
    records["public_imaging_core_sequence"] = capture(
        destination, lambda: destination.putdata(source.getdata())
    )
    return records


def oversized_sequence_observations(Image):
    records = {}
    image = Image.frombytes("RGB", (1, 1), bytes([9, 10, 11]))
    oversized_reads = []

    class TooMany:
        def __len__(self):
            return 2

        def __getitem__(self, index):
            oversized_reads.append(index)
            return (1, 2, 3)

    records["oversized_sequence_is_rejected_before_iteration"] = {
        **capture(image, lambda: image.putdata(TooMany())),
        "reads": oversized_reads,
    }
    image = Image.frombytes("L", (2, 1), bytes([91, 92]))
    records["iterator_without_sequence_protocol_is_rejected"] = capture(
        image, lambda: image.putdata(iter([1, 2]))
    )
    return records


def observations():
    warnings.filterwarnings("ignore", category=DeprecationWarning)
    from PIL import Image

    records = {}
    for case_group in (
        exact_and_generic_list_observations,
        outer_sequence_observations,
        callback_observations,
        bytes_observations,
        mixed_pixel_observations,
        oversized_sequence_observations,
    ):
        records.update(case_group(Image))
    return records


def run_side(target):
    env = os.environ.copy()
    paths = [
        path
        for path in env.get("PYTHONPATH", "").split(os.pathsep)
        if path and Path(path).resolve() != TARGET.resolve()
    ]
    if target:
        paths.insert(0, str(TARGET))
    env["PYTHONPATH"] = os.pathsep.join(paths)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    process = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "--worker"],
        env=env,
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
        timeout=60,
    )
    result = json.loads(process.stdout)
    loaded_module = Path(result["module"]).resolve()
    loaded_from_target = TARGET.resolve() in loaded_module.parents
    if loaded_from_target != target:
        raise RuntimeError(
            f"wrong PIL import for {'target' if target else 'oracle'}: {result['module']}"
        )
    return result["records"]


class PutDataSequenceParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = run_side(False)
        cls.target = run_side(True)

    def test_behavior_matches_live_pillow(self):
        self.assertEqual(self.target, self.oracle)

    def test_exact_and_generic_sequence_order(self):
        self.assertEqual(
            self.target["exact_list_reads_later_items_live"]["bytes"], "010207"
        )
        self.assertEqual(
            self.target["generic_sequence_is_materialized_first"]["bytes"], "010203"
        )
        self.assertEqual(
            self.target["declared_length_materializes_before_writes"]["reads"],
            [[0, "090909"], [1, "090909"], [2, "090909"]],
        )
        self.assertEqual(
            self.target["nested_getitem_only_object_is_a_sequence"],
            {
                "error": {"type": "TypeError", "message": "sequence must be flattened"},
                "bytes": "015c",
            },
        )
        self.assertEqual(
            self.target["oversized_sequence_is_rejected_before_iteration"]["reads"],
            [],
        )
        self.assertEqual(
            self.target["iterator_without_sequence_protocol_is_rejected"]["bytes"],
            "5b5c",
        )

    def test_coercion_callbacks_observe_completed_pixels(self):
        self.assertEqual(
            self.target["rgb_coercion_observes_prior_write"]["observed_pixels"],
            [[1, 2, 3]],
        )
        self.assertEqual(
            self.target["l_coercion_observes_prior_write"]["observed_pixels"], [1]
        )
        self.assertEqual(
            self.target["outer_materialization_error_is_atomic"]["bytes"], "5b5c"
        )

    def test_bytes_subclasses_use_raw_payload_and_reported_length(self):
        self.assertEqual(
            self.target["bytes_subclass_uses_raw_storage"]["bytes"], "0204"
        )
        self.assertEqual(
            self.target["bytes_subclass_declared_length_long"]["bytes"], "01020300"
        )
        self.assertEqual(
            self.target["bytes_subclass_declared_length_short"]["bytes"], "01020909"
        )
        self.assertEqual(
            self.target["bytes_subclass_declared_length_scaled"]["bytes"], "03050701"
        )

    def test_multiband_error_order_and_public_core_sequence(self):
        self.assertEqual(
            self.target["mixed_packed_integer_then_float_preserves_prefix"]["bytes"],
            "010203090909",
        )
        self.assertEqual(
            self.target["rgb_noninteger_singleton_tuple_error"]["error"],
            {
                "type": "TypeError",
                "message": "color must be int, or tuple of one, three or four elements",
            },
        )
        for mode in ("la", "pa"):
            with self.subTest(mode=mode):
                self.assertEqual(
                    self.target[f"{mode}_noninteger_singleton_tuple_error"]["error"],
                    {
                        "type": "SystemError",
                        "message": "new style getargs format but argument is not a tuple",
                    },
                )
        self.assertEqual(self.target["public_imaging_core_sequence"]["bytes"], "0102")


if __name__ == "__main__":
    if sys.argv[1:] == ["--worker"]:
        import PIL

        print(
            json.dumps(
                {"module": PIL.__file__, "records": observations()}, sort_keys=True
            )
        )
    else:
        unittest.main()

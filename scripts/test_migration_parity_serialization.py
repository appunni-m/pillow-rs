import base64
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from scripts import build_migration_parity_inputs
from scripts.run_migration_parity import AssetStore, serialize_value


class MultibandCore:
    mode = "RGB"
    size = (2, 1)

    def __iter__(self):
        return iter(((17, 31, 47), (53, 67, 79)))

    def tobytes(self):
        return bytes(self)


class UnreadableCore:
    mode = "RGB"
    size = (1, 1)

    def __bytes__(self):
        raise TypeError("multiband values are not bytes")

    def tobytes(self):
        raise TypeError("multiband values are not bytes")


class ParitySerializationTests(unittest.TestCase):
    def test_large_generated_byte_assets_round_trip_through_file_reference(self):
        raw = bytes(range(256)) * 300
        asset = {
            "id": "pixels",
            "kind": "inline",
            "encoding": "base64",
            "data": base64.b64encode(raw).decode("ascii"),
            "sha256": hashlib.sha256(raw).hexdigest(),
            "media_type": "application/octet-stream",
        }
        payload = {"cases": [{"assets": [asset]}]}

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            input_path = root / "inputs" / "parity" / "sample.json"
            old_limit = build_migration_parity_inputs.MAX_INPUT_JSON_BYTES
            build_migration_parity_inputs.MAX_INPUT_JSON_BYTES = 1024
            try:
                build_migration_parity_inputs.write_json(input_path, payload)
            finally:
                build_migration_parity_inputs.MAX_INPUT_JSON_BYTES = old_limit

            document = json.loads(input_path.read_text(encoding="utf-8"))
            stored = document["cases"][0]["assets"][0]
            self.assertEqual(stored["id"], "pixels")
            self.assertEqual(stored["kind"], "ref_bytes")
            self.assertEqual(stored["sha256"], hashlib.sha256(raw).hexdigest())
            store = AssetStore([stored], root / "assets", root / "tmp")
            self.assertEqual(store.resolve("pixels"), raw)

    def test_large_encoded_image_assets_remain_path_inputs(self):
        raw = b"image-bytes" * 7000
        asset = {
            "id": "encoded-image",
            "kind": "inline",
            "encoding": "base64",
            "data": base64.b64encode(raw).decode("ascii"),
            "sha256": hashlib.sha256(raw).hexdigest(),
            "media_type": "image/png",
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            input_path = root / "inputs" / "parity" / "sample.json"
            old_limit = build_migration_parity_inputs.MAX_INPUT_JSON_BYTES
            build_migration_parity_inputs.MAX_INPUT_JSON_BYTES = 1024
            try:
                build_migration_parity_inputs.write_json(
                    input_path, {"cases": [{"assets": [asset]}]}
                )
            finally:
                build_migration_parity_inputs.MAX_INPUT_JSON_BYTES = old_limit

            stored = json.loads(input_path.read_text(encoding="utf-8"))["cases"][0][
                "assets"
            ][0]
            self.assertEqual(stored["kind"], "ref")
            store = AssetStore([stored], root / "assets", root / "tmp")
            stored_path = Path(store.resolve("encoded-image"))
            self.assertEqual(stored_path.read_bytes(), raw)

    def test_multiband_mask_keeps_pixel_tuples_when_bytes_rejects_them(self):
        value = serialize_value(
            MultibandCore(),
            "mask",
            side="source",
            surface="PIL.Image.Image",
            operation="getdata",
        )

        self.assertEqual(
            value,
            {
                "kind": "mask",
                "mode": "RGB",
                "size": [2, 1],
                "bytes": "",
                "pixels": [[17, 31, 47], [53, 67, 79]],
            },
        )

    def test_scalar_mask_keeps_the_compact_byte_representation(self):
        value = serialize_value(
            [0, 17, 255],
            "mask",
            side="source",
            surface="PIL.Image.Image",
            operation="getdata",
        )

        self.assertEqual(value["bytes"], "ABH/")
        self.assertNotIn("pixels", value)

    def test_failed_multiband_iteration_is_not_replaced_with_empty_bytes(self):
        with self.assertRaisesRegex(TypeError, "not iterable"):
            serialize_value(
                UnreadableCore(),
                "mask",
                side="source",
                surface="PIL.Image.Image",
                operation="getdata",
            )


if __name__ == "__main__":
    unittest.main()

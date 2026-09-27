import unittest

from scripts.run_migration_parity import serialize_value


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

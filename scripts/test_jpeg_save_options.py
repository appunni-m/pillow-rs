"""Exercise JPEG save options through the public PIL interface."""

import unittest
from io import BytesIO
from pathlib import Path
from tempfile import TemporaryDirectory

from PIL import Image


def jpeg_headers(encoded):
    """Collect marker payloads up to the start of the compressed scan."""
    headers = {}
    offset = 2  # SOI
    while encoded[offset : offset + 1] == b"\xff":
        marker = encoded[offset + 1]
        if marker == 0xDA:  # SOS
            break
        length = int.from_bytes(encoded[offset + 2 : offset + 4], "big")
        end = offset + 2 + length
        headers.setdefault(marker, []).append(encoded[offset + 4 : end])
        offset = end
    return headers


class JpegSaveOptionsTests(unittest.TestCase):
    def setUp(self):
        width, height = 48, 36
        pixels = bytes((n * 37 + n // 7) % 256 for n in range(width * height * 3))
        self.image = Image.frombytes("RGB", (width, height), pixels)

    def encode(self, **options):
        out = BytesIO()
        self.image.save(out, format="JPEG", **options)
        return out.getvalue()

    def test_quality_changes_quantization_and_size(self):
        low = self.encode(quality=35, subsampling=0)
        high = self.encode(quality=90, subsampling=0)
        self.assertNotEqual(jpeg_headers(low)[0xDB], jpeg_headers(high)[0xDB])
        self.assertLess(len(low), len(high))

    def test_subsampling_in_frame_header_for_stream_and_path(self):
        with TemporaryDirectory() as directory:
            path = Path(directory) / "output.jpg"
            for number, name, y_sampling in (
                (0, "4:4:4", 0x11),
                (1, "4:2:2", 0x21),
                (2, "4:2:0", 0x22),
            ):
                with self.subTest(subsampling=name):
                    encoded = self.encode(quality=90, subsampling=number)
                    self.image.save(path, quality=90, subsampling=name)
                    self.assertEqual(path.read_bytes(), encoded)
                    (frame,) = jpeg_headers(encoded)[0xC0]  # SOF0
                    self.assertEqual(frame[7::3], bytes((y_sampling, 0x11, 0x11)))

    def test_quality_and_sampling_can_be_set_independently(self):
        frame = jpeg_headers(self.encode(subsampling=0))[0xC0][0]
        self.assertEqual(frame[7::3], b"\x11\x11\x11")
        self.assertNotEqual(
            jpeg_headers(self.encode(quality=35))[0xDB],
            jpeg_headers(self.encode(quality=90))[0xDB],
        )

    def test_encoder_defaults_and_quality_limits(self):
        self.assertEqual(self.encode(quality=-1, subsampling=-1), self.encode())
        self.assertEqual(self.encode(quality=0), self.encode(quality=1))
        self.assertEqual(self.encode(quality=101), self.encode(quality=100))
        self.assertEqual(self.encode(subsampling="4:1:1"), self.encode(subsampling=2))

    def test_subsampling_tuple_is_not_a_pillow_sampling_value(self):
        with self.assertRaises(TypeError):
            self.encode(subsampling=(4, 4, 4))

    def test_png_still_ignores_jpeg_options(self):
        expected, actual = BytesIO(), BytesIO()
        self.image.save(expected, "PNG", compress_level=1)
        self.image.save(actual, "PNG", compress_level=1, quality="unused", subsampling=())
        self.assertEqual(expected.getvalue(), actual.getvalue())


if __name__ == "__main__":
    unittest.main()

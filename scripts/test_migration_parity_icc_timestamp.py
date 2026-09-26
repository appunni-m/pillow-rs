"""Tests for narrowly scoped comparison of Pillow's live LAB profile time."""

from __future__ import annotations

import base64
from pathlib import Path
import unittest

from scripts.run_migration_parity import compare_value


ROOT = Path(__file__).resolve().parents[1]
LAB_PROFILE_TEMPLATE = ROOT / "pillow-rs/src/data/lab-identity-profile.icc"


def lab_image(
    timestamp: tuple[int, int, int, int, int, int], *, pixel: bytes = b"pixels"
) -> dict[str, object]:
    profile = bytearray(LAB_PROFILE_TEMPLATE.read_bytes())
    for index, value in enumerate(timestamp):
        profile[24 + index * 2 : 26 + index * 2] = value.to_bytes(2, "big")
    return {
        "kind": "image",
        "mode": "LAB",
        "size": [2, 1],
        "format": None,
        "info": {
            "icc_profile": {
                "kind": "bytes",
                "encoding": "base64",
                "data": base64.b64encode(profile).decode("ascii"),
            }
        },
        "palette": None,
        "bytes": base64.b64encode(pixel).decode("ascii"),
    }


class LabProfileTimestampComparisonTests(unittest.TestCase):
    def compare(self, source: dict[str, object], target: dict[str, object]):
        return compare_value(source, target, {"kind": "image"}, "call.value")

    def test_live_timestamp_may_cross_a_second(self) -> None:
        source = lab_image((2026, 9, 26, 23, 59, 59))
        target = lab_image((2026, 9, 27, 0, 0, 0))
        self.assertEqual(self.compare(source, target), [])

    def test_pixels_and_every_other_profile_byte_remain_exact(self) -> None:
        source = lab_image((2026, 9, 26, 12, 0, 0))
        pixel_mismatch = lab_image((2026, 9, 26, 12, 0, 1), pixel=b"pixEls")
        self.assertTrue(self.compare(source, pixel_mismatch))

        profile_mismatch = bytearray(LAB_PROFILE_TEMPLATE.read_bytes())
        profile_mismatch[100] ^= 1
        for index, value in enumerate((2026, 9, 26, 12, 0, 1)):
            profile_mismatch[24 + index * 2 : 26 + index * 2] = value.to_bytes(2, "big")
        changed_profile = lab_image((2026, 9, 26, 12, 0, 1))
        changed_profile["info"]["icc_profile"]["data"] = base64.b64encode(
            profile_mismatch
        ).decode("ascii")
        self.assertTrue(self.compare(source, changed_profile))

    def test_invalid_or_unrelated_timestamps_still_fail(self) -> None:
        valid = lab_image((2026, 9, 26, 12, 0, 0))
        invalid = lab_image((2026, 13, 26, 12, 0, 1))
        unrelated = lab_image((2029, 9, 26, 12, 0, 1))
        self.assertTrue(self.compare(valid, invalid))
        self.assertTrue(self.compare(valid, unrelated))


if __name__ == "__main__":
    unittest.main()

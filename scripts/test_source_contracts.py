from __future__ import annotations

import unittest

from scripts.check_source_contracts import Finding, growth, scan_source


class SourceContractTests(unittest.TestCase):
    def test_test_only_constructors_are_excluded(self) -> None:
        source = """fn production() {
    let image = DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, bytes)?);
}
#[cfg(test)]
mod tests {
    let image = DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, bytes)?);
}
"""

        findings = scan_source("pillow-rs/src/sample.rs", source)

        self.assertEqual([finding.line for finding in findings], [2])

    def test_test_only_helper_file_is_excluded(self) -> None:
        source = "let image = DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, bytes)?);"

        self.assertEqual(scan_source("pillow-rs/src/sample_tests.rs", source), [])

    def test_public_palette_error_signatures_remain_compatibility_exemptions(self) -> None:
        public_source = """pub fn palette_getcolor_validate() -> Result<usize, String> {
    Ok(0)
}
"""
        private_source = """fn palette_getcolor_validate_impl() -> Result<usize, String> {
    Ok(0)
}
"""

        self.assertEqual(scan_source("pillow-rs/src/color.rs", public_source), [])
        self.assertEqual(
            [
                finding.contract
                for finding in scan_source("pillow-rs/src/color.rs", private_source)
            ],
            ["string_error_types"],
        )

    def test_per_file_ratchet_rejects_growth_and_allows_removal(self) -> None:
        baseline = [
            Finding("pillow-rs/src/a.rs", 10, "raw_image_construction", "old"),
            Finding("pillow-rs/src/a.rs", 11, "raw_image_construction", "old"),
        ]
        same = [Finding("pillow-rs/src/a.rs", 50, "raw_image_construction", "moved")]
        growth_findings = [
            Finding("pillow-rs/src/a.rs", 10, "raw_image_construction", "old"),
            Finding("pillow-rs/src/a.rs", 11, "raw_image_construction", "old"),
            Finding("pillow-rs/src/a.rs", 12, "raw_image_construction", "new"),
        ]

        self.assertEqual(growth(baseline, same), {})
        self.assertEqual(
            growth(baseline, growth_findings),
            {("pillow-rs/src/a.rs", "raw_image_construction"): (2, 3)},
        )


if __name__ == "__main__":
    unittest.main()

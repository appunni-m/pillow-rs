import unittest

from scripts.migration_parity_inventory import (
    CORRECTIONS,
    EXCLUDED_ENDPOINTS,
    EXPECTED_AUTHORITY_SHA256,
    EXPECTED_LEGACY_ROWS,
    EXPECTED_LEGACY_UNIQUE_ENDPOINTS,
    derive_inventory,
    render_json,
)


class MigrationParityInventoryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.endpoints, cls.legacy_rows = derive_inventory()
        cls.by_id = {endpoint.id: endpoint for endpoint in cls.endpoints}

    def test_denominator_is_frozen(self):
        self.assertEqual(self.legacy_rows, EXPECTED_LEGACY_ROWS)
        self.assertEqual(
            len(self.endpoints),
            EXPECTED_LEGACY_UNIQUE_ENDPOINTS
            + len(CORRECTIONS)
            - len(EXCLUDED_ENDPOINTS),
        )
        self.assertEqual(len(self.by_id), len(self.endpoints))

    def test_every_legacy_row_is_accounted_for_once(self):
        all_endpoints, _ = derive_inventory(include_excluded=True)
        references = [
            reference.id
            for endpoint in all_endpoints
            for reference in endpoint.legacy_refs
        ]
        self.assertEqual(len(references), EXPECTED_LEGACY_ROWS)
        self.assertEqual(len(set(references)), EXPECTED_LEGACY_ROWS)

    def test_only_known_legacy_aliases_are_merged(self):
        merged = {
            endpoint.id: tuple(
                reference.id for reference in endpoint.legacy_refs
            )
            for endpoint in self.endpoints
            if len(endpoint.legacy_refs) > 1
        }
        self.assertEqual(
            merged,
            {
                "PIL.Image::new": (
                    "Image.class_methods.new",
                    "ImageModule.functions.new",
                ),
                "PIL.Image::open": (
                    "Image.class_methods.open",
                    "ImageModule.functions.open",
                ),
            },
        )

    def test_corrections_are_explicit_and_reasoned(self):
        correction_ids = {
            endpoint.id
            for endpoint in self.endpoints
            if endpoint.authority == "workflow-correction"
        }
        self.assertEqual(
            correction_ids,
            {
                "PIL.ImageDraw::Draw",
                "PIL.ImageDraw::Outline",
                "PIL.ImageEnhance.Brightness::enhance",
                "PIL.ImageEnhance.Color::enhance",
                "PIL.ImageEnhance.Contrast::enhance",
                "PIL.ImageEnhance.Sharpness::enhance",
                "PIL.ImagePalette::ImagePalette",
                "PIL.ImageFilter.Color3DLUT::__repr__",
                "PIL.ImageSequence.Iterator::__iter__",
                "PIL.ImageSequence.Iterator::__next__",
            },
        )
        for endpoint in self.endpoints:
            if endpoint.id in correction_ids:
                self.assertIsNotNone(endpoint.correction_reason)
                self.assertEqual(endpoint.legacy_refs, ())

    def test_headless_endpoints_are_excluded_from_active_scope(self):
        all_endpoints, _ = derive_inventory(include_excluded=True)
        all_ids = {endpoint.id for endpoint in all_endpoints}
        self.assertTrue(EXCLUDED_ENDPOINTS.keys() <= all_ids)
        self.assertTrue(EXCLUDED_ENDPOINTS.keys().isdisjoint(self.by_id))

    def test_font_uses_canonical_public_surfaces(self):
        font_endpoints = [
            endpoint
            for endpoint in self.endpoints
            if endpoint.source_path.startswith("PIL.ImageFont")
        ]
        self.assertTrue(font_endpoints)
        self.assertTrue(
            all(
                endpoint.surface.startswith("PIL.ImageFont")
                for endpoint in font_endpoints
            )
        )
        self.assertNotIn("font", {endpoint.surface for endpoint in self.endpoints})

    def test_diagnostic_json_records_authority_digest(self):
        payload = render_json(self.endpoints, self.legacy_rows)
        self.assertIn(EXPECTED_AUTHORITY_SHA256, payload)
        self.assertIn('"endpoint_count": 206', payload)


if __name__ == "__main__":
    unittest.main()

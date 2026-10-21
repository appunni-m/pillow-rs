import unittest

from scripts import run_migration_parity


class MigrationParitySelectionTests(unittest.TestCase):
    def test_unknown_requested_case_id_is_rejected(self) -> None:
        manifest = run_migration_parity.load_manifest(
            run_migration_parity.DEFAULT_MANIFEST
        )

        with self.assertRaisesRegex(
            ValueError, "unknown requested parity case ID"
        ):
            run_migration_parity.load_cases(
                manifest,
                case_ids={"PIL.Image.Image.filter.unknown-requested-case"},
                surface=None,
            )


if __name__ == "__main__":
    unittest.main()

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

    def test_parallel_cpu_posterize_threshold_case_is_declared(self) -> None:
        manifest = run_migration_parity.load_manifest(
            run_migration_parity.DEFAULT_MANIFEST
        )
        posterize = next(
            operation
            for surface in manifest["surfaces"]
            if surface["id"] == "PIL.ImageOps"
            for operation in surface["operations"]
            if operation["id"] == "posterize"
        )
        requirement_id = "PIL.ImageOps.posterize.backend.parallel-cpu"
        self.assertEqual(
            posterize["parity"]["target_profiles"],
            ["python-cpu", "python-parallel-cpu"],
        )
        requirement = next(
            item for item in posterize["requirements"] if item["id"] == requirement_id
        )
        self.assertEqual(requirement["dimension"], "backend")
        self.assertEqual(requirement["lanes"], ["parity"])
        self.assertEqual(requirement["target_profiles"], ["python-parallel-cpu"])

        cases, _ = run_migration_parity.load_cases(
            manifest,
            case_ids={
                "PIL.ImageOps.posterize.nuanced."
                "performance-large-l-noise-1024x768"
            },
            surface=None,
        )
        self.assertEqual(len(cases), 1)
        case = cases[0]
        self.assertIn("python-parallel-cpu", case["target_profiles"])
        self.assertIn(requirement_id, case["covers"])
        source = next(
            step
            for step in case["steps"]
            if step["surface"] == "PIL.Image" and step["operation"] == "frombytes"
        )
        self.assertEqual(source["arguments"]["mode"]["value"], "L")
        self.assertEqual(source["arguments"]["size"]["value"], [1024, 768])


if __name__ == "__main__":
    unittest.main()

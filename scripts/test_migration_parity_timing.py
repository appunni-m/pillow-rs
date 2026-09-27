import unittest

from scripts.run_migration_parity import build_argument_parser, effective_timing_steps


class MigrationParityTimingTests(unittest.TestCase):
    def test_unspecified_timing_steps_keep_call_as_the_default(self):
        args = build_argument_parser().parse_args([])
        self.assertEqual(effective_timing_steps(args.timing_step), {"call"})

    def test_explicit_timing_steps_replace_the_default_call_step(self):
        args = build_argument_parser().parse_args(
            [
                "--timing-step",
                "apply-filter",
                "--timing-step",
                "observe-filter-result",
            ]
        )
        self.assertEqual(
            effective_timing_steps(args.timing_step),
            {"apply-filter", "observe-filter-result"},
        )


if __name__ == "__main__":
    unittest.main()

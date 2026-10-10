import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from scripts.run_migration_parity import (
    _run_side_subprocess_batch,
    build_argument_parser,
    effective_timing_steps,
)


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

    def test_execution_receipt_fields_are_optional_adapter_envelope_fields(self):
        case = {"case_id": "filter-case"}
        identity = {"side": "target"}
        result = {"case_id": "filter-case", "status": "completed"}
        process = Mock()
        process.communicate.return_value = (
            '{"identity":{"side":"target"},'
            '"results":[{"case_id":"filter-case","status":"completed"}],'
            '"timings_ns":{},"telemetry":{},'
            '"execution":{"filter-case":[]}}',
            "",
        )
        process.returncode = 0

        with patch("scripts.run_migration_parity.subprocess.Popen", return_value=process):
            actual_identity, actual_results = _run_side_subprocess_batch(
                "target", Path("manifest.yaml"), [case], 10
            )

        self.assertEqual(actual_identity, identity)
        self.assertEqual(actual_results, {"filter-case": result})


if __name__ == "__main__":
    unittest.main()

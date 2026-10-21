import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from scripts.run_migration_benchmark import execution_operation_telemetry
from scripts.run_migration_parity import (
    _phase_totals,
    _run_side_subprocess_batch,
    build_argument_parser,
    effective_timing_steps,
    run_resident_case,
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

    def test_untimed_frombytes_fixture_setup_is_not_reported_as_pipeline_work(self):
        case = {
            "steps": [
                {"step_id": "image", "operation": "frombytes"},
                {"step_id": "call", "operation": "resize"},
                {"step_id": "materialize", "operation": "tobytes"},
            ]
        }
        totals = _phase_totals(
            case,
            [
                {"step_id": "image", "duration_ns": 300},
                {"step_id": "call", "duration_ns": 200},
                {"step_id": "materialize", "duration_ns": 400},
            ],
            0,
            timing_steps={"call", "materialize"},
        )

        self.assertEqual(totals["setup_ns"], 300)
        self.assertEqual(totals["pipeline_ns"], 200)
        self.assertEqual(totals["terminal_ns"], 400)

    def test_timed_frombytes_public_call_remains_pipeline_work(self):
        case = {"steps": [{"step_id": "call", "operation": "frombytes"}]}

        totals = _phase_totals(
            case,
            [{"step_id": "call", "duration_ns": 300}],
            0,
            timing_steps={"call"},
        )

        self.assertEqual(totals["setup_ns"], 0)
        self.assertEqual(totals["pipeline_ns"], 300)

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

    def test_resident_telemetry_receipt_keeps_the_step_that_emitted_it(self):
        receipt = {
            "operation_count": 1,
            "actual_backend": "simd",
            "operation_telemetry": [
                {"operation": "Quantize", "path": "vector"}
            ],
        }
        telemetry_api = Mock(take_pipeline_telemetry=Mock(side_effect=[None, receipt]))
        execution = []
        case = {
            "case_id": "resident-quantize",
            "assets": [],
            "steps": [
                {
                    "step_id": "image",
                    "surface": "PIL.Image",
                    "operation": "new",
                },
                {
                    "step_id": "quantize",
                    "surface": "PIL.Image.Image",
                    "operation": "quantize",
                },
            ],
        }

        with patch("scripts.run_migration_parity.AssetStore"), patch(
            "scripts.run_migration_parity.operation_definition", return_value={}
        ), patch(
            "scripts.run_migration_parity.call_workflow_step", return_value=object()
        ), patch(
            "scripts.run_migration_parity.should_lock_target_backend", return_value=False
        ):
            run_resident_case(
                "target",
                case,
                {},
                Path("unused"),
                repeat=1,
                timing_steps={"quantize"},
                timing_sink=[],
                telemetry_sink=[],
                execution_sink=execution,
                timing_boundary="observed_steps",
                telemetry_api=telemetry_api,
            )

        self.assertEqual(execution[0]["step_id"], "quantize")
        self.assertEqual(
            execution_operation_telemetry(execution)[0]["step_id"], "quantize"
        )


if __name__ == "__main__":
    unittest.main()

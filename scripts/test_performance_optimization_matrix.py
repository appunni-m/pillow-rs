"""Protect SIMD acceleration evidence without changing measured latencies."""

import copy
import unittest

from build_performance_optimization_matrix import (
    SIMD_POSITIVE_VECTOR_PATHS,
    classify,
    matrix_rows,
    simd_vector_evidence,
)


def measured_row(subject="python-simd"):
    backend = subject.removeprefix("python-")
    return {
        "subject": subject,
        "workload": "channel-extraction",
        "machine_id": "runner",
        "status": "completed",
        "median_us": 10,
        "correctness": "parity_pass: pass",
        "policy": {"correctness_gate": "parity_pass"},
        "context": {"mode": "L", "size": [1024, 768]},
        "actual_backend": backend,
        "requested_backend": backend,
        "terminal_complete": True,
        "fallback_reasons": {},
        "selected_step_ids": ["call"],
    }


def vector_event(path, count):
    return {
        "step_id": "call",
        "operation": "ExtractBand",
        "path": path,
        "vector_block_count": count,
        "scalar_tail_count": 0,
        "mode_conversion_count": 0,
        "handoff_count": 0,
    }


class SimdVectorEvidenceTests(unittest.TestCase):
    def test_backend_label_without_vector_evidence_is_not_acceleration(self):
        row = measured_row()
        self.assertEqual(simd_vector_evidence(row), "missing")
        self.assertEqual(classify(row, True), "simd_vector_not_proven")

    def test_zero_vector_scalar_copy_is_not_acceleration(self):
        row = measured_row()
        row["operation_telemetry"] = [
            vector_event("scalar-gather", 0)
        ]
        self.assertEqual(simd_vector_evidence(row), "no_vector_work")
        self.assertEqual(classify(row, True), "simd_vector_not_proven")

    def test_positive_native_vector_work_is_verified(self):
        row = measured_row()
        row["operation_telemetry"] = [
            vector_event("neon-deinterleave", 49_152)
        ]
        self.assertEqual(simd_vector_evidence(row), "positive_vector_work")
        self.assertEqual(classify(row, True), "parity_backend_verified")
        row["fallback_reasons"] = {"unsupported input": 1}
        self.assertEqual(classify(row, True), "fallback")

    def test_vector_event_from_unselected_step_is_not_acceleration(self):
        row = measured_row()
        row["operation_telemetry"] = [
            vector_event("scalar-gather", 0),
            {**vector_event("neon-deinterleave", 64), "step_id": "unselected-step"},
        ]
        self.assertEqual(simd_vector_evidence(row), "invalid")
        self.assertEqual(classify(row, True), "simd_vector_not_proven")

    def test_positive_vector_work_requires_a_current_supported_path(self):
        expected_paths = {
            "native-copy",
            "neon-blend",
            "neon-cmyk-l-blend",
            "neon-copy",
            "neon-deinterleave",
            "neon-f64-resize",
            "neon-interleave",
            "neon-la-l-blend",
            "neon-rgba-l-blend",
            "avx2-transpose",
            "rgba-l-mask-vector",
            "ssse3-deinterleave",
            "ssse3-gather",
            "ssse3-interleave",
            "vector",
            "vector-gather",
            "vector-rgb-to-rgba-copy",
            "wide-shuffle",
        }
        self.assertEqual(SIMD_POSITIVE_VECTOR_PATHS, expected_paths)
        for path in sorted(expected_paths):
            with self.subTest(path=path):
                row = measured_row()
                row["operation_telemetry"] = [vector_event(path, 1)]
                self.assertEqual(simd_vector_evidence(row), "positive_vector_work")

        for path in (
            "scalar-control",
            "unsupported",
            "scalar-gather",
            "native-identity",
            "native-crop",
            "native-zero-fill",
            "neon",
            "unknown-vector-path",
        ):
            with self.subTest(path=path):
                row = measured_row()
                row["operation_telemetry"] = [vector_event(path, 1)]
                self.assertEqual(simd_vector_evidence(row), "invalid")
                self.assertEqual(classify(row, True), "simd_vector_not_proven")

    def test_malformed_vector_proof_is_rejected(self):
        invalid = [
            None,
            [],
            {},
            [None],
            [vector_event("neon-deinterleave", True)],
            [vector_event("neon-deinterleave", -1)],
            [vector_event("neon-deinterleave", "12")],
            [{**vector_event("neon-deinterleave", 1), "path": ""}],
        ]
        for value in invalid:
            with self.subTest(value=value):
                row = measured_row()
                row["operation_telemetry"] = value
                self.assertEqual(classify(row, True), "simd_vector_not_proven")

    def test_cpu_evidence_does_not_require_vector_work(self):
        self.assertEqual(classify(measured_row("python-cpu"), True), "parity_backend_verified")

    def test_matrix_preserves_observed_ratios_and_withholds_simd_and_gpu_credit(self):
        simd = measured_row()
        pillow = measured_row("pillow")
        pillow["median_us"] = 20
        gpu = measured_row("python-gpu")
        source = "github-pages/benchmark.json"
        snapshot = {"_source_snapshot": source, "clean": True, "rows": [pillow, simd, gpu]}
        rows = matrix_rows([snapshot], {source: "a" * 64}, {}, {})
        simd_row = next(row for row in rows if row["profile"] == "python-simd")
        self.assertEqual(simd_row["observed_speedup_vs_pillow"], "2.000000")
        self.assertEqual(simd_row["verified_speedup_vs_pillow"], "")
        self.assertEqual(simd_row["simd_2x_goal_status"], "simd_vector_not_proven")
        gpu_row = next(row for row in rows if row["profile"] == "python-gpu")
        self.assertEqual(gpu_row["observed_gpu_latency_speedup_vs_simd"], "1.000000")
        self.assertEqual(gpu_row["verified_gpu_latency_speedup_vs_simd"], "")
        proven = copy.deepcopy(snapshot)
        proven["rows"][1]["operation_telemetry"] = [
            vector_event("neon-deinterleave", 49_152)
        ]
        rows = matrix_rows([proven], {source: "a" * 64}, {}, {})
        self.assertEqual(next(row for row in rows if row["profile"] == "python-simd")
                         ["verified_speedup_vs_pillow"], "2.000000")


if __name__ == "__main__":
    unittest.main()

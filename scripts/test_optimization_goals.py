"""Prevent incomplete or fallback measurements from satisfying optimization goals."""

import copy
import unittest

from scripts.report_optimization_goals import assess_workload


def measured_subject(name, milliseconds):
    backend = name.removeprefix("python-")
    return {
        "id": name, "status": "completed",
        "measurements": [{"metric": "latency", "unit": "millisecond", "sample_count": 100,
                          "statistics": {"median": milliseconds, "mean": milliseconds}}],
        "execution": {"status": "not_applicable" if name == "pillow" else "completed",
                      "terminal_complete": name != "pillow", "requested_backend": backend,
                      "actual_backend": backend, "actual_backend_counts": {backend: 100},
                      "sample_count": 100, "fallback_reason_counts": {}, "errors": []},
    }


class OptimizationGoalEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.spec = {"workload_id": "operation", "covers": ["op.performance"],
                     "input": {"kind": "parity_case", "case_id": "case"},
                     "measurement": {"cache_state": "warm", "measurement_iterations": 20, "samples": 5},
                     "context": {"mode": "RGB"}}
        self.result = {"measurement_policy": self.spec["measurement"], "context": self.spec["context"],
                       "correctness": {"evidence_id": "gate"},
                       "subjects": [measured_subject(name, time) for name, time in
                                    (("pillow", 10), ("python-cpu", 5), ("python-simd", 1), ("python-gpu", 0.5))]}
        self.parity = {("case", profile): "pass" for profile in ("python-cpu", "python-simd", "python-gpu")}

    def assess(self):
        return assess_workload(self.spec, self.result, self.parity, "gate", [])

    def test_latency_win_cannot_prove_sustained_throughput(self):
        row = self.assess()
        self.assertEqual(row["subjects"]["python-gpu"]["status"], "sample_meets_target")
        self.assertEqual(row["sustained_gpu_throughput"]["status"], "not_proven")

    def test_cpu_only_parity_does_not_prove_accelerated_output(self):
        self.parity = {("case", "python-cpu"): "pass"}
        row = self.assess()
        self.assertEqual(row["subjects"]["python-cpu"]["status"], "sample_meets_target")
        for profile in ("python-simd", "python-gpu"):
            self.assertEqual(row["subjects"][profile]["status"], "not_proven")

    def test_fast_fallback_cannot_pass_requested_gpu_goal(self):
        execution = self.result["subjects"][-1]["execution"]
        execution.update(actual_backend="cpu", actual_backend_counts={"cpu": 100},
                         fallback_reason_counts={"unsupported": 100})
        state = self.assess()["subjects"]["python-gpu"]
        self.assertTrue(state["observed_target_met"])
        self.assertEqual(state["status"], "not_proven")

    def test_execution_only_workflow_has_no_output_proof(self):
        self.spec["input"] = {"kind": "workflow"}
        for state in self.assess()["subjects"].values():
            self.assertIn("matching_output_not_proven", state["reasons"])

    def test_wrong_parity_receipt_cannot_admit_fast_samples(self):
        self.result["correctness"]["evidence_id"] = "different-run"
        for state in self.assess()["subjects"].values():
            self.assertEqual(state["status"], "not_proven")
            self.assertIn("unlinked_parity_evidence", state["reasons"])

    def test_resident_timings_do_not_prove_fresh_request_latency(self):
        self.spec = copy.deepcopy(self.spec)
        self.spec["measurement"]["cache_state"] = "resident"
        self.result["measurement_policy"] = self.spec["measurement"]
        for state in self.assess()["subjects"].values():
            self.assertEqual(state["status"], "not_proven")

    def test_shortened_sampling_policy_cannot_pass(self):
        subject = self.result["subjects"][-1]
        subject["measurements"][0]["sample_count"] = 1
        subject["execution"]["sample_count"] = 1
        self.assertIn("incomplete_declared_samples", self.assess()["subjects"]["python-gpu"]["reasons"])


if __name__ == "__main__":
    unittest.main()

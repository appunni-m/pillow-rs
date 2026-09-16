"""Guard benchmark provenance and freshness across Pages deployment events."""
from __future__ import annotations

import copy
import unittest
from unittest.mock import Mock

from select_docs_benchmark import select_run


class BenchmarkSelectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.run = {"id": 42, "path": ".github/workflows/benchmark.yml", "event": "push",
                    "status": "completed", "conclusion": "success", "head_branch": "main",
                    "head_sha": "a" * 40, "head_repository": {"full_name": "owner/repo"}}
        self.artifacts = {"artifacts": [{"name": "public-benchmark", "expired": False}]}

    def test_docs_push_and_benchmark_event_select_the_same_verified_run(self) -> None:
        fetch = Mock(side_effect=[{"workflow_runs": [self.run]}, self.artifacts])
        selected = select_run("owner/repo", "push", {}, fetch)
        self.assertEqual(selected, {"run_id": "42", "head_sha": "a" * 40})
        self.assertIn("branch=main&status=success", fetch.call_args_list[0].args[0])
        self.assertEqual(select_run("owner/repo", "workflow_run", {"workflow_run": self.run},
                                    Mock(return_value=self.artifacts)), selected)

    def test_pull_requests_use_committed_data_without_accessing_artifacts(self) -> None:
        fetch = Mock(side_effect=AssertionError("PR must not fetch hosted artifacts"))
        self.assertIsNone(select_run("owner/repo", "pull_request", {}, fetch))
        fetch.assert_not_called()

    def test_missing_and_expired_data_only_allow_an_explicitly_named_fallback(self) -> None:
        self.assertIsNone(select_run("owner/repo", "push", {}, Mock(return_value={"workflow_runs": []})))
        for payload in [{"artifacts": []}, {"artifacts": [{"name": "public-benchmark", "expired": True}]}]:
            fetch = Mock(side_effect=[{"workflow_runs": [self.run]}, payload])
            self.assertIsNone(select_run("owner/repo", "push", {}, fetch))
            with self.assertRaisesRegex(ValueError, "no available"):
                select_run("owner/repo", "workflow_run", {"workflow_run": self.run}, Mock(return_value=payload))

    def test_foreign_failed_or_non_main_runs_cannot_supply_data(self) -> None:
        changes = [("head_branch", "feature"), ("conclusion", "failure"), ("status", "in_progress"),
                   ("event", "pull_request"), ("path", ".github/workflows/other.yml"),
                   ("head_sha", "invalid\nvalue"), ("head_repository", {"full_name": "fork/repo"})]
        for key, value in changes:
            run = copy.deepcopy(self.run)
            run[key] = value
            with self.subTest(field=key), self.assertRaises(ValueError):
                select_run("owner/repo", "workflow_run", {"workflow_run": run}, Mock())

    def test_network_errors_and_duplicate_artifacts_are_not_hidden(self) -> None:
        with self.assertRaises(OSError):
            select_run("owner/repo", "push", {}, Mock(side_effect=OSError("network unavailable")))
        with self.assertRaisesRegex(ValueError, "duplicate"):
            select_run("owner/repo", "workflow_run", {"workflow_run": self.run},
                       Mock(return_value={"artifacts": self.artifacts["artifacts"] * 2}))


if __name__ == "__main__":
    unittest.main()

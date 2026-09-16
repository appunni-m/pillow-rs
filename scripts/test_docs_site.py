"""Regression guards for public documentation and benchmark presentation."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from docs_evidence import SCHEMA, cell, number, project_jpeg, project_pillow, validate
from docs_site import anchors, prepare_output, read_config, rewrite_links


class DocumentationTests(unittest.TestCase):
    def test_links_follow_published_pages_and_keep_repository_source_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            (root / "docs/guide.md").write_text("# Guide\n")
            (root / "Cargo.toml").write_text("")
            config = {"repository": "owner/library", "pages": [
                {"source": "docs/guide.md", "output": "guide.md"}]}
            result = rewrite_links("[Guide](docs/guide.md#guide) [Cargo](Cargo.toml)", root / "README.md", root, config)
            self.assertIn("(guide.md#guide)", result)
            self.assertIn("https://github.com/owner/library/blob/main/Cargo.toml", result)

    def test_unowned_output_and_symlink_cannot_delete_user_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "target/docs-source"
            output.mkdir(parents=True)
            sentinel = output / "important.txt"
            sentinel.write_text("user content")
            with self.assertRaises(ValueError):
                prepare_output(root)
            self.assertEqual(sentinel.read_text(), "user content")
            other = root / "other"
            other.mkdir()
            linked_root = root / "linked"
            linked_root.mkdir()
            (linked_root / "target").symlink_to(other, target_is_directory=True)
            with self.assertRaises(ValueError):
                prepare_output(linked_root)

    def test_owned_output_is_repeatable_and_preserves_sources(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "README.md"
            source.write_text("source")
            first = prepare_output(root)
            (first / "stale.html").write_text("old output")
            second = prepare_output(root)
            self.assertFalse((second / "stale.html").exists())
            self.assertEqual(source.read_text(), "source")

    def test_code_fences_do_not_create_fake_heading_anchors(self) -> None:
        result = anchors("# Guide\n\n```python\n# Not a heading\n```\n\n## Setup\n")
        self.assertEqual(result, {"guide", "setup"})


class BenchmarkTests(unittest.TestCase):
    def test_incomplete_jpeg_matrix_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "metadata.json").write_text(json.dumps({"schema": 1, "rounds": 5, "case_count": 20}))
            (root / "summary.csv").write_text("operation,case\nencode,only-one-case\n")
            with self.assertRaisesRegex(ValueError, "incomplete.*matrix"):
                project_jpeg(root)

    def test_downloaded_snapshot_requires_matching_workflow_revision(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "documentation.json").write_text(json.dumps({"repository": "owner/repo", "benchmark": {}}))
            (root / "snapshot.json").write_text(json.dumps({"revision": "a" * 40}))
            with patch("docs_evidence.validate"), patch.dict("os.environ", {
                "DOCS_BENCHMARK_SNAPSHOT": "snapshot.json", "DOCS_BENCHMARK_REVISION": "b" * 40,
            }):
                with self.assertRaisesRegex(ValueError, "workflow source revision"):
                    read_config(root)

    def test_missing_and_invalid_measurements_never_become_zero(self) -> None:
        self.assertIsNone(number(None))
        self.assertEqual(cell(None), "Not measured")
        for value in (float("nan"), float("inf"), -1, True, "12"):
            with self.assertRaises(ValueError):
                number(value)

    def test_failed_gpu_and_fallback_rows_remain_explicit(self) -> None:
        source = {
            "schema": "migration-parity/benchmark-result@1",
            "identity": {"targets": [{"revision": "a" * 40, "dirty": False}],
                         "finished_at": "2026-09-16T00:00:00Z", "run_id": "run"},
            "status": "failed", "environment": {"os": "test", "machine_id": "private-host"},
            "workloads": [{"workload_id": "resize", "measurement_policy": {"boundary": "whole_workflow"},
                           "correctness": {"gate": "source_target_match", "outcome": "not_proven"},
                           "subjects": [{"id": "python-gpu", "status": "failed", "measurements": [],
                                         "execution": {"requested_backend": "gpu", "actual_backend": "cpu", "terminal_complete": False}}]}],
        }
        snapshot = project_pillow(source)
        row = snapshot["rows"][0]
        self.assertEqual(row["status"], "failed")
        self.assertIsNone(row["median_us"])
        self.assertEqual(row["actual_backend"], "cpu")
        self.assertFalse(row["terminal_complete"])
        self.assertNotIn("private-host", json.dumps(snapshot))
        snapshot.update(schema=SCHEMA, repository="owner/repo", source_sha256="b" * 64)
        validate(snapshot, "owner/repo")
        with self.assertRaises(ValueError):
            validate(snapshot, "other/repo")
        duplicate = copy.deepcopy(snapshot)
        duplicate["rows"].append(duplicate["rows"][0])
        with self.assertRaises(ValueError):
            validate(duplicate, "owner/repo")


if __name__ == "__main__":
    unittest.main()

"""Regression guards for public documentation and benchmark presentation."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from docs_evidence import SCHEMA, cell, contract_code, number, project_jpeg, project_pillow, validate
from docs_site import HtmlContracts, anchors, check_api_contracts, prepare_output, read_config, rewrite_links
from report_pipeline_roadmap_status import ROADMAP, build_report
from report_pipeline_benchmark_coverage import DEFAULT_INPUT, report as pipeline_report


class DocumentationTests(unittest.TestCase):
    def test_contract_code_preserves_quotes_unions_arrows_and_literal_markup(self) -> None:
        signature = "__call__(mode: 'str | None' = None, note='<tag>&`', *args, **kwargs) -> 'Image'\n# next line"
        parser = HtmlContracts()
        parser.feed(contract_code("PIL.Image.convert", signature))
        self.assertEqual(parser.contracts, {"PIL.Image.convert": signature})

    def test_rendered_contract_gate_rejects_double_escaping_or_missing_code(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            signature = "convert(mode: 'str | None') -> 'Image'"
            manifest = {"surfaces": [{"id": "PIL.Image", "operations": [{"source": {"path": "PIL.Image.convert", "signature": signature}}]}]}
            (root / "manifest.json").write_text(json.dumps(manifest))
            config = {"support": {"source": "manifest.json"}}
            block = contract_code("PIL.Image.convert", signature)
            valid = '<h2 id="pilimage">PIL.Image</h2>' + block
            check_api_contracts(root, config, valid)
            with self.assertRaisesRegex(ValueError, "text differs"):
                check_api_contracts(root, config, valid.replace("&gt;", "&amp;gt;"))
            with self.assertRaisesRegex(ValueError, "lacks a code element"):
                check_api_contracts(root, config, valid.replace("<code>", "").replace("</code>", ""))
            with self.assertRaisesRegex(ValueError, "inventory differs"):
                check_api_contracts(root, config, "<p>No contracts</p>")
            with self.assertRaisesRegex(ValueError, "surface headings"):
                check_api_contracts(root, config, '<td>## PIL.Image</td>' + block)

    def test_contract_duplicate_paths_are_rejected(self) -> None:
        parser = HtmlContracts()
        block = contract_code("PIL.Image.convert", "convert()")
        with self.assertRaisesRegex(ValueError, "duplicate"):
            parser.feed(block + block)

    def test_eager_and_point_workloads_remain_required_after_descriptor_removal(self) -> None:
        baseline = pipeline_report(DEFAULT_INPUT)
        self.assertEqual(baseline["operation_variants_total"], 87)
        self.assertEqual(baseline["missing_operation_workloads"], [])
        self.assertEqual(baseline["unexpected_benchmark_specs"], [])
        document = json.loads(DEFAULT_INPUT.read_text())
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "benchmark.json"
            for operation in ("quantize", "pointop", "lineargradient",
                              "radialgradient", "effectmandelbrot"):
                workload_id = f"pipeline-op.{operation}.benchmark-materialized"
                modified = copy.deepcopy(document)
                modified["workloads"] = [item for item in modified["workloads"]
                                         if item["workload_id"] != workload_id]
                path.write_text(json.dumps(modified))
                result = pipeline_report(path)
                self.assertEqual(result["operation_variants_total"], 87)
                self.assertEqual(result["missing_operation_workloads"], [workload_id])
                self.assertLess(result["operation_coverage_percent"], 100.0)

    def test_public_roadmap_preserves_the_complete_index_without_claiming_a_run(self) -> None:
        report = build_report(None)
        self.assertEqual(report["items_total"], 64)
        self.assertEqual(report["missing_ids"], [])
        self.assertEqual(report["unexpected_ids"], [])
        self.assertEqual(report["duplicate_ids"], [])
        self.assertIsNone(report["evidence"]["coverage"])
        self.assertIsNone(report["evidence"]["benchmark_result"])
        self.assertGreater(len(report["open_ids"]), 0)

    def test_roadmap_removal_or_duplicate_cannot_disappear_from_the_report(self) -> None:
        text = ROADMAP.read_text()
        first = next(line for line in text.splitlines() if line.startswith("| FIL-01 |"))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "roadmap.md"
            with patch("report_pipeline_roadmap_status.ROOT", Path(directory)), \
                 patch("report_pipeline_roadmap_status.ROADMAP", path):
                path.write_text(text.replace(first + "\n", ""))
                self.assertEqual(build_report(None)["missing_ids"], ["FIL-01"])
                path.write_text(text + first + "\n")
                self.assertEqual(build_report(None)["duplicate_ids"], ["FIL-01"])

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

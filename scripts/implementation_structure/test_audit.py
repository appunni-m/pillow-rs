"""Contracts for the changed-path implementation-structure audit."""

from __future__ import annotations

import io
import json
import unittest
import tempfile
import subprocess
from contextlib import redirect_stdout
from pathlib import Path

from scripts.implementation_structure.audit import (
    build_report,
    classify_path,
    main as audit_main,
    maintained_tree_paths,
)


class PathClassificationTests(unittest.TestCase):
    def test_nested_evidence_has_family_and_experiment(self) -> None:
        result = classify_path("docs/evidence/filter/rgb-blur-simd-x86-20261019/README.md")
        self.assertEqual(result["assessment"], "matches_family_experiment_shape")

    def test_flat_evidence_is_review_only(self) -> None:
        result = classify_path("docs/evidence/filter-rgb-blur-simd-x86-20261019/README.md")
        self.assertEqual(result["assessment"], "review_candidate")
        self.assertEqual(result["rule"], "flat_evidence_bundle")
        whole_tree = classify_path(
            "docs/evidence/filter-rgb-blur-simd-x86-20261019/README.md",
            exhaustive=True,
        )
        self.assertEqual(whole_tree["assessment"], "documented_migration_debt")

    def test_fixture_payload_requires_a_domain(self) -> None:
        self.assertEqual(
            classify_path("pillow-rs/tests/fixtures/inputs/parity/case.json")["assessment"],
            "matches_domain_input_shape",
        )
        self.assertEqual(
            classify_path("pillow-rs/tests/fixtures/inputs/case.json")["rule"],
            "fixture_domain_missing",
        )

    def test_adapter_root_architecture_kernel_is_flagged_without_move(self) -> None:
        result = classify_path(
            "pillow-rs/src/compute/pool_simd/ops/adapters/transpose_avx2.rs"
        )
        self.assertEqual(result["rule"], "architecture_kernel_at_adapter_root")
        self.assertIn("kernels/", result["expected_shape"])

    def test_nested_kernel_is_recognized(self) -> None:
        result = classify_path(
            "pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs"
        )
        self.assertEqual(result["assessment"], "matches_family_kernel_shape")

    def test_family_kernel_module_entry_is_recognized(self) -> None:
        result = classify_path(
            "pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels.rs"
        )
        self.assertEqual(result["assessment"], "matches_family_kernel_shape")
        self.assertEqual(result["rule"], "simd_kernel_dispatch")

    def test_flat_shader_is_a_candidate_not_a_deletion(self) -> None:
        result = classify_path("pillow-rs/src/compute/pool_gpu/shaders/transpose.wgsl")
        self.assertEqual(result["rule"], "flat_gpu_shader")
        self.assertEqual(result["assessment"], "review_candidate")

    def test_shared_perf_ledger_is_a_supported_root_role(self) -> None:
        result = classify_path("docs/evidence/performance-optimization-local-runs.csv")
        self.assertEqual(result["rule"], "shared_evidence_ledger")

    def test_rust_size_baseline_is_quality_tool_configuration(self) -> None:
        result = classify_path("scripts/rust_size_baseline.json")
        self.assertEqual(result["assessment"], "supported_tool_configuration")
        self.assertEqual(result["rule"], "rust_function_size_ratchet_baseline")

    def test_workflow_test_importing_tool_package_is_reviewed_for_colocation(self) -> None:
        result = classify_path(
            "scripts/test_operation_source_crosswalk.py",
            "from source_crosswalk.builder import build_crosswalk\n",
        )
        self.assertEqual(result["rule"], "workflow_test_separate_from_tool_package")

    def test_workflow_contract_tests_live_in_the_tool_package(self) -> None:
        result = classify_path(
            "scripts/source_crosswalk/test_operation_source_crosswalk.py"
        )
        self.assertEqual(result["assessment"], "matches_tool_concern_shape")

    def test_core_cpu_and_gpu_files_follow_the_family_grammar(self) -> None:
        self.assertEqual(
            classify_path("pillow-rs/src/ops/resize.rs")["assessment"],
            "matches_core_operation_shape",
        )
        self.assertEqual(
            classify_path("pillow-rs/src/compute/pool_cpu/ops/resize.rs")["assessment"],
            "matches_cpu_family_shape",
        )
        self.assertEqual(
            classify_path("pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/l_avx2.rs")["assessment"],
            "matches_family_kernel_shape",
        )
        self.assertEqual(
            classify_path("pillow-rs/src/compute/pool_gpu/shaders/resize/nearest.wgsl")["assessment"],
            "matches_family_shader_shape",
        )

    def test_known_multi_family_aggregators_remain_visible_as_migration_debt(self) -> None:
        paths = (
            "pillow-rs/src/ops/imageops.rs",
            "pillow-rs/src/compute/pool_cpu/ops/color.rs",
            "pillow-rs/src/compute/pool_cpu/ops/effects.rs",
            "pillow-rs/src/compute/pool_cpu/ops/geometry.rs",
            "pillow-rs/src/compute/pool_cpu/ops/imageops.rs",
            "pillow-rs/src/compute/pool_cpu/ops/filter.rs",
            "pillow-rs/src/compute/pool_simd/ops/adapters.rs",
        )
        for path in paths:
            with self.subTest(path=path):
                changed = classify_path(path)
                self.assertEqual(changed["assessment"], "review_candidate")
                self.assertEqual(changed["rule"], "documented_multi_family_aggregator")
                self.assertTrue(changed["expected_shape"])
                tree = classify_path(path, exhaustive=True)
                self.assertEqual(tree["assessment"], "documented_migration_debt")
                self.assertEqual(tree["rule"], "documented_multi_family_aggregator")

    def test_gpu_root_is_a_supported_coordinator(self) -> None:
        result = classify_path("pillow-rs/src/compute/pool_gpu/mod.rs")
        self.assertEqual(result["assessment"], "supported_backend_support_role")
        self.assertEqual(result["rule"], "gpu_pool_coordinator")

    def test_cpu_registration_and_test_files_are_not_family_implementations(self) -> None:
        self.assertEqual(
            classify_path("pillow-rs/src/compute/pool_cpu/ops/mod.rs")["rule"],
            "cpu_operation_registration",
        )
        tests = classify_path(
            "pillow-rs/src/compute/pool_cpu/ops/color_convert_allocation_tests.rs"
        )
        self.assertEqual(tests["assessment"], "review_candidate")
        self.assertEqual(tests["rule"], "cpu_test_module_at_family_root")
        helper = classify_path(
            "pillow-rs/src/compute/pool_cpu/ops/filter/rgb_blur_ring.rs",
            exhaustive=True,
        )
        self.assertEqual(helper["rule"], "cpu_filter_helper_outside_kernel_layer")
        self.assertEqual(helper["assessment"], "documented_migration_debt")

    def test_examples_and_embedded_resources_follow_owner_domains(self) -> None:
        self.assertEqual(
            classify_path("pillow-rs/examples/pipeline_layers.rs")["assessment"],
            "matches_example_entrypoint_shape",
        )
        self.assertEqual(
            classify_path("pillow-rs/src/font/default_aileron.ttf")["assessment"],
            "matches_domain_resource_shape",
        )
        self.assertEqual(
            classify_path("pillow-rs/src/ops/lut_overlay.bin")["assessment"],
            "matches_operation_resource_shape",
        )

    def test_whole_tree_mode_keeps_unknown_owner_path_explicitly_unclassified(self) -> None:
        result = classify_path("tools/bench-report/src/main.rs", exhaustive=True)
        self.assertEqual(result["assessment"], "unclassified")
        self.assertEqual(result["rule"], "layout_rule_unclassified")

    def test_tree_inventory_includes_tracked_and_untracked_maintained_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(
                ["git", "-c", "user.name=Test", "-c", "user.email=test@example.com",
                 "commit", "--allow-empty", "-qm", "initial"],
                cwd=root,
                check=True,
            )
            tracked = root / "docs" / "ARCHITECTURE.md"
            tracked.parent.mkdir(parents=True)
            tracked.write_text("# Architecture\n", encoding="utf-8")
            subprocess.run(["git", "add", "docs/ARCHITECTURE.md"], cwd=root, check=True)
            subprocess.run(
                [
                    "git",
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "-qm",
                    "add guide",
                ],
                cwd=root,
                check=True,
            )
            moved_from = (
                root / "pillow-rs" / "src" / "compute" / "pool_simd" / "ops"
                / "adapters" / "box_blur_luma.rs"
            )
            moved_from.parent.mkdir(parents=True)
            moved_from.write_text("mod kernel {}\n", encoding="utf-8")
            subprocess.run(["git", "add", str(moved_from.relative_to(root))], cwd=root, check=True)
            subprocess.run(
                ["git", "-c", "user.name=Test", "-c", "user.email=test@example.com",
                 "commit", "-qm", "add old kernel path"],
                cwd=root,
                check=True,
            )
            moved_to = (
                root / "pillow-rs" / "src" / "compute" / "pool_simd" / "ops"
                / "adapters" / "box_blur" / "kernels" / "luma.rs"
            )
            moved_from.unlink()
            moved_to.parent.mkdir(parents=True)
            moved_to.write_text("mod kernel {}\n", encoding="utf-8")
            untracked = root / "pillow-rs" / "src" / "ops" / "resize.rs"
            untracked.parent.mkdir(parents=True)
            untracked.write_text("", encoding="utf-8")

            paths = maintained_tree_paths(root)
            self.assertEqual(
                paths,
                [
                    {"path": "docs/ARCHITECTURE.md", "change": "tracked"},
                    {
                        "path": "pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/luma.rs",
                        "change": "untracked",
                    },
                    {"path": "pillow-rs/src/ops/resize.rs", "change": "untracked"},
                ],
            )
            report = build_report(root, scope="tree")
            self.assertEqual(
                report["scope"],
                "all current tracked and non-ignored untracked maintained paths",
            )
            self.assertEqual(report["summary"]["paths"], 3)

    def test_unclassified_gate_writes_report_before_failing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(
                ["git", "-c", "user.name=Test", "-c", "user.email=test@example.com",
                 "commit", "--allow-empty", "-qm", "initial"],
                cwd=root,
                check=True,
            )
            unclassified = root / "tools" / "bench-report" / "src" / "main.rs"
            unclassified.parent.mkdir(parents=True)
            unclassified.write_text("fn main() {}\n", encoding="utf-8")
            output = io.StringIO()
            with redirect_stdout(output):
                status = audit_main([
                    "--root", str(root),
                    "--scope", "tree",
                    "--output", "target/structure.json",
                    "--fail-on-unclassified",
                ])

            report_path = root / "target/structure.json"
            self.assertEqual(status, 1)
            self.assertTrue(report_path.is_file())
            report = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["summary"]["unclassified_paths"], 1)
            self.assertEqual(report["items"][0]["path"], "tools/bench-report/src/main.rs")


if __name__ == "__main__":
    unittest.main()

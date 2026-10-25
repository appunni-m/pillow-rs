"""Regression tests for the repository backend-boundary checker."""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from check_backend_boundaries import (
    SIMD_ADAPTER_FACADE,
    active_feature_tokens,
    parallel_module_gate_violations,
    rayon_violations,
    simd_facade_violations,
)
from run_migration_benchmark import runtime_backend_for_profile


ROOT = Path(__file__).resolve().parents[1]


class SimdFacadeBoundaryTests(unittest.TestCase):
    def test_current_facade_contains_only_dispatch_functions(self) -> None:
        source = (ROOT / SIMD_ADAPTER_FACADE).read_text(encoding="utf-8")
        self.assertEqual(simd_facade_violations(source), [])

    def test_family_helper_added_to_facade_is_rejected(self) -> None:
        source = (ROOT / SIMD_ADAPTER_FACADE).read_text(encoding="utf-8")
        source += "\nfn native_rank_filter_supported_for_image() -> bool { true }\n"

        errors = simd_facade_violations(source)

        self.assertTrue(any("native_rank_filter_supported_for_image" in error for error in errors))
        self.assertTrue(any("adapters/<family>.rs" in error for error in errors))

    def test_inline_image_admission_is_rejected(self) -> None:
        source = (ROOT / SIMD_ADAPTER_FACADE).read_text(encoding="utf-8")
        source += "\nfn simd_supports_for_image(img: &Image) -> bool { img.width() > 0 }\n"

        errors = simd_facade_violations(source)

        self.assertTrue(any("image and shape admission belongs" in error for error in errors))

    def test_parallel_default_aliases_are_visible(self) -> None:
        package = {"features": {"default": ["parallel"], "parallel": []}}

        self.assertIn("parallel", active_feature_tokens(package))

    def test_rayon_is_rejected_outside_parallel_cpu_module(self) -> None:
        with TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            gpu_source = root / "pillow-rs/src/compute/pool_gpu/gpu.rs"
            gpu_source.parent.mkdir(parents=True)
            gpu_source.write_text("rayon::join(|| (), || ());\n", encoding="utf-8")

            errors = rayon_violations(root)

        self.assertEqual(len(errors), 1)
        self.assertIn("gpu.rs:1", errors[0])
        self.assertIn("opt-in Parallel CPU module", errors[0])

    def test_rayon_owner_files_remain_allowed(self) -> None:
        with TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            library = root / "pillow-rs/src/lib.rs"
            library.parent.mkdir(parents=True)
            library.write_text(
                '#[cfg(feature = "parallel")]\nuse rayon as _;\n', encoding="utf-8"
            )
            parallel_module = root / "pillow-rs/src/par.rs"
            parallel_module.write_text("use rayon::iter::ParallelIterator;\n", encoding="utf-8")

            self.assertEqual(rayon_violations(root), [])

    def test_rayon_use_in_crate_root_must_be_feature_gated(self) -> None:
        with TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            library = root / "pillow-rs/src/lib.rs"
            library.parent.mkdir(parents=True)
            library.write_text("use rayon as _;\n", encoding="utf-8")

            errors = rayon_violations(root)

        self.assertEqual(len(errors), 1)
        self.assertIn("opt-in Parallel CPU module", errors[0])

    def test_parallel_module_declaration_must_use_opt_in_feature(self) -> None:
        with TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            library = root / "pillow-rs/src/lib.rs"
            library.parent.mkdir(parents=True)
            library.write_text("/// Parallel helpers.\nmod par;\n", encoding="utf-8")

            errors = parallel_module_gate_violations(root)

        self.assertEqual(len(errors), 1)
        self.assertIn("mod par;` must use", errors[0])

    def test_rayon_profile_is_measured_as_parallel_cpu(self) -> None:
        parallel_profile = "python-parallel-cpu"
        self.assertEqual(runtime_backend_for_profile(parallel_profile), "cpu")
        self.assertNotEqual(runtime_backend_for_profile(parallel_profile), "simd")


if __name__ == "__main__":
    unittest.main()

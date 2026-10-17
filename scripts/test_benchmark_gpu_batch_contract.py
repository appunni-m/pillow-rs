import unittest
import hashlib
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace

from benchmark_gpu_batch import (
    DEFAULT_BATCH_SIZES,
    batch_workload_id,
    default_batch_modes,
    extension_identity,
    expected_workload_ids,
    resolve_selection,
    validate_queued_capacity,
)


class GpuBatchBenchmarkContractTests(unittest.TestCase):
    def test_singular_selectors_narrow_the_emitted_cohort(self):
        modes, sizes = resolve_selection(
            "boxblur", None, "L", None, 1024, None
        )

        self.assertEqual(modes, ("L",))
        self.assertEqual(sizes, (1024,))
        self.assertEqual(
            expected_workload_ids("boxblur", modes, sizes, 768, "uniform"),
            {"pipeline-batch.boxblur.l-1024x768"},
        )
        self.assertEqual(
            batch_workload_id("boxblur", modes[0], sizes[0], 768, "uniform"),
            "pipeline-batch.boxblur.l-1024x768",
        )

    def test_omitted_selectors_preserve_the_default_matrix(self):
        modes, sizes = resolve_selection(
            "boxblur", None, None, None, None, None
        )

        self.assertEqual(modes, ("L", "RGB"))
        self.assertEqual(sizes, DEFAULT_BATCH_SIZES)
        self.assertEqual(
            expected_workload_ids("boxblur", modes, sizes, None, "uniform"),
            {
                "pipeline-batch.boxblur.l-256x256",
                "pipeline-batch.boxblur.l-1024x1024",
                "pipeline-batch.boxblur.rgb-256x256",
                "pipeline-batch.boxblur.rgb-1024x1024",
            },
        )

    def test_operation_specific_modes_remain_unchanged(self):
        self.assertEqual(default_batch_modes("filter"), ("L", "LA"))
        self.assertEqual(default_batch_modes("merge"), ("RGB",))
        self.assertEqual(default_batch_modes("resize"), ("F",))
        self.assertEqual(default_batch_modes("alpha-composite-mirror"), ("RGBA",))
        self.assertEqual(
            expected_workload_ids("alpha-composite-mirror", ("RGBA",), (256,), None, "noise"),
            {"pipeline-batch.alpha-composite-mirror.rgba-256x256.noise"},
        )

    def test_extension_identity_hashes_the_loaded_module_file(self):
        with TemporaryDirectory() as directory:
            path = Path(directory) / "_core.abi3.so"
            payload = b"focused extension identity fixture"
            path.write_bytes(payload)

            self.assertEqual(
                extension_identity(SimpleNamespace(__file__=str(path))),
                {
                    "path": str(path.resolve()),
                    "sha256": hashlib.sha256(payload).hexdigest(),
                },
            )

    def test_conflicting_singular_and_plural_selectors_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "either --mode or --modes"):
            resolve_selection("boxblur", None, "L", ["L", "RGB"], None, None)
        with self.assertRaisesRegex(ValueError, "either --size or --sizes"):
            resolve_selection("boxblur", None, None, None, 1024, [256, 1024])
        with self.assertRaisesRegex(ValueError, "cohort selectors"):
            resolve_selection("boxblur", "GPU queued", None, ["L"], None, None)

    def test_subject_mode_retains_its_single_workload_defaults(self):
        modes, sizes = resolve_selection(
            "boxblur", "GPU queued", None, None, None, None
        )

        self.assertEqual(modes, ("L",))
        self.assertEqual(sizes, (256,))

    def test_queued_throughput_requires_capacity_to_form_a_batch(self):
        with self.assertRaisesRegex(ValueError, "at least 2 images"):
            validate_queued_capacity(("GPU queued",), 1, 16, 4)
        with self.assertRaisesRegex(ValueError, "--max-jobs >= 2"):
            validate_queued_capacity(("GPU queued",), 32, 1, 4)
        with self.assertRaisesRegex(ValueError, "--max-in-flight >= 2"):
            validate_queued_capacity(("GPU queued",), 32, 16, 1)

        validate_queued_capacity(("GPU queued",), 2, 2, 2)
        validate_queued_capacity(("GPU eager",), 1, 1, 1)


if __name__ == "__main__":
    unittest.main()

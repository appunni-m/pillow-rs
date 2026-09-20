"""Regression guards for truthful native coverage evidence."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "pillow-rs-py/python"))
import run_migration_font_native_cases as runner


def case(operation="getmask", **params):
    return {"case_id": "probe", "operation": operation,
            "inputs": {"assets": {"font": {"kind": "load_default"}}, "params": params}}


class NativeCoverageTests(unittest.TestCase):
    def test_errors_are_observations_but_harness_exceptions_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "cases.json").write_text(json.dumps({"cases": [case()]}))
            with patch.object(runner, "FONT_NATIVE_ROOT", path), patch.object(runner, "FIXTURE_ROOT", path), patch.object(runner, "require_target"):
                for error, status in [(runner.ApiRaised("ValueError", "bad size", "truetype"), "raised"),
                                      (AttributeError("missing method"), "failed"),
                                      (TypeError("bad harness adaptation"), "failed")]:
                    with self.subTest(error=error), patch.object(runner, "run_case", side_effect=error):
                        report = runner.run_native_cases()
                        self.assertEqual(report[status], 1)
                        self.assertFalse(report["parity_verified"])
                        self.assertNotIn("passed", report)

    def test_getmask_keeps_its_method_and_native_start_tuple(self):
        font = Mock()
        font.getmask.return_value = b"pixels"
        with patch.object(runner, "load_font", return_value=font):
            runner.run_case(case(text="AV", start=[0.25, 0.5], mode="L"))
        font.getmask.assert_called_once_with("AV", mode="L", start=(0.25, 0.5))
        font.getmask2.assert_not_called()

    def test_positional_getmask2_arguments_are_not_overridden_by_defaults(self):
        font = Mock()
        font.getmask2.return_value = (b"pixels", (0, 0))
        args = ["L", None, None, None, 0, None, 123, None, "ignored"]
        with patch.object(runner, "load_font", return_value=font):
            runner.run_case(case("getmask2", text="AV", args=args, kwargs={"unknown": 1}))
        font.getmask2.assert_called_once_with("AV", *args, unknown=1)

    def test_variants_and_repeated_byte_names_reach_the_api(self):
        font = Mock()
        font.font_variant.return_value = None
        font.getbbox.return_value = (0, 0, 10, 10)
        variant = case("font_variant", size=20, variant_size=22, variant_index=1)
        variant["inputs"]["assets"]["variant_font"] = {"id": "input/fonts/DejaVuSans.ttf"}
        with patch.object(runner, "load_font", return_value=font):
            runner.run_case(variant)
            runner.run_case(case("set_variation_by_name", name_bytes_hex="5468696e", repeat_count=2))
        font.font_variant.assert_called_once_with(size=22, index=1, font=str(runner.ASSETS / "font/fonts/DejaVuSans.ttf"))
        self.assertEqual([c.args for c in font.set_variation_by_name.call_args_list], [(b"Thin",), (b"Thin",)])

    def test_unknown_operations_and_parameters_fail_before_loading(self):
        with patch.object(runner, "load_font") as load:
            for probe in [case("typo"), case("getmask", misspelled_mode="L")]:
                with self.assertRaisesRegex(ValueError, "unknown|unhandled"):
                    runner.run_case(probe)
            load.assert_not_called()

    def test_duplicate_id_cannot_overwrite_an_observation(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "cases.json").write_text(json.dumps({"cases": [case(), case()]}))
            with patch.object(runner, "FONT_NATIVE_ROOT", path), patch.object(runner, "FIXTURE_ROOT", path), patch.object(runner, "require_target"), \
                 patch.object(runner, "run_case", return_value=None):
                with self.assertRaisesRegex(ValueError, "duplicate font case ID"):
                    runner.run_native_cases()

    def test_oracle_import_cannot_be_used_as_target_coverage(self):
        import PIL
        with patch.object(PIL, "__file__", "/unrelated/site-packages/PIL/__init__.py"):
            with self.assertRaisesRegex(RuntimeError, "checkout PIL"):
                runner.require_target()

    def test_missing_rust_driver_is_an_infrastructure_error(self):
        with patch.dict("os.environ", {"MIGRATION_FONT_NATIVE_DRIVER": "/nonexistent/font-driver"}):
            with self.assertRaises(FileNotFoundError):
                runner.run_case(case("text_bbox", text="Hello"))


if __name__ == "__main__":
    unittest.main()

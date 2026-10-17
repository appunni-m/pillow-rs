import unittest
from pathlib import Path
from types import SimpleNamespace

from scripts import run_migration_parity
from scripts import validate_migration_parity_contract as contract
from scripts.migration_parity.display import (
    build_capture_viewer,
    isolated_viewer_registry,
)


class FakeImage:
    def __init__(self, mode, size, pixels, info=None):
        self.mode = mode
        self.size = size
        self._pixels = pixels
        self.info = {} if info is None else info

    def tobytes(self):
        return self._pixels

    def convert(self, mode):
        # Deliberately drop metadata like one target conversion path does.
        return FakeImage(mode, self.size, b"converted pixels", info={})


class FakeViewer:
    format = None
    launched = 0

    def show(self, image, **options):
        if not (
            image.mode in ("1", "RGBA")
            or (self.format == "PNG" and image.mode in ("I;16", "LA"))
        ):
            base_mode = {"LA": "L", "RGBA": "RGB"}.get(image.mode, image.mode)
            if image.mode != base_mode:
                image = image.convert(base_mode)
        return self.show_image(image, **options)

    def show_image(self, image, **options):
        return self.show_file("unused", **options)

    def show_file(self, path, **options):
        FakeViewer.launched += 1
        return 1


class FakeImageShow:
    Viewer = FakeViewer

    def __init__(self, viewers=None):
        self._viewers = [] if viewers is None else viewers

    def register(self, viewer, order=1):
        if isinstance(viewer, type) and issubclass(viewer, self.Viewer):
            viewer = viewer()
        if order > 0:
            self._viewers.append(viewer)
        else:
            self._viewers.insert(0, viewer)

    def show(self, image, title=None, **options):
        for viewer in self._viewers:
            if viewer.show(image, title=title, **options):
                return True
        return False


def capture_spec(**overrides):
    spec = {
        "protocol": "capture-viewer",
        "viewer_id": "capture-one",
        "image_step_id": "image",
        "format": None,
        "accept": True,
        "registration": "class",
        "hook": "show",
    }
    spec.update(overrides)
    return spec


def display_case():
    return {
        "steps": [
            {
                "surface": "PIL.ImageShow",
                "operation": "register",
                "receiver": None,
                "arguments": {},
            }
        ]
    }


class MigrationParityDisplayTests(unittest.TestCase):
    def setUp(self):
        FakeViewer.launched = 0

    def build_capture(self, image, module=None, **overrides):
        module = FakeImageShow() if module is None else module
        capture = build_capture_viewer(
            capture_spec(**overrides),
            side="target",
            bindings={"image": image},
            import_surface=lambda side, surface: module,
        )
        return module, capture

    def test_class_hook_captures_exact_raw_bytes_and_options_without_launch(self):
        events = []
        image = FakeImage(
            "RGB", (2, 1), b"\x00\x7f\xff\x10", {"display_callbacks": events}
        )
        platform_viewer = FakeViewer()
        original_registry = [platform_viewer]
        module = FakeImageShow(original_registry)
        module, capture_class = self.build_capture(
            image,
            module=module,
            registration="class",
            hook="show",
            accept=False,
        )

        with isolated_viewer_registry(
            "target", display_case(), lambda side, surface: module
        ):
            self.assertEqual(module._viewers, [])
            module.register(capture_class, order=0)
            self.assertFalse(module.show(image, title="raw view", fit="contain"))

        self.assertIs(module._viewers, original_registry)
        self.assertEqual(module._viewers, [platform_viewer])

        self.assertEqual(
            events,
            [
                {
                    "viewer_id": "capture-one",
                    "title": "raw view",
                    "mode": "RGB",
                    "size": (2, 1),
                    "pixels": b"\x00\x7f\xff\x10",
                    "options": {"title": "raw view", "fit": "contain"},
                }
            ],
        )
        self.assertEqual(FakeViewer.launched, 0)

    def test_png_format_keeps_la_mode_before_capture(self):
        events = []
        image = FakeImage("LA", (1, 1), b"\x80\x40", {"display_callbacks": events})
        module, capture_instance = self.build_capture(
            image,
            registration="instance",
            hook="show_image",
            format="PNG",
        )

        module.register(capture_instance, order=0)
        self.assertTrue(module.show(image, title="png view"))

        self.assertEqual(events[0]["mode"], "LA")
        self.assertEqual(events[0]["pixels"], b"\x80\x40")
        self.assertEqual(FakeViewer.launched, 0)

    def test_instance_show_image_hook_records_converted_image_in_original_sink(self):
        events = []
        image = FakeImage("LA", (1, 1), b"\x80\x40", {"display_callbacks": events})
        module, capture_instance = self.build_capture(
            image,
            registration="instance",
            hook="show_image",
            accept=1,
        )

        module.register(capture_instance, order=0)
        self.assertTrue(module.show(image, title="converted view"))

        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]["viewer_id"], "capture-one")
        self.assertEqual(events[0]["title"], "converted view")
        self.assertEqual(events[0]["mode"], "L")
        self.assertEqual(events[0]["size"], (1, 1))
        self.assertEqual(events[0]["pixels"], b"converted pixels")
        self.assertIs(image.info["display_callbacks"], events)
        self.assertEqual(FakeViewer.launched, 0)

    def test_registry_is_restored_after_workflow_exception(self):
        original_viewer = object()
        original_registry = [original_viewer]
        module = FakeImageShow(original_registry)

        with self.assertRaisesRegex(RuntimeError, "workflow failed"):
            with isolated_viewer_registry(
                "target", display_case(), lambda side, surface: module
            ):
                self.assertEqual(module._viewers, [])
                module._viewers.append(object())
                raise RuntimeError("workflow failed")

        self.assertIs(module._viewers, original_registry)
        self.assertEqual(module._viewers, [original_viewer])

    def test_registry_must_be_an_existing_list(self):
        for registry in (None, (), "not-a-list"):
            with self.subTest(registry=registry):
                module = SimpleNamespace(Viewer=FakeViewer, _viewers=registry)
                with self.assertRaisesRegex(
                    RuntimeError, "cannot safely isolate display workflow"
                ):
                    with isolated_viewer_registry(
                        "target", display_case(), lambda side, surface: module
                    ):
                        self.fail("unsafe registry state must not enter workflow")

        module_without_registry = SimpleNamespace(Viewer=FakeViewer)
        with self.assertRaisesRegex(
            RuntimeError, "cannot safely isolate display workflow"
        ):
            with isolated_viewer_registry(
                "target",
                display_case(),
                lambda side, surface: module_without_registry,
            ):
                self.fail("missing registry must not enter workflow")

    def test_missing_public_module_is_left_for_public_operation_error(self):
        def missing_module(side, surface):
            raise ImportError("public ImageShow is unavailable")

        with isolated_viewer_registry("target", display_case(), missing_module):
            pass

    def test_non_display_workflow_does_not_import_imageshow(self):
        case = {
            "steps": [
                {
                    "surface": "PIL.Image",
                    "operation": "new",
                    "receiver": None,
                    "arguments": {},
                }
            ]
        }

        def unexpected_import(side, surface):
            self.fail("non-display workflows must not import ImageShow")

        with isolated_viewer_registry("target", case, unexpected_import):
            pass

    def test_parity_document_checks_capture_references_in_receivers_and_arguments(
        self,
    ):
        operation_index = {
            ("Test.Image", "new"): {
                "source": {"parameters": [], "result": {"shape": "image"}}
            },
            ("Test.Handle", "other"): {
                "source": {"parameters": [], "result": {"shape": "handle"}}
            },
            ("Test.Operation", "run"): {
                "source": {
                    "parameters": [
                        {
                            "id": "viewer",
                            "style": "positional",
                            "value_types": ["handle"],
                            "omission": {"kind": "literal", "value": None},
                        }
                    ],
                    "result": {"shape": "scalar"},
                }
            },
        }
        requirement_index = {
            "req": {"lanes": ["parity"], "target_profiles": ["python-cpu"]}
        }

        invalid_references = (
            ("unknown", False, False, "earlier workflow step"),
            ("future", False, True, "earlier workflow step"),
            ("run", False, False, "earlier workflow step"),
            ("other", True, False, "earlier image result"),
        )
        for placement in ("receiver", "arguments"):
            for (
                image_step_id,
                add_other_step,
                add_future_step,
                message,
            ) in invalid_references:
                with self.subTest(
                    placement=placement,
                    image_step_id=image_step_id,
                    add_other_step=add_other_step,
                    add_future_step=add_future_step,
                ):
                    steps = [
                        {
                            "step_id": "image",
                            "surface": "Test.Image",
                            "operation": "new",
                            "receiver": None,
                            "arguments": {},
                        }
                    ]
                    if add_other_step:
                        steps.append(
                            {
                                "step_id": "other",
                                "surface": "Test.Handle",
                                "operation": "other",
                                "receiver": None,
                                "arguments": {},
                            }
                        )
                    descriptor = {
                        "kind": "literal",
                        "value": capture_spec(image_step_id=image_step_id),
                    }
                    steps.append(
                        {
                            "step_id": "run",
                            "surface": "Test.Operation",
                            "operation": "run",
                            "receiver": descriptor if placement == "receiver" else None,
                            "arguments": (
                                {"viewer": descriptor}
                                if placement == "arguments"
                                else {}
                            ),
                        }
                    )
                    if add_future_step:
                        steps.append(
                            {
                                "step_id": "future",
                                "surface": "Test.Image",
                                "operation": "new",
                                "receiver": None,
                                "arguments": {},
                            }
                        )
                    document = {
                        "schema": "migration-parity/parity-input@1",
                        "cases": [
                            {
                                "case_id": "capture-reference",
                                "surface": "Test.Operation",
                                "operation": "run",
                                "covers": ["req"],
                                "target_profiles": ["python-cpu"],
                                "assets": [],
                                "steps": steps,
                                "observations": ["run"],
                            }
                        ],
                    }
                    with self.assertRaisesRegex(ValueError, message):
                        contract._validate_parity_document(
                            document,
                            "inputs/parity/capture-reference.json",
                            operation_index,
                            requirement_index,
                            {"python-cpu"},
                            Path("."),
                            set(),
                        )

    def test_capture_protocol_is_exact_and_must_be_a_top_level_literal(self):
        for malformed in (
            capture_spec(extra=True),
            {key: value for key, value in capture_spec().items() if key != "hook"},
            capture_spec(registration=[]),
        ):
            with self.subTest(malformed=malformed):
                with self.assertRaises(ValueError):
                    contract._validate_descriptor(
                        {"kind": "literal", "value": malformed},
                        "step.arguments.viewer",
                        set(),
                        {"image"},
                        {"image"},
                    )

        nested = {
            "kind": "literal",
            "value": {"protocol": "list", "items": [capture_spec()]},
        }
        with self.assertRaisesRegex(ValueError, "top-level literal"):
            contract._validate_descriptor(
                nested,
                "step.receiver",
                set(),
                {"image"},
                {"image"},
            )

    def test_capture_reference_is_a_runner_dependency(self):
        descriptor = {"kind": "literal", "value": capture_spec()}
        self.assertEqual(contract._literal_type(descriptor["value"]), "handle")
        self.assertEqual(
            run_migration_parity._descriptor_binding_steps(descriptor), {"image"}
        )


if __name__ == "__main__":
    unittest.main()

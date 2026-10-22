"""Focused contracts for the generated operation-source crosswalk."""

from __future__ import annotations

import unittest
import tempfile
from pathlib import Path

from .python_bindings import Binding, PythonBindingIndex
from .pyo3_bindings import PyO3Index, python_native_calls, struct_field_type
from . import rust_source as crosswalk
from .builder import build_crosswalk, crosswalk_source_identity
from .rust_calls import (
    crate_root_api_routes,
    direct_pipeline_variant_references,
    direct_pipeline_variant_references_at,
    direct_public_api_calls,
    direct_literal_python_method_callbacks,
    top_level_function_definitions,
    direct_receiver_aliases,
    direct_receiver_method_calls,
    inherent_method_definitions,
    resolve_public_api_call,
    rust_family_for_path,
    structure_role,
)
from .rust_modules import build_rust_module_inventory


class RustSourceParserTests(unittest.TestCase):
    def test_matching_delimiter_ignores_nested_comments_and_literals(self) -> None:
        source = 'call("ignored )", /* outer ( /* nested ) */ */ child("x\\\"}")) tail'
        opening = source.index("(")
        closing = crosswalk.matching_delimiter(source, opening, "(", ")")
        self.assertEqual(source[closing + 1 :], " tail")

    def test_masking_preserves_newlines_and_code_after_lifetimes(self) -> None:
        source = (
            "fn borrow<'py>(value: &Bound<'_, PyAny>) {}\n"
            "#[pyfunction]\nfn registered_after_lifetimes() {}\n"
        )
        masked = crosswalk.code_only(source)
        self.assertEqual(masked.count("\n"), source.count("\n"))
        self.assertIn("fn registered_after_lifetimes", masked)

    def test_nonzero_offset_raw_and_escaped_char_literals_keep_scanning_aligned(self) -> None:
        source = (
            "fn first() { let raw = r#\" ) } \"#; let quote = '\\\''; }\n"
            "fn registered_after_literals() {}\n"
        )
        masked = crosswalk.code_only(source)
        self.assertEqual(masked.count("\n"), source.count("\n"))
        self.assertIn("fn registered_after_literals", masked)
        opening = source.index("{")
        closing = crosswalk.matching_delimiter(source, opening, "{", "}")
        self.assertEqual(source[closing + 1 :], "\nfn registered_after_literals() {}\n")

    def test_pipeline_mapping_keeps_aliases_as_separate_edges(self) -> None:
        source = '''
pub enum PipelineOp {
    Resize { width: u32 },
    ResizeBoxed { width: u32 },
    Equalize,
}
fn variant_key(op: &PipelineOp) -> &'static str {
    match op {
        PipelineOp::Resize { .. } | PipelineOp::ResizeBoxed { .. } => "Resize",
        PipelineOp::Equalize => "Equalize",
    }
}
'''
        variants = crosswalk.enum_variants(source)
        edges = crosswalk.variant_key_edges(source)
        self.assertEqual([item["name"] for item in variants], ["Resize", "ResizeBoxed", "Equalize"])
        self.assertEqual(
            [(item["variant"], item["key"]) for item in edges],
            [("Resize", "Resize"), ("ResizeBoxed", "Resize"), ("Equalize", "Equalize")],
        )

    def test_registry_parser_resolves_default_shader_and_draw_macro(self) -> None:
        source = '''
fn register_all(m: &mut Entries) {
    macro_rules! draw_entry {
        ($key:expr, $func:expr) => { m.insert($key, gpu_entry!($func, "draw.wgsl")); };
    }
    m.insert("Invert", gpu_entry!(|img| { execute_invert(img) }, "invert.wgsl"));
    draw_entry!("DrawPoint", |img| { op_draw_point(img) });
    simd_set(m, "Invert", adapters::simd_invert)?;
}
'''
        registrations, simd = crosswalk.parse_registry(source)
        self.assertEqual(
            [(item["key"], item["shader"]) for item in registrations],
            [("Invert", "invert.wgsl"), ("DrawPoint", "draw.wgsl")],
        )
        self.assertEqual([(item["key"], item["symbol"]) for item in simd], [("Invert", "simd_invert")])


class RustFamilyPathTests(unittest.TestCase):
    def test_family_ids_follow_classified_family_roles(self) -> None:
        self.assertEqual(
            rust_family_for_path("pillow-rs/src/ops/resize.rs"), "core:resize"
        )
        self.assertEqual(
            rust_family_for_path("pillow-rs/src/ops/rotate/nearest.rs"), "core:rotate"
        )
        self.assertEqual(
            rust_family_for_path("pillow-rs/src/ops/imageops/geometry.rs"),
            "core:geometry",
        )
        self.assertEqual(
            rust_family_for_path("pillow-rs/src/compute/pool_cpu/ops/grayscale.rs"),
            "cpu:grayscale",
        )
        self.assertEqual(
            rust_family_for_path(
                "pillow-rs/src/compute/pool_cpu/ops/filter/kernels/i32_cross_smooth.rs"
            ),
            "cpu:filter",
        )
        self.assertEqual(
            rust_family_for_path(
                "pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs"
            ),
            "simd:transpose",
        )
        self.assertEqual(
            rust_family_for_path(
                "pillow-rs/src/compute/pool_gpu/shaders/resize/nearest.wgsl"
            ),
            "gpu:resize",
        )

    def test_aggregators_facades_tests_and_flat_shaders_get_no_pseudo_family(self) -> None:
        paths = (
            "pillow-rs/src/ops/imageops.rs",
            "pillow-rs/src/compute/pool_cpu/ops/color.rs",
            "pillow-rs/src/compute/pool_cpu/ops/mod.rs",
            "pillow-rs/src/compute/pool_cpu/ops/color_convert_allocation_tests.rs",
            "pillow-rs/src/compute/pool_simd/ops/adapters.rs",
            "pillow-rs/src/compute/pool_simd/ops/adapters/cmyk_packing_tests.rs",
            "pillow-rs/src/compute/pool_simd/ops/native_channel.rs",
            "pillow-rs/src/compute/pool_gpu/shaders/resize.wgsl",
        )
        for path in paths:
            with self.subTest(path=path):
                self.assertIsNone(rust_family_for_path(path))

    def test_simd_legacy_kernel_names_map_to_their_operation_families(self) -> None:
        cases = {
            "box_blur_luma.rs": "simd:box_blur",
            "gaussian_rgba_uniform.rs": "simd:gaussian_blur",
            "native_put_pixel.rs": "simd:put_pixel",
            "native_thumbnail.rs": "simd:thumbnail",
            "native_transpose_avx2.rs": "simd:transpose",
        }
        for name, family in cases.items():
            path = f"pillow-rs/src/compute/pool_simd/ops/adapters/{name}"
            with self.subTest(path=path):
                self.assertEqual(rust_family_for_path(path), family)
        self.assertEqual(
            rust_family_for_path(
                "pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32.rs"
            ),
            "simd:resize",
        )
        self.assertEqual(
            rust_family_for_path(
                "pillow-rs/src/compute/pool_simd/ops/adapters/native_thumbnail/integer.rs"
            ),
            "simd:thumbnail",
        )

    def test_family_layout_uses_the_structure_classifier_for_aggregator(self) -> None:
        result = structure_role("pillow-rs/src/ops/imageops.rs")
        self.assertEqual(result["assessment"], "documented_migration_debt")
        self.assertEqual(result["rule"], "documented_multi_family_aggregator")

    def test_cpu_registration_uses_imported_operation_symbols_only(self) -> None:
        source = """
fn register_all(m: &mut Entries) {
    use crate::compute::pool_cpu::ops::imageops::{
        op_grayscale as grayscale,
        execute_resize,
    };
    use crate::compute::pool_cpu::ops::grayscale::op_grayscale;
}
"""
        imports = crosswalk.cpu_registration_imports(source)
        self.assertEqual(
            imports,
            {
                "grayscale": "op_grayscale",
                "execute_resize": "execute_resize",
                "op_grayscale": "op_grayscale",
            },
        )
        registration = """
matches!(op, PipelineOp::Grayscale);
image.grayscale(input);
grayscale(image, mode);
execute_resize(image, width, height);
"""
        self.assertEqual(
            crosswalk.cpu_registration_references(registration, imports),
            ["op_grayscale", "execute_resize"],
        )

    def test_cpu_registration_resolves_fully_qualified_operation_paths(self) -> None:
        registration = """
gpu_entry!(|image, op, mode| {
    crate::compute::pool_cpu::ops::grayscale::op_grayscale(image, mode)
});
"""
        self.assertEqual(
            crosswalk.cpu_registration_references(registration, {}),
            ["op_grayscale"],
        )


class CrosswalkSourceIdentityTests(unittest.TestCase):
    def test_source_identity_changes_when_an_input_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "scripts/source_crosswalk/builder.py"
            source.parent.mkdir(parents=True)
            source.write_text("first snapshot", encoding="utf-8")
            js_source = root / "pillow-rs-js/src/lib.rs"
            js_source.parent.mkdir(parents=True)
            js_source.write_text("js source first snapshot", encoding="utf-8")

            first = crosswalk_source_identity(root)
            source.write_text("second snapshot", encoding="utf-8")
            second = crosswalk_source_identity(root)
            js_source.write_text("js source second snapshot", encoding="utf-8")
            third = crosswalk_source_identity(root)

            self.assertEqual(first["crosswalk_input_file_count"], 2)
            self.assertNotEqual(
                first["crosswalk_input_sha256"], second["crosswalk_input_sha256"]
            )
            self.assertNotEqual(
                second["crosswalk_input_sha256"], third["crosswalk_input_sha256"]
            )

    def test_source_identity_includes_the_shared_structure_classifier(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "scripts/source_crosswalk").mkdir(parents=True)
            (root / "scripts/implementation_structure").mkdir(parents=True)
            (root / "docs").mkdir()
            (root / "pillow-rs/tests/fixtures").mkdir(parents=True)
            (root / "pillow-rs/src/compute").mkdir(parents=True)
            required = [
                "pillow-rs/src/compute/registry.rs",
                "pillow-rs/src/pipeline.rs",
                "pillow-rs/src/lib.rs",
                "pillow-rs/tests/fixtures/manifest.yaml",
                "docs/IMPLEMENTATION_STRUCTURE.md",
                "scripts/build_operation_source_crosswalk.py",
                "scripts/source_crosswalk/builder.py",
                "scripts/implementation_structure/audit.py",
            ]
            for relative in required:
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("first snapshot", encoding="utf-8")

            first = crosswalk_source_identity(root)
            classifier = root / "scripts/implementation_structure/audit.py"
            classifier.write_text("second classifier", encoding="utf-8")
            second = crosswalk_source_identity(root)
            self.assertNotEqual(first["crosswalk_input_sha256"], second["crosswalk_input_sha256"])


class RustCoreCallTests(unittest.TestCase):
    def test_direct_public_calls_ignore_comments_strings_and_nested_generic_parens(self) -> None:
        source = (
            "fn wrapper<F: Fn()>(image: &Image) {\n"
            "    // pillow_rs::comment_only()\n"
            "    let text = \"pillow_rs::string_only()\";\n"
            "    pillow_rs::imageops_grayscale(image);\n"
            "}\n"
        )
        calls = direct_public_api_calls(source, source.index("fn wrapper"))
        self.assertEqual(
            [(call["symbol"], call["line"]) for call in calls],
            [("imageops_grayscale", 4)],
        )

    def test_public_reexport_resolves_module_chain_and_keeps_ambiguity_reviewable(self) -> None:
        source = "pub use crate::ops::imageops::grayscale as imageops_grayscale;\n"
        module_edges = [
            {"from": "pillow-rs/src/lib.rs", "module": "ops", "to": "pillow-rs/src/ops/mod.rs", "resolution": "resolved"},
            {"from": "pillow-rs/src/ops/mod.rs", "module": "imageops", "to": "pillow-rs/src/ops/imageops.rs", "resolution": "resolved"},
        ]
        index = crate_root_api_routes(source, module_edges)
        target, status = resolve_public_api_call({"symbol": "imageops_grayscale"}, index)
        self.assertEqual(status, "resolved")
        self.assertEqual(target["item"], "grayscale")
        self.assertEqual(target["implementation_path"], "pillow-rs/src/ops/imageops.rs")

        ambiguous = crate_root_api_routes(
            source,
            module_edges + [
                {"from": "pillow-rs/src/lib.rs", "module": "ops", "to": "pillow-rs/src/ops.rs", "resolution": "resolved"}
            ],
        )
        target, status = resolve_public_api_call({"symbol": "imageops_grayscale"}, ambiguous)
        self.assertIsNone(target)
        self.assertEqual(status, "ambiguous_module_edge")

    def test_public_crate_root_function_is_a_direct_route(self) -> None:
        source = "pub fn imagefont_get_variation_names(font: &Font) {}\n"
        index = crate_root_api_routes(source, [], crate_root="pillow-rs/src/lib.rs")
        target, status = resolve_public_api_call(
            {"symbol": "imagefont_get_variation_names"}, index
        )
        self.assertEqual(status, "resolved")
        self.assertEqual(target["kind"], "public_function_definition")
        self.assertEqual(target["implementation_path"], "pillow-rs/src/lib.rs")

    def test_core_function_links_only_direct_pipeline_variants(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_path = root / "pillow-rs/src/ops/imageops.rs"
            source_path.parent.mkdir(parents=True)
            source_path.write_text(
                "pub fn grayscale(image: &Image) -> Image {\n"
                "    // PipelineOp::CommentOnly\n"
                "    let text = \"PipelineOp::StringOnly\";\n"
                "    Image::push_op(image, PipelineOp::Grayscale)\n"
                "}\n"
                "mod tests { fn stray() { PipelineOp::TestOnly; } }\n",
                encoding="utf-8",
            )
            references = direct_pipeline_variant_references(
                root, "pillow-rs/src/ops/imageops.rs", "grayscale"
            )
            self.assertEqual([item["variant"] for item in references], ["Grayscale"])

    def test_direct_receiver_calls_ignore_literals_and_resolve_image_helpers(self) -> None:
        wrapper = (
            "impl PyImage {\n"
            "    fn filter_name(&self) {\n"
            "        // self.inner.comment_only();\n"
            "        let text = \"self.inner.string_only()\";\n"
            "        py.detach(|| self.inner\n"
            "            .filter(&text));\n"
            "    }\n"
            "}\n"
        )
        calls = direct_receiver_method_calls(wrapper, wrapper.index("fn filter_name"), "self.inner")
        self.assertEqual([call["symbol"] for call in calls], ["filter"])
        self.assertEqual(calls[0]["line"], 5)

        core = (
            "impl Image {\n"
            "    pub fn filter(&self) { self.filter_push(); }\n"
            "    fn filter_push(&self) { PipelineOp::Filter5x5; }\n"
            "}\n"
            "#[cfg(test)] mod tests { impl Image { pub fn filter(&self) {} } }\n"
        )
        methods = inherent_method_definitions(core, "pillow-rs/src/ops/filter.rs", "Image")
        self.assertEqual([(item["symbol"], item["public"]) for item in methods], [
            ("filter", True), ("filter_push", False)
        ])
        helper = next(item for item in methods if item["symbol"] == "filter_push")
        refs = direct_pipeline_variant_references_at(
            core, "pillow-rs/src/ops/filter.rs", helper["function_start"], "filter_push"
        )
        self.assertEqual([item["variant"] for item in refs], ["Filter5x5"])

    def test_function_body_parser_accepts_array_return_types_and_local_receiver_aliases(self) -> None:
        source = (
            "fn rgb(&self) -> Result<Option<[u8; 3]>, Error> {\n"
            "    let inner = self.inner.clone();\n"
            "    inner.enhance_brightness(1.0)\n"
            "}\n"
        )
        start = source.index("fn rgb")
        aliases = direct_receiver_aliases(source, start, "self.inner")
        self.assertEqual(
            [(alias["name"], alias["line"]) for alias in aliases], [("inner", 2)]
        )
        calls = direct_receiver_method_calls(source, start, "inner")
        self.assertEqual([call["symbol"] for call in calls], ["enhance_brightness"])

    def test_literal_pyo3_callback_name_is_reported_without_resolving_its_runtime_type(self) -> None:
        source = (
            "fn filter(&self) {\n"
            "    // filter.call_method1(\"wrong\", value);\n"
            "    filter.call_method1(\"_apply\", (image,))?;\n"
            "}\n"
        )
        callbacks = direct_literal_python_method_callbacks(
            source, source.index("fn filter")
        )
        self.assertEqual(
            [(item["method_name"], item["line"]) for item in callbacks],
            [("_apply", 3)],
        )

    def test_pyo3_inner_field_alias_resolves_only_explicit_core_image_import(self) -> None:
        source = (
            "use pillow_rs::Image as RsImage;\n"
            "struct PyImage { inner: RsImage }\n"
        )
        self.assertEqual(
            struct_field_type(source, "PyImage", "inner"),
            {
                "field": "inner",
                "field_type": "RsImage",
                "resolved_type": "pillow_rs::Image",
                "resolution": "resolved",
            },
        )
        self.assertEqual(
            struct_field_type("struct PyImage { inner: Image }", "PyImage", "inner")["resolution"],
            "unresolved_type",
        )
        self.assertEqual(
            struct_field_type(
                "struct PyDraw { draw: pillow_rs::Draw }", "PyDraw", "draw"
            )["resolved_type"],
            "pillow_rs::Draw",
        )

    def test_top_level_function_graph_resolves_unique_same_module_helper(self) -> None:
        source = (
            "pub fn merge_inputs(mode: &str) { merge(mode); }\n"
            "pub fn merge(mode: &str) { PipelineOp::Merge; }\n"
            "mod tests { fn stray() { PipelineOp::TestOnly; } }\n"
        )
        functions = top_level_function_definitions(source, "pillow-rs/src/ops/module_fns.rs")
        self.assertEqual(set(functions), {"merge_inputs", "merge"})
        self.assertEqual(
            [call["symbol"] for call in functions["merge_inputs"][0]["direct_function_calls"]],
            ["merge"],
        )
        self.assertEqual(
            [item["variant"] for item in functions["merge"][0]["pipeline_references"]],
            ["Merge"],
        )


class RustModuleInventoryTests(unittest.TestCase):
    def test_resolves_conventional_path_cfg_inline_path_and_literal_include(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            package = repo / "fixture"
            source = package / "src"
            (source / "outer").mkdir(parents=True)
            (source / "inline").mkdir()
            (source / "assets").mkdir()
            (source / "lib.rs").write_text(
                "mod outer;\n"
                "pub use crate::outer::Thing;\n"
                "mod inline {\n"
                "    #[cfg(target_arch = \"x86_64\")]\n"
                "    #[path = \"child.rs\"]\n"
                "    mod selected;\n"
                "}\n"
                "#[cfg(test)]\n"
                "#[path = \"test_module.rs\"]\n"
                "mod tests_only;\n"
                "const ASSET: &[u8] = include_bytes!(\"assets/input.bin\");\n"
                "include!(concat!(\"generated\", \".rs\"));\n",
                encoding="utf-8",
            )
            (source / "outer.rs").write_text(
                "#[cfg(feature = \"optional\")]\nmod optional;\n",
                encoding="utf-8",
            )
            (source / "outer/optional.rs").write_text("", encoding="utf-8")
            (source / "inline/child.rs").write_text("", encoding="utf-8")
            (source / "test_module.rs").write_text("", encoding="utf-8")
            (source / "generated.rs").write_text("", encoding="utf-8")
            (source / "orphan.rs").write_text("", encoding="utf-8")
            (source / "assets/input.bin").write_bytes(b"fixture")

            report = build_rust_module_inventory(repo, packages=("fixture",))

            resolved = {
                (edge["from"], edge["module"], edge["to"], edge["conditional"])
                for edge in report["module_edges"]
            }
            self.assertIn(
                ("fixture/src/lib.rs", "outer", "fixture/src/outer.rs", False),
                resolved,
            )
            self.assertIn(
                ("fixture/src/outer.rs", "optional", "fixture/src/outer/optional.rs", True),
                resolved,
            )
            self.assertIn(
                ("fixture/src/lib.rs", "selected", "fixture/src/inline/child.rs", True),
                resolved,
            )
            self.assertIn(
                ("fixture/src/lib.rs", "tests_only", "fixture/src/test_module.rs", True),
                resolved,
            )
            self.assertEqual(report["summary"]["unresolved_module_edges"], 0)
            self.assertEqual(report["summary"]["include_references"], 2)
            self.assertEqual(report["summary"]["reexports"], 1)
            self.assertEqual(
                report["reexports"][0]["declaration"],
                "pub use crate::outer::Thing;",
            )
            self.assertEqual(
                {item["status"] for item in report["include_references"]},
                {"resolved", "dynamic_path_review"},
            )
            self.assertEqual(
                [item["path"] for item in report["unreached_source_candidates"]],
                ["fixture/src/generated.rs", "fixture/src/orphan.rs"],
            )
            self.assertIn(
                "not proof of unreachable",
                " ".join(report["limits"]),
            )


class PythonBindingIndexTests(unittest.TestCase):
    def test_facade_module_function_alias_and_inherited_method_resolve(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            python = repo / "python"
            (python / "PIL").mkdir(parents=True)
            (python / "pillow_rs").mkdir()
            (python / "PIL/__init__.py").write_text(
                "from . import Image as Image\nfrom pillow_rs import imageops as ImageOps\n",
                encoding="utf-8",
            )
            (python / "PIL/Image.py").write_text(
                "from pillow_rs.image import Image as Image\n"
                "from pillow_rs import operations as _operations\n"
                "new = _operations.new\n",
                encoding="utf-8",
            )
            (python / "pillow_rs/__init__.py").write_text("", encoding="utf-8")
            (python / "pillow_rs/operations.py").write_text(
                "def new():\n    pass\n", encoding="utf-8"
            )
            (python / "pillow_rs/imageops.py").write_text(
                "def grayscale(image):\n    pass\n", encoding="utf-8"
            )
            (python / "pillow_rs/image.py").write_text(
                "class _Base:\n    def enhance(self):\n        pass\n"
                "class Image(_Base):\n    def resize(self):\n        pass\n",
                encoding="utf-8",
            )
            index = PythonBindingIndex(python, repo)
            new, _ = index.resolve_public_path("PIL.Image.new")
            resize, _ = index.resolve_public_path("PIL.Image.Image.resize")
            grayscale, _ = index.resolve_public_path("PIL.ImageOps.grayscale")
            inherited, _ = index.resolve_public_path("PIL.Image.Image.enhance")
            self.assertIsInstance(new, Binding)
            self.assertEqual((new.path, new.name), ("python/pillow_rs/operations.py", "new"))
            self.assertEqual((resize.path, resize.name), ("python/pillow_rs/image.py", "resize"))
            self.assertEqual((grayscale.path, grayscale.name), ("python/pillow_rs/imageops.py", "grayscale"))
            self.assertEqual((inherited.path, inherited.name), ("python/pillow_rs/image.py", "enhance"))

    def test_wrapper_calls_resolve_to_registered_pyo3_functions_and_methods(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            python = repo / "python"
            (python / "PIL").mkdir(parents=True)
            (python / "pillow_rs").mkdir()
            (python / "PIL/__init__.py").write_text("", encoding="utf-8")
            (python / "pillow_rs/__init__.py").write_text("", encoding="utf-8")
            (python / "pillow_rs/imagefilter.py").write_text(
                "class Callback:\n"
                "    def _apply(self, rust_image):\n"
                "        return rust_image.filter_name()\n",
                encoding="utf-8",
            )
            (python / "pillow_rs/operations.py").write_text(
                "from . import _core\n"
                "def invert(image):\n"
                "    return _core.ops_invert(image._rust_image.resize())\n"
                "def draw(canvas):\n"
                "    canvas._draw.line()\n",
                encoding="utf-8",
            )
            index = PythonBindingIndex(python, repo)
            binding = index.bindings["pillow_rs.operations"]["invert"]
            calls = python_native_calls(binding, index)
            self.assertEqual(
                [(call.kind, call.owner, call.symbol) for call in calls],
                [("pyfunction", None, "ops_invert"), ("pymethod", "PyImage", "resize")],
            )
            draw_binding = index.bindings["pillow_rs.operations"]["draw"]
            draw_calls = python_native_calls(draw_binding, index)
            self.assertEqual(
                [(call.kind, call.owner, call.symbol) for call in draw_calls],
                [("pymethod", "PyDraw", "line")],
            )
            callback = index.bindings["pillow_rs.imagefilter"]["Callback"]
            callback_apply = callback.members["_apply"]
            callback_calls = python_native_calls(callback_apply, index)
            self.assertEqual(
                [(call.kind, call.owner, call.symbol) for call in callback_calls],
                [("pymethod", "PyImage", "filter_name")],
            )
            self.assertTrue(callback_calls[0].dynamic_callback_candidate)

            rust = repo / "pillow-rs-py/src"
            rust.mkdir(parents=True)
            (rust / "lib.rs").write_text(
                "#[pyfunction]\nfn ops_invert(image: &Image) { pillow_rs::imageops_grayscale(image); }\n"
                "#[pymethods]\nimpl PyImage {\n"
                "    fn resize() {}\n"
                "    fn filter_name() {}\n"
                "    #[pyo3(name = \"transpose\")]\n"
                "    fn transpose_py() {}\n"
                "}\n"
                "#[pymethods]\nimpl PyDraw { fn line() {} }\n"
                "#[pymethods]\nimpl PyFont { fn get_name<'py>(&self) {} }\n"
                "fn register(m: &Bound<PyModule>) {\n"
                "    m.add_class::<PyImage>();\n"
                "    m.add_class::<PyDraw>();\n"
                "    m.add_class::<PyFont>();\n"
                "    m.add_function(wrap_pyfunction!(ops_invert, m));\n"
                "}\n",
                encoding="utf-8",
            )
            declarations = PyO3Index(rust, repo)
            invert = declarations.resolve("pyfunction", "ops_invert")
            self.assertTrue(invert.registered)
            self.assertEqual(
                [call["symbol"] for call in invert.direct_core_calls],
                ["imageops_grayscale"],
            )
            self.assertTrue(declarations.resolve("pymethod", "resize", "PyImage").registered)
            self.assertTrue(declarations.resolve("pymethod", "line", "PyDraw").registered)
            self.assertTrue(declarations.resolve("pymethod", "get_name", "PyFont").registered)
            renamed = declarations.resolve("pymethod", "transpose", "PyImage")
            self.assertEqual(renamed.symbol, "transpose_py")


class WorkspaceCrosswalkTests(unittest.TestCase):
    def test_current_workspace_maps_every_pipeline_variant_without_claiming_execution(self) -> None:
        report = build_crosswalk()
        self.assertEqual(report["schema"], "pillow-rs/operation-source-crosswalk@10")
        self.assertEqual(len(report["source"]["crosswalk_input_sha256"]), 64)
        self.assertGreater(report["source"]["crosswalk_input_file_count"], 100)
        self.assertEqual(
            report["summary"]["rust_source_files"],
            len(report["rust_module_inventory"]["source_files"]),
        )
        self.assertEqual(
            report["summary"]["rust_crate_roots"],
            len(report["rust_module_inventory"]["roots"]),
        )
        self.assertEqual(report["summary"]["rust_module_edges_unresolved"], 0)
        self.assertEqual(report["summary"]["rust_source_unreached_candidates"], 0)
        self.assertFalse(report["scope"]["rust_module_reachability_proven"])
        self.assertFalse(report["declared_evidence"]["scope"]["parity_execution_proven"])
        self.assertFalse(report["declared_evidence"]["scope"]["benchmark_execution_proven"])
        self.assertEqual(
            report["summary"]["operations_with_parity_cases"]
            + report["summary"]["operations_without_parity_cases"],
            report["summary"]["public_operations"],
        )
        self.assertGreater(report["summary"]["declared_parity_cases"], 0)
        self.assertGreater(report["summary"]["declared_benchmark_workloads"], 0)
        self.assertEqual(
            report["summary"]["operations_without_benchmark_workloads"],
            len(report["review_queues"]["public_operation_without_benchmark_workload"]),
        )
        source_files = report["rust_module_inventory"]["source_files"]
        self.assertTrue(source_files)
        self.assertTrue(
            all(item["operation_crosswalk"]["status"] in {
                "reached_from_public_operation_by_static_edges",
                "possible_route_via_dynamic_python_callback_candidate",
                "static_graph_edge_without_public_operation_route",
                "no_direct_static_edge",
            } for item in source_files)
        )
        review_queue = report["review_queues"]["operation_graph_source_without_public_route"]
        expected_review_paths = {
            item["path"] for item in source_files
            if item["operation_crosswalk"]["status"]
            != "reached_from_public_operation_by_static_edges"
        }
        self.assertEqual({item["path"] for item in review_queue}, expected_review_paths)
        self.assertEqual(
            report["summary"]["operation_graph_review_candidates"], len(review_queue)
        )
        self.assertTrue(all(item["review_only"] for item in review_queue))
        self.assertTrue(
            {
                item["operation_crosswalk"]["status"] for item in review_queue
            }.issubset({
                "possible_route_via_dynamic_python_callback_candidate",
                "static_graph_edge_without_public_operation_route",
                "no_direct_static_edge",
            })
        )
        self.assertTrue(all(
            "not an unused-code finding" in item["operation_crosswalk"]["reason_for_review"]
            or "unused code" in item["operation_crosswalk"]["reason_for_review"]
            for item in review_queue
        ))
        self.assertTrue(all("family_layout" in item for item in source_files))
        self.assertGreater(report["summary"]["pyo3_core_public_api_calls"], 0)
        self.assertGreater(report["summary"]["pyo3_core_public_api_calls_mapped_to_core_routes"], 0)
        self.assertEqual(
            report["summary"]["pyo3_core_public_api_calls"],
            report["summary"]["pyo3_core_public_api_calls_mapped_to_core_routes"]
            + report["summary"]["pyo3_core_public_api_calls_unresolved"],
        )
        self.assertFalse(report["scope"]["core_function_call_graph_proven"])
        grayscale = next(
            item for item in source_files
            if item["path"] == "pillow-rs/src/ops/imageops.rs"
        )
        self.assertIn(
            "PIL.ImageOps.grayscale",
            grayscale["operation_crosswalk"]["public_operation_ids"],
        )
        self.assertIsNone(grayscale["family_id"])
        self.assertEqual(
            grayscale["family_layout"]["assessment"], "documented_migration_debt"
        )
        self.assertTrue(
            any(
                edge["from"].startswith("core-public-api:imageops_grayscale:")
                and edge["relation"] == "references_pipeline_variant"
                and edge["to"] == "pipeline:Grayscale"
                for edge in report["edges"]
            )
        )
        cpu_grayscale = next(
            item for item in source_files
            if item["path"] == "pillow-rs/src/compute/pool_cpu/ops/grayscale.rs"
        )
        self.assertEqual(
            cpu_grayscale["operation_crosswalk"]["status"],
            "reached_from_public_operation_by_static_edges",
        )
        self.assertEqual(cpu_grayscale["family_id"], "cpu:grayscale")
        self.assertIn(
            "PIL.ImageOps.grayscale",
            cpu_grayscale["operation_crosswalk"]["public_operation_ids"],
        )
        self.assertGreater(report["summary"]["core_public_api_pipeline_variant_routes"], 0)
        self.assertGreater(report["summary"]["pyo3_direct_core_receiver_method_calls_mapped"], 0)
        self.assertGreater(report["summary"]["core_method_pipeline_variant_routes"], 0)
        convert_method = next(
            node for node in report["nodes"]
            if node.get("kind") == "core_image_method" and node.get("symbol") == "convert_with_input"
        )
        convert_node_id = convert_method["id"]
        adjacency = {}
        for edge in report["edges"]:
            adjacency.setdefault(edge["from"], []).append(edge["to"])
        reached = set()
        pending = [convert_node_id]
        while pending:
            current = pending.pop()
            if current in reached:
                continue
            reached.add(current)
            pending.extend(adjacency.get(current, []))
        self.assertTrue({"pipeline:Convert", "pipeline:ConvertLab"} & reached)
        merge_api = next(
            node for node in report["nodes"]
            if node.get("kind") == "core_public_api_route"
            and node.get("public_name") == "image_merge_inputs"
        )
        merge_reached = set()
        pending = [merge_api["id"]]
        while pending:
            current = pending.pop()
            if current in merge_reached:
                continue
            merge_reached.add(current)
            pending.extend(adjacency.get(current, []))
        self.assertIn("pipeline:Merge", merge_reached)
        self.assertGreater(report["summary"]["core_function_helper_pipeline_variant_routes"], 0)
        self.assertFalse(report["scope"]["core_receiver_method_call_graph_complete"])
        self.assertTrue(
            any(
                node.get("kind") == "core_draw_method"
                and node.get("symbol") == "line"
                for node in report["nodes"]
            )
        )
        cpu_enhance = next(
            item for item in source_files
            if item["path"] == "pillow-rs/src/compute/pool_cpu/ops/enhance.rs"
        )
        self.assertIn(
            "PIL.ImageEnhance.Brightness.enhance",
            cpu_enhance["operation_crosswalk"]["public_operation_ids"],
        )
        self.assertIn(
            "PIL.ImageEnhance.Sharpness.enhance",
            cpu_enhance["operation_crosswalk"]["public_operation_ids"],
        )
        simd_merge = next(
            item for item in source_files
            if item["path"] == "pillow-rs/src/compute/pool_simd/ops/adapters/merge.rs"
        )
        self.assertIn(
            "PIL.Image.merge",
            simd_merge["operation_crosswalk"]["public_operation_ids"],
        )
        filter_binding = next(
            node for node in report["nodes"]
            if node.get("kind") == "pymethod"
            and node.get("owner") == "PyImage"
            and node.get("symbol") == "filter"
        )
        self.assertEqual(
            [item["method_name"] for item in filter_binding["dynamic_python_callbacks"]],
            ["_apply"],
        )
        self.assertTrue(
            any(
                edge["from"] == filter_binding["id"]
                and edge["relation"] == "may_dispatch_to_python_callback"
                and edge["status"] == "dynamic_python_callback_candidate"
                for edge in report["edges"]
            )
        )
        cpu_filter = next(
            item for item in source_files
            if item["path"] == "pillow-rs/src/compute/pool_cpu/ops/filter.rs"
        )
        self.assertEqual(
            cpu_filter["operation_crosswalk"]["status"],
            "possible_route_via_dynamic_python_callback_candidate",
        )
        self.assertNotIn(
            "PIL.Image.Image.filter",
            cpu_filter["operation_crosswalk"]["public_operation_ids"],
        )
        self.assertIn(
            "PIL.Image.Image.filter",
            cpu_filter["operation_crosswalk"]["possible_public_operation_ids"],
        )
        self.assertIn(
            cpu_filter["path"],
            {
                item["path"]
                for item in report["review_queues"][
                    "operation_graph_possible_dynamic_callback_routes"
                ]
            },
        )
        simd_filter = next(
            item for item in source_files
            if item["path"] == "pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs"
        )
        self.assertEqual(
            simd_filter["operation_crosswalk"]["status"],
            "possible_route_via_dynamic_python_callback_candidate",
        )
        self.assertIn(
            "PIL.Image.Image.filter",
            simd_filter["operation_crosswalk"]["possible_public_operation_ids"],
        )
        backend_sources = [
            item for item in source_files
            if item["path"].startswith((
                "pillow-rs/src/compute/pool_cpu/ops/",
                "pillow-rs/src/compute/pool_simd/",
                "pillow-rs/src/compute/pool_gpu/",
            ))
        ]
        repo_root = Path(__file__).resolve().parents[2]
        backend_roots = (
            repo_root / "pillow-rs/src/compute/pool_cpu/ops",
            repo_root / "pillow-rs/src/compute/pool_simd",
            repo_root / "pillow-rs/src/compute/pool_gpu",
        )
        expected_backend_paths = {
            path.relative_to(repo_root).as_posix()
            for directory in backend_roots
            for path in directory.rglob("*.rs")
        }
        self.assertEqual({item["path"] for item in backend_sources}, expected_backend_paths)
        self.assertTrue(
            all(item["static_role"] == "declared_module" for item in backend_sources)
        )
        no_edge = [
            item for item in source_files
            if item["operation_crosswalk"]["status"] != "reached_from_public_operation_by_static_edges"
        ]
        self.assertTrue(no_edge)
        self.assertTrue(
            all("not an unused-code finding" in item["operation_crosswalk"]["reason_for_review"]
                for item in no_edge)
        )
        self.assertEqual(report["inventory"]["manifest_operation_count"], 212)
        self.assertEqual(report["summary"]["public_operations"], 212)
        self.assertEqual(report["summary"]["public_operations_with_python_bindings"], 212)
        self.assertEqual(report["summary"]["unmapped_public_operation_bindings"], 0)
        self.assertGreater(report["summary"]["python_binding_native_calls"], 0)
        self.assertGreater(
            report["summary"]["python_binding_native_calls_mapped_to_registered_pyo3"], 0
        )
        self.assertEqual(
            report["summary"]["python_binding_native_calls"],
            report["summary"]["python_binding_native_calls_mapped_to_registered_pyo3"],
        )
        self.assertEqual(report["summary"]["python_binding_native_calls_unresolved"], 0)
        self.assertEqual(
            report["summary"]["python_binding_native_calls_with_registration_review"], 0
        )
        self.assertEqual(
            report["summary"]["pipeline_variants"],
            report["summary"]["variant_key_edges"],
        )
        self.assertGreater(report["summary"]["simd_registrations"], 0)
        self.assertEqual(report["summary"]["unmapped_pipeline_variants"], 0)
        self.assertFalse(report["scope"]["runtime_execution_proven"])
        self.assertTrue(
            all(
                edge["runtime_execution_proven"] is False
                for edge in report["edges"]
                if edge["relation"] == "registers_default_gpu_shader"
            )
        )
        self.assertNotIn(
            "simd_adapter_definition_unresolved",
            {finding["flag"] for finding in report["findings"]},
        )
        self.assertFalse(
            {"matches", "new", "or", "width", "value"}
            & {
                node["symbol"]
                for node in report["nodes"]
                if node["kind"] == "cpu_function"
            }
        )
        self.assertTrue(
            all(
                finding["reason_for_review"]
                for finding in report["findings"]
            )
        )


if __name__ == "__main__":
    unittest.main()

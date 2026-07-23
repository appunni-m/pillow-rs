"""Generic PIL-RSPIL parity test runner.

Discovers fixture pairs from fixtures/input/jsons/ and fixtures_2/input/jsons/.
One parametrized test per individual case — each is tracked separately in coverage.
Zero per-operation logic — the engine handles everything.
"""

import json
import re
from pathlib import Path

import pytest
import pillow_rs as rspil

# Set up headless QApplication for Qt operations (toqpixmap needs it)
try:
    from PySide6.QtWidgets import QApplication
    _qt_app = QApplication.instance()
    if _qt_app is None:
        _qt_app = QApplication([])
except ImportError:
    pass

from engine import (
    ASSERT,
    CASE_MODE_CALL_STYLES,
    CALL_STYLE,
    IMAGE_INPUT_CALL_STYLES,
    _pilify,
    create_input,
    get_call_style,
)
import engine as _engine
from fixture_coverage import (
    assertion_image_modes as _assertion_image_modes,
    backend_parity_coverage_errors,
    coverage_gaps,
    operation_name as _operation_name,
    unknown_fixture_operations,
)

FIXTURES_DIRS = {
    "fixtures": Path(__file__).parent / "fixtures",
    "fixtures_2": Path(__file__).parent / "fixtures_2",
}
PILLOW_VERSION = "12.2.0"
FREETYPE_VERSION = "2.14.3"
ASSERTION_METHODS = {
    "error",
    "image",
    "image_list",
    "typed",
    "tuple",
}
HOST_CHECKOUT_PATH = re.compile(
    r"(?:[A-Za-z]:[\\/]|/(?:Users|home|private/tmp|tmp|workspace|workspaces)/)"
    r"[^\"']*pillow-rs[\\/]pillow-rs-freetype[\\/]tests[\\/]fixtures"
)

# Register extra reference image dirs for fixtures_2 support
_engine.EXTRA_REFERENCE_DIRS = [
    str(Path(__file__).parent / "fixtures_2" / "input" / "images"),
]


def _assertion_references(assertion):
    method = assertion.get("method")
    if method == "image":
        yield assertion.get("reference"), assertion
    elif method == "image_list":
        for item in assertion.get("items", []):
            yield from _assertion_references(item)
    elif method == "tuple":
        for item in assertion.get("items", []):
            yield from _assertion_references(item)


def _assertion_nodes(assertion):
    yield assertion
    if assertion.get("method") in {"image_list", "tuple"}:
        for item in assertion.get("items", []):
            yield from _assertion_nodes(item)


def _fixture_pair_errors():
    """Return every fixture file or case that cannot participate in parity."""
    errors = []
    for base_name, base_dir in FIXTURES_DIRS.items():
        input_dir = base_dir / "input" / "jsons"
        output_dir = base_dir / "outputs" / "jsons"
        input_files = {path.name: path for path in input_dir.glob("*.json")}
        output_files = {path.name: path for path in output_dir.glob("*.json")}

        for name in sorted(input_files.keys() - output_files.keys()):
            errors.append(f"{base_name}: missing oracle output {name}")
        for name in sorted(output_files.keys() - input_files.keys()):
            errors.append(f"{base_name}: orphan oracle output {name}")

        referenced_artifacts = set()
        for name in sorted(input_files.keys() & output_files.keys()):
            inp = json.loads(input_files[name].read_text())
            out = json.loads(output_files[name].read_text())
            if inp.get("format_version") != 2:
                errors.append(f"{base_name}/{name}: input format_version is not 2")
            if out.get("format_version") != 2:
                errors.append(f"{base_name}/{name}: output format_version is not 2")
            if out.get("pillow_version") != PILLOW_VERSION:
                errors.append(
                    f"{base_name}/{name}: oracle Pillow version is "
                    f"{out.get('pillow_version')!r}, expected {PILLOW_VERSION!r}"
                )
            if out.get("freetype_version") != FREETYPE_VERSION:
                errors.append(
                    f"{base_name}/{name}: oracle FreeType version is "
                    f"{out.get('freetype_version')!r}, expected {FREETYPE_VERSION!r}"
                )
            if out.get("operation") != inp.get("operation"):
                errors.append(f"{base_name}/{name}: input/output operation differs")
            if out.get("suite", 0) != inp.get("suite", 0):
                errors.append(f"{base_name}/{name}: input/output suite differs")

            operation = inp["operation"]
            call_style = get_call_style(
                operation["module"],
                operation["target"],
                operation.get("class"),
            )
            input_ids = [case["id"] for case in inp.get("cases", [])]
            output_ids = [case["id"] for case in out.get("cases", [])]
            if len(input_ids) != len(set(input_ids)):
                errors.append(f"{base_name}/{name}: duplicate input case ids")
            if len(output_ids) != len(set(output_ids)):
                errors.append(f"{base_name}/{name}: duplicate output case ids")
            missing = sorted(set(input_ids) - set(output_ids))
            orphaned = sorted(set(output_ids) - set(input_ids))
            if missing:
                errors.append(f"{base_name}/{name}: missing output cases {missing}")
            if orphaned:
                errors.append(f"{base_name}/{name}: orphan output cases {orphaned}")

            for case in inp.get("cases", []):
                if call_style not in IMAGE_INPUT_CALL_STYLES and (
                    case.get("input") is not None
                    or case.get("input2") is not None
                ):
                    errors.append(
                        f"{base_name}/{name}/{case.get('id')}: "
                        f"{call_style} ignores fixture input images"
                    )
                if (
                    case.get("mode")
                    and call_style not in CASE_MODE_CALL_STYLES
                ):
                    errors.append(
                        f"{base_name}/{name}/{case.get('id')}: "
                        f"{call_style} ignores the top-level fixture mode"
                    )

            for case in out.get("cases", []):
                assertion = case.get("assert", {})
                for assertion_node in _assertion_nodes(assertion):
                    method = assertion_node.get("method")
                    if method not in ASSERTION_METHODS:
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            f"unsupported assertion method {method!r}"
                        )
                        continue
                    forbidden = {
                        "message_contains",
                        "prefix",
                        "tolerance",
                    } & assertion_node.keys()
                    if forbidden:
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            f"non-exact assertion fields {sorted(forbidden)}"
                        )
                    if method == "error" and "message" not in assertion_node:
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            "error assertion lacks exact message"
                        )
                    if (
                        method == "error"
                        and HOST_CHECKOUT_PATH.search(
                            str(assertion_node.get("message", ""))
                        )
                    ):
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            "error assertion embeds a host checkout path"
                        )
                    if method == "string" and "value" not in assertion_node:
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            "string assertion lacks exact repr"
                        )
                    if (
                        method == "image_list"
                        and "container_type" not in assertion_node
                    ):
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            "image-list assertion lacks exact container type"
                        )
                for reference, image_assertion in _assertion_references(assertion):
                    if not isinstance(reference, str):
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            f"invalid artifact reference {reference!r}"
                        )
                        continue
                    relative = Path(reference)
                    if relative.is_absolute() or ".." in relative.parts:
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            f"unsafe artifact reference {reference!r}"
                        )
                        continue
                    referenced_artifacts.add(relative.as_posix())
                    if not (base_dir / "outputs" / relative).is_file():
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            f"missing artifact {reference}"
                        )
                    if relative.suffix == ".bin" and "raw_kind" not in image_assertion:
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            "raw assertion lacks result-kind metadata"
                        )
                    if image_assertion.get("raw_kind") in {"bytes", "qt_image"} and (
                        "result_type" not in image_assertion
                    ):
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            "raw assertion lacks exact result type"
                        )
                    if image_assertion.get("raw_kind") == "image" and (
                        "mode" not in image_assertion
                        or "size" not in image_assertion
                    ):
                        errors.append(
                            f"{base_name}/{name}/{case.get('id')}: "
                            "raw image assertion lacks mode or size"
                        )

        artifact_files = {
            path.relative_to(base_dir / "outputs").as_posix()
            for directory in ("images", "raws")
            for path in (base_dir / "outputs" / directory).rglob("*")
            if path.is_file()
        }
        for reference in sorted(artifact_files - referenced_artifacts):
            errors.append(f"{base_name}: orphan oracle artifact {reference}")
    return errors


def _discover():
    """Yield one parametrized test per fixture case from all fixture directories.

    Each yield produces a pytest.param with:
      - (fixtures_base, fixture_file, case_id) as the test args
      - id = "Module.target_caseId" (e.g., "Image.resize_Image_resize_L")
      - marks = @pytest.mark.covers("Module.target", mode="L")
    """
    seen = set()

    for base_name, base_dir in FIXTURES_DIRS.items():
        input_dir = base_dir / "input" / "jsons"
        output_dir = base_dir / "outputs" / "jsons"

        if not input_dir.exists():
            continue

        for fpath in sorted(input_dir.glob("*.json")):
            if not (output_dir / fpath.name).exists():
                continue
            inp = json.loads(fpath.read_text())
            out = json.loads((output_dir / fpath.name).read_text())
            op = inp["operation"]
            target = _operation_name(op)
            call_style = get_call_style(
                op["module"],
                op["target"],
                op.get("class"),
            )
            out_cases = {c["id"]: c for c in out["cases"]}

            for case in inp["cases"]:
                cid = case["id"]
                if cid not in out_cases:
                    continue
                mode = case.get("mode", "")
                param_id = f"{fpath.stem}__{cid}"

                # Guard against case ID collisions across fixture directories
                assert param_id not in seen, f"Collision: {param_id}"
                seen.add(param_id)

                # Build @pytest.mark.covers marker for coverage tracking
                marker_kwargs = {}
                if mode and (
                    call_style in CASE_MODE_CALL_STYLES
                    and (
                        call_style in {"file_open", "palette_method"}
                        or case.get("input") is not None
                        or case.get("input2") is not None
                    )
                ):
                    marker_kwargs["mode"] = mode
                else:
                    parameter_mode = case.get("params", {}).get("mode")
                    output_modes = set(
                        _assertion_image_modes(
                            base_dir,
                            out_cases[cid].get("assert", {}),
                        )
                    )
                    if isinstance(parameter_mode, str):
                        marker_kwargs["mode"] = parameter_mode
                    elif len(output_modes) == 1:
                        marker_kwargs["mode"] = output_modes.pop()
                covers_marker = getattr(pytest.mark, "covers")(target, **marker_kwargs)

                yield pytest.param(
                    base_name, fpath.name, cid,
                    id=param_id,
                    marks=[covers_marker],
                )


@pytest.mark.parametrize("fixtures_base,fixture_file,case_id", _discover())
def test_parity(fixtures_base, fixture_file, case_id):
    """Run a single fixture case and assert PIL parity."""
    base_dir = Path(__file__).parent / fixtures_base
    input_dir = base_dir / "input" / "jsons"
    output_dir = base_dir / "outputs" / "jsons"

    # Override OUTPUTS_DIR so _load_reference reads from the correct directory
    _engine.OUTPUTS_DIR = base_dir / "outputs"
    # Override ASSETS_DIR so _decode_asset/_encode_roundtrip resolve correctly
    _engine.ASSETS_DIR = base_dir / "input" / "images"

    inp = json.loads((input_dir / fixture_file).read_text())
    out = json.loads((output_dir / fixture_file).read_text())
    op = inp["operation"]
    call_style = get_call_style(op["module"], op["target"], op.get("class"))

    out_cases = {c["id"]: c for c in out["cases"]}
    case = next(c for c in inp["cases"] if c["id"] == case_id)
    assertion = out_cases[case_id]["assert"]

    mode = case.get("mode")
    img = create_input(rspil, mode, case.get("input"))
    img2 = create_input(rspil, mode, case.get("input2"))
    params = _pilify(dict(case.get("params", {})))
    if op["module"] == "ImagePalette" and mode:
        params["_fixture_mode"] = mode
    if call_style == "file_open" and mode:
        params["_fixture_mode"] = mode

    # Decode/Encode cases pass asset info via params (no input/mode fields)
    if op["module"] == "Decode":
        params["asset"] = case["asset"]
    elif op["module"] == "Encode":
        params["source_asset"] = case["source_asset"]
        params["source_format"] = case.get("source_format", op["target"])

    try:
        result = CALL_STYLE[call_style](rspil, img, img2, op["target"], params)
    except Exception as e:
        if assertion["method"] == "error":
            assert ASSERT["error"](assertion, e), f"[{case_id}] error mismatch"
            return
        raise

    assert ASSERT[assertion["method"]](assertion, result), \
        f"[{case_id}] {assertion['method']} mismatch"


# ── Coverage validation ──────────────────────────────────────────

@pytest.mark.coverage_meta
def test_fixture_inputs_and_oracles_are_one_to_one():
    errors = _fixture_pair_errors()
    assert not errors, (
        f"{len(errors)} fixture inputs/oracles are not one-to-one:\n"
        + "\n".join(errors)
    )


@pytest.mark.coverage_meta
def test_assertion_contracts_are_exact():
    assert ASSERT["exact"]({"value": 1}, 1)
    assert not ASSERT["exact"]({"value": 1}, True)
    assert not ASSERT["float"]({"value": 1.0}, 1)
    assert ASSERT["error"](
        {"exception": "ValueError", "message": "exact"},
        ValueError("exact"),
    )
    assert not ASSERT["error"](
        {"exception": "ValueError", "message": "exact"},
        ValueError("exactly"),
    )
    assert not ASSERT["string"]({"value": "'expected'"}, "other")
    assert not ASSERT["tuple"](
        {"items": [{"method": "exact", "value": 1}]},
        (1, 2),
    )


@pytest.mark.coverage_meta
def test_opaque_results_use_semantic_call_styles():
    assert get_call_style("Image", "getdata") == "instance_method_sequence"
    assert get_call_style("Image", "load") == "pixel_access"
    assert get_call_style("Image", "getim") == "result_descriptor"
    assert get_call_style("Image", "paste") == "instance_method_dual_mutate"
    assert get_call_style("Image", "apply_transparency") == "instance_method_mutate"
    assert get_call_style("Image", "close") == "terminal_image_method"
    assert get_call_style("Image", "seek") == "seek"
    assert get_call_style("ImageDraw", "getfont") == "draw_getfont"


@pytest.mark.coverage_meta
def test_backend_parity_coverage_metadata_integrity():
    path = (
        Path(__file__).parent.parent
        / "pillow-rs"
        / "tests"
        / "fixtures"
        / "image_backend"
        / "backend_parity.json"
    )
    fixture = json.loads(path.read_text())
    assert not backend_parity_coverage_errors(fixture, path.parent)

    missing = json.loads(json.dumps(fixture))
    missing["coverage"].pop()
    assert any(
        error.startswith("missing backend coverage row:")
        for error in backend_parity_coverage_errors(missing, path.parent)
    )

    orphan = json.loads(json.dumps(fixture))
    orphan_row = json.loads(json.dumps(orphan["coverage"][0]))
    orphan_row["case_id"] = "orphan_case"
    orphan["coverage"].append(orphan_row)
    assert any(
        error.startswith("orphan backend coverage row:")
        for error in backend_parity_coverage_errors(orphan, path.parent)
    )

    mismatched = json.loads(json.dumps(fixture))
    mismatched["coverage"][0]["expected"]["pixels_hex"] += "00"
    assert any(
        error.startswith("mismatched backend coverage row:")
        for error in backend_parity_coverage_errors(mismatched, path.parent)
    )


@pytest.mark.coverage_meta
def test_coverage_complete():
    """Every implemented operation must have a fixture, and every declared
    supported_mode must have at least one fixture case. Fails if any gaps exist."""
    missing_operations, missing_modes = coverage_gaps()
    unknown_operations = unknown_fixture_operations()
    assert not unknown_operations and not missing_operations and not missing_modes, (
        f"Fixtures for {len(unknown_operations)} unknown operations:\n  "
        + "\n  ".join(unknown_operations)
        + f"\n\nMissing fixtures for {len(missing_operations)} operations:\n  "
        + "\n  ".join(missing_operations)
        + f"\n\nMissing mode cases ({len(missing_modes)} gaps):\n  "
        + "\n  ".join(missing_modes)
    )

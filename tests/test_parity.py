"""Generic PIL-RSPIL parity test runner.

Discovers fixture pairs from fixtures/input/jsons/ and fixtures_2/input/jsons/.
One parametrized test per individual case — each is tracked separately in coverage.
Zero per-operation logic — the engine handles everything.
"""

import json
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

from engine import CALL_STYLE, ASSERT, _pilify, create_input, get_call_style
import engine as _engine

FIXTURES_DIRS = {
    "fixtures": Path(__file__).parent / "fixtures",
    "fixtures_2": Path(__file__).parent / "fixtures_2",
}

# Register extra reference image dirs for fixtures_2 support
_engine.EXTRA_REFERENCE_DIRS = [
    str(Path(__file__).parent / "fixtures_2" / "input" / "images"),
]


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
            target = f"{op.get('module', '?')}.{op['target']}"
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
                if mode:
                    marker_kwargs["mode"] = mode
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
    call_style = get_call_style(op["module"], op["target"])

    out_cases = {c["id"]: c for c in out["cases"]}
    case = next(c for c in inp["cases"] if c["id"] == case_id)
    assertion = out_cases[case_id]["assert"]

    mode = case.get("mode")
    img = create_input(rspil, mode, case.get("input"))
    img2 = create_input(rspil, mode, case.get("input2"))
    params = _pilify(dict(case.get("params", {})))

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

def test_coverage_complete():
    """Every implemented operation must have a fixture, and every declared
    supported_mode must have at least one fixture case. Fails if any gaps exist."""
    import yaml
    manifest_path = Path(__file__).parent.parent / "manifest.yaml"
    with open(manifest_path) as f:
        manifest = yaml.safe_load(f)

    # Build {operation_name: {set of supported_modes}} from manifest
    op_modes = {}
    for mod_name, mod_data in manifest.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions"]:
            for entry in mod_data.get(section, []):
                if entry.get("status") == "implemented":
                    op = f"{mod_name}.{entry['name']}"
                    op_modes[op] = set(entry.get("supported_modes", []))

        for cls in mod_data.get("classes", []):
            if cls.get("status") == "implemented":
                methods = cls.get("methods", [])
                if methods:
                    for entry in methods:
                        entry_status = entry.get("status", "")
                        if cls.get("status") == "implemented" and entry_status != "ignored":
                            op = f"{mod_name}.{entry['name']}"
                            op_modes[op] = set(entry.get("supported_modes",
                                                cls.get("supported_modes", [])))
                else:
                    op = f"{mod_name}.{cls['name']}"
                    op_modes[op] = set(cls.get("supported_modes", []))

    # Build {operation_name: {modes with fixture cases}} from ALL fixture directories
    fixture_modes = {}
    for base_name, base_dir in FIXTURES_DIRS.items():
        input_dir = base_dir / "input" / "jsons"
        if not input_dir.exists():
            continue
        for fpath in sorted(input_dir.glob("*.json")):
            stem = fpath.stem
            if stem not in fixture_modes:
                fixture_modes[stem] = set()
            fx = json.loads(fpath.read_text())
            for case in fx.get("cases", []):
                mode = case.get("mode", "")
                if mode:
                    fixture_modes[stem].add(mode)

    # Check gaps
    missing_ops = []
    missing_modes = []
    for op, declared_modes in sorted(op_modes.items()):
        if op not in fixture_modes:
            if declared_modes:  # only if it has modes
                missing_ops.append(op)
            continue
        fixture_m = fixture_modes[op]
        gap = declared_modes - fixture_m
        if gap:
            for mode in sorted(gap):
                missing_modes.append(f"  {op}: {mode}")

    if missing_ops or missing_modes:
        msg_parts = []
        if missing_ops:
            msg_parts.append(f"Missing fixtures for {len(missing_ops)} operations:\n  " +
                             "\n  ".join(sorted(missing_ops)))
        if missing_modes:
            msg_parts.append(f"Missing mode cases ({len(missing_modes)} gaps):\n" +
                             "\n".join(sorted(missing_modes)[:30]))
            if len(missing_modes) > 30:
                msg_parts.append(f"  ... and {len(missing_modes) - 30} more")
        pytest.fail("\n\n".join(msg_parts))

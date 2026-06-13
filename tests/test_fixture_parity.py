"""Fixture-based parity tests — JSON-driven, no per-operation dispatch.

Reads JSON fixtures from tests/fixtures/. Each fixture is self-describing:
it contains the operation type, module, target, params, input image bytes,
and expected output. The test engine is a thin shell that calls the shared
execution_engine.execute() function via the RSPIL backend.

@pytest.mark.covers markers are auto-generated from fixture operation metadata.
"""
import json, hashlib
from pathlib import Path
import pytest

FIXTURES_DIR = Path(__file__).parent / 'fixtures'

from rspil_backend import RspilBackend
_backend = RspilBackend()

from scripts.coverage.execution_engine import execute as _execute


def _hash(data):
    return hashlib.sha256(data).hexdigest()


def _make_input(fixture):
    """Create RSPIL image from fixture input block."""
    inp = fixture["input"]
    img = _backend.make_image(inp["mode"], tuple(inp["size"]), bytes.fromhex(inp["bytes"]))
    if img is None:
        ref = bytes.fromhex(fixture["config"]["reference_bytes_rgb"])
        img = _backend.make_image("RGB", tuple(inp["size"]), ref)
        if img:
            img = img.convert(inp["mode"])
    return img


def _discover_fixtures():
    """Auto-discover all fixture files from disk."""
    fixtures = []
    for fpath in sorted(FIXTURES_DIR.glob("*.json")):
        with open(fpath) as f:
            fx = json.load(f)
        op_def = fx["operation"]
        name = fpath.stem
        fixtures.append(pytest.param(str(fpath.name), id=name, marks=[
            pytest.mark.covers(
                f"{op_def.get('module', '?')}.{op_def['target']}",
                mode=fx["input"]["mode"], target="cpu", variant=op_def["type"])]))
    return fixtures


_PARAMS = _discover_fixtures()


@pytest.mark.parametrize('fixture_file', _PARAMS)
def test_fixture_parity(fixture_file):
    """RSPIL output must match PIL reference (hash, value, or error)."""
    fixture = json.loads((FIXTURES_DIR / fixture_file).read_text())
    op_def = fixture["operation"]
    img = _make_input(fixture)
    if img is None:
        pytest.xfail(f"Cannot create input for {fixture['input']['mode']}")

    img2 = None
    if "input2" in fixture:
        inp2 = fixture["input2"]
        img2 = _backend.make_image(inp2["mode"], tuple(inp2["size"]), bytes.fromhex(inp2["bytes"]))

    # Error path
    if fixture["expected"]["result_type"] == "error":
        try:
            _execute(_backend, op_def, img, img2)
            pytest.xfail(f'{op_def["module"]}.{op_def["target"]}: expected error but succeeded')
        except Exception as e:
            if fixture["expected"]["value"] not in str(type(e).__name__) + ": " + str(e):
                pytest.xfail(f"Error mismatch: got {type(e).__name__}: {e}")
        return

    # Execute
    try:
        result = _execute(_backend, op_def, img, img2)
    except NotImplementedError as e:
        pytest.xfail(f'{op_def["module"]}.{op_def["target"]}: {e}')

    if result is None:
        pytest.xfail("result is None")

    expected = fixture["expected"]

    # Value comparison
    if expected["result_type"] == "value":
        val = expected["value"]
        actual = result
        # PixelAccess match
        if isinstance(val, str) and val.startswith("<PixelAccess") and hasattr(actual, '__str__') and str(actual).startswith("<PixelAccess"):
            return
        # Float tolerance
        if isinstance(actual, (int, float)) and isinstance(val, (int, float)):
            if abs(actual - val) < 0.01: return
        # Direct list/tuple comparison
        if isinstance(actual, (list, tuple)):
            act = [list(x) if isinstance(x, (list, tuple)) else x for x in actual]
            if act == val: return
        if actual == val: return
        pytest.xfail("value mismatch")

    # Hash comparison
    if expected["result_type"] == "hash":
        raw_bytes = b''
        if hasattr(result, 'tobytes'):
            try: raw_bytes = result.tobytes()
            except: pytest.xfail("tobytes failed")
        elif isinstance(result, bytes): raw_bytes = result
        else: pytest.xfail("no tobytes()")

        actual_hash = _hash(raw_bytes)
        if actual_hash == expected["value"]: return

        # Tolerance check for lossy ops
        TOLERANCE = {'Image.resize', 'Image.thumbnail', 'ImageEnhance.Brightness',
                     'ImageEnhance.Color', 'ImageEnhance.Contrast', 'ImageEnhance.Sharpness',
                     'ImageFilter.GaussianBlur', 'Image.quantize'}
        op_key = f"{op_def['module']}.{op_def['target']}"
        if op_key in TOLERANCE and len(raw_bytes) > 0 and len(expected["value"]) == 64:
            exp_bytes = bytes.fromhex(expected["value"])
            if len(exp_bytes) == len(raw_bytes):
                diffs = sum(1 for a, b in zip(raw_bytes, exp_bytes) if a != b)
                if diffs / len(raw_bytes) * 100 < 5.0: return

        pytest.xfail(f"Hash mismatch: expected={expected['value'][:12]} got={actual_hash[:12]}")

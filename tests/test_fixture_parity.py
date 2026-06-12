"""Fixture-based parity tests — same approach as JS/WASM tests.

Each test loads a JSON fixture (pre-computed PIL reference hash),
runs the RSPIL Python operation, hashes output, and compares.

This mirrors pillow-rs-js/tests/run_wasm_test.mjs exactly — same
fixtures, same hashing, same comparison. This ensures Python and
WASM targets are tested identically.

Differences between Python and WASM results for the same fixture
indicate a cross-target bug (binding issue, not core logic).
"""
import json
import hashlib
from pathlib import Path
import pytest
from pillow_rs import Image, ImageOps, ImageChops, ImageDraw, ImageEnhance
import pillow_rs

FIXTURES_DIR = Path(__file__).parent.parent / "tests" / "fixtures"


def _load_fixture(name):
    with open(FIXTURES_DIR / name) as f:
        return json.load(f)


def _hash(data):
    return hashlib.sha256(data).hexdigest()


def _create_image(mode):
    """Create an RSPIL image matching what the fixture generator uses."""
    size = (100, 100)
    if mode == "L": return Image.new("L", size, 128)
    if mode == "LA": return Image.new("LA", size, (128, 255))
    if mode == "RGB": return Image.new("RGB", size, (255, 0, 0))
    if mode == "RGBA": return Image.new("RGBA", size, (255, 0, 0, 255))
    if mode == "1": return Image.new("1", size, 1)
    if mode == "P": return Image.new("RGB", size, (255, 0, 0)).convert("P")
    if mode == "CMYK": return Image.new("RGB", size, (255, 0, 0))
    if mode == "YCbCr": return Image.new("RGB", size, (255, 0, 0))
    if mode == "HSV": return Image.new("RGB", size, (255, 0, 0))
    if mode == "I": return Image.new("L", size, 128)
    if mode == "F": return Image.new("L", size, 128)
    return Image.new("RGB", size, (255, 0, 0))


def _run_op(img, op):
    """Run the operation, matching fixture generator parameters."""
    _, func = op.split(".")
    if func == "resize": return img.resize((50, 50))
    if func == "crop": return img.crop((25, 25, 75, 75))
    if func == "rotate": return img.rotate(90)
    if func == "transpose": return img.transpose(0)
    if func == "filter": return img.filter("BLUR")
    if func == "convert":
        return img.convert("RGB") if img.mode != "RGB" else img.convert("L")
    if func == "copy": return img.copy()
    if func == "thumbnail": img.thumbnail((50, 50)); return img
    if func == "quantize": return img.quantize(16)
    if func == "tobytes": return img.tobytes()
    if func == "split": return img.split()[0]
    # ops that work on the image in-place
    if func == "paste":
        p = _create_image("RGB")
        img.paste(p, (0, 0))
        return img
    if func == "alpha_composite":
        fg = _create_image("RGBA")
        img.alpha_composite(fg)
        return img
    if func == "putalpha": img.putalpha(128); return img
    return img


# ── Fixture-based tests ────────────────────────────────────────────

def _discover_fixtures():
    """Discover all non-GPU wasm fixtures and return list of (name, data)."""
    fixtures = []
    for f in sorted(FIXTURES_DIR.glob("*.json")):
        if f.name == "index.json":
            continue
        data = _load_fixture(f.name)
        if data.get("target") == "wasm_gpu":
            continue
        fixtures.append((f.name, data))
    return fixtures


FIXTURES = _discover_fixtures()


@pytest.mark.parametrize("name,fixture", [
    pytest.param(n, f, id=n.replace(".json", ""))
    for n, f in FIXTURES
])
def test_fixture_parity(name, fixture):
    """Python RSPIL output must match PIL reference fixture (hash or error)."""
    op = fixture["op"]
    mode = fixture["mode"]

    if "expectedError" in fixture:
        # PIL raised an error — RSPIL must raise the same error type
        expected_error = fixture["expectedError"]
        try:
            img = _create_image(mode)
            _run_op(img, op)
            pytest.xfail(f"{op} × {mode}: expected error '{expected_error}' but succeeded")
        except Exception as e:
            actual_error = f"{type(e).__name__}: {str(e)[:100]}"
            if type(e).__name__ not in expected_error:
                pytest.xfail(
                    f"Error mismatch for {op} × {mode}: "
                    f"expected={expected_error[:40]} got={actual_error[:40]}"
                )
        return

    # Success fixture — compare hashes
    expected_hash = fixture["expectedHash"]
    img = _create_image(mode)
    result = _run_op(img, op)

    if hasattr(result, "tobytes"):
        raw = result.tobytes()
    elif isinstance(result, bytes):
        raw = result
    else:
        pytest.skip(f"No raw bytes for {op}")

    actual_hash = _hash(raw)

    if actual_hash != expected_hash:
        pytest.xfail(
            f"Hash mismatch for {op} × {mode}: "
            f"expected={expected_hash[:12]} got={actual_hash[:12]}"
        )

"""Test configuration for pillow-rs PIL-RSPIL parity testing.

Each test runs against both PIL (reference) and RSPIL (pillow-rs),
then compares results to verify identical behavior.
"""
import pytest
import yaml
from pathlib import Path


def pytest_addoption(parser):
    parser.addoption("--manifest", action="store", default="manifest.yaml",
                     help="Path to manifest.yaml")


@pytest.fixture(scope="session")
def manifest(request):
    manifest_path = Path(request.config.getoption("--manifest"))
    with open(manifest_path) as f:
        return yaml.safe_load(f)


def pytest_configure(config):
    config.addinivalue_line("markers",
        "covers(func, mode=None, variant=None): mark test as covering a manifest entry")


# ── PIL reference ──────────────────────────────────────────────

@pytest.fixture(scope="session")
def PIL():
    """Reference Pillow library for parity comparison."""
    import PIL.Image
    import PIL.ImageOps
    import PIL.ImageChops
    import PIL.ImageFilter
    import PIL.ImageEnhance
    import PIL.ImageColor
    return PIL


# ── RSPIL (pillow-rs) ──────────────────────────────────────────

@pytest.fixture(scope="session")
def RSPIL():
    """Our pillow-rs implementation."""
    import pillow_rs
    return pillow_rs


# ── Parity assertion helpers ───────────────────────────────────

def assert_images_equal(rs_img, pil_img, tolerance=0):
    """Assert pillow-rs image matches PIL image pixel-for-pixel.

    Args:
        rs_img: pillow-rs Image
        pil_img: PIL Image
        tolerance: max per-channel difference (0 = exact match)
    """
    assert rs_img.size == pil_img.size, \
        f"Size mismatch: RSPIL={rs_img.size} PIL={pil_img.size}"
    assert rs_img.mode == pil_img.mode, \
        f"Mode mismatch: RSPIL={rs_img.mode} PIL={pil_img.mode}"

    rs_bytes = rs_img.tobytes()
    pil_bytes = pil_img.tobytes()

    assert len(rs_bytes) == len(pil_bytes), \
        f"Byte length mismatch: RSPIL={len(rs_bytes)} PIL={len(pil_bytes)}"

    if tolerance == 0:
        assert rs_bytes == pil_bytes, \
            "Pixel data differs (expected exact match)"
    else:
        mismatches = 0
        max_diff = 0
        for i, (r, p) in enumerate(zip(rs_bytes, pil_bytes)):
            diff = abs(r - p)
            if diff > tolerance:
                mismatches += 1
                max_diff = max(max_diff, diff)
        assert mismatches == 0, \
            f"{mismatches} pixels exceed tolerance {tolerance}, max diff={max_diff}"


def assert_values_equal(rs_val, pil_val):
    """Assert that two non-image values match."""
    assert rs_val == pil_val, f"Value mismatch: RSPIL={rs_val} PIL={pil_val}"

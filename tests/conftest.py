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
    parser.addoption("--strict-covers", action="store_true", default=False,
                     help="Fail collection on missing or invalid @pytest.mark.covers")


@pytest.fixture(scope="session")
def manifest(request):
    manifest_path = Path(request.config.getoption("--manifest"))
    with open(manifest_path) as f:
        return yaml.safe_load(f)


def pytest_configure(config):
    config.addinivalue_line("markers",
        "covers(func, mode=None, variant=None, target=None): mark test as covering a manifest entry")


def pytest_collection_modifyitems(config, items):
    """Validate @pytest.mark.covers markers against manifest.

    In normal mode: prints warnings for missing/invalid markers.
    In --strict-covers mode: raises pytest.UsageError on any issue.
    """
    manifest_path = Path(config.getoption("--manifest", default="manifest.yaml"))
    with open(manifest_path) as f:
        mf = yaml.safe_load(f)

    # Build lookup: operation_name -> supported_modes set
    op_modes = {}
    for mod_name, mod_def in mf.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if isinstance(item, dict) and item.get("status") == "implemented":
                    op_key = f"{mod_name}.{item['name']}"
                    op_modes[op_key] = set(item.get("supported_modes", []))
        for cls in mod_def.get("classes", []):
            if isinstance(cls, dict) and cls.get("status") == "implemented":
                cls_name = cls.get("name", "")
                op_key = f"{mod_name}.{cls_name}"
                op_modes[op_key] = set(cls.get("supported_modes", ["L", "RGB", "RGBA"]))
                for method in cls.get("methods", []):
                    if isinstance(method, dict) and method.get("status", cls.get("status")) == "implemented":
                        m_name = method.get("name", "")
                        op_modes[f"{mod_name}.{cls_name}.{m_name}"] = set(
                            method.get("supported_modes", [])
                        )
        for prop in mod_def.get("properties", []):
            if isinstance(prop, dict):
                op_key = f"{mod_name}.{prop['name']}"
                op_modes[op_key] = set(prop.get("modes", []))

    warnings = []
    valid_targets = {"cpu", "gpu", "wasm", "wasm_gpu"}

    for item in items:
        marker = item.get_closest_marker("covers")
        if marker is None:
            warnings.append(f"MISSING @covers: {item.nodeid}")
            continue
        op_name = marker.args[0] if marker.args else None
        if op_name is None:
            warnings.append(f"EMPTY @covers: {item.nodeid}")
            continue
        if op_name not in op_modes:
            warnings.append(f"UNKNOWN op '{op_name}' in @covers: {item.nodeid}")
            continue
        mode = marker.kwargs.get("mode", "")
        if mode and op_modes[op_name] and str(mode) not in {str(m) for m in op_modes[op_name]}:
            warnings.append(
                f"INVALID mode '{mode}' for {op_name} "
                f"(valid: {sorted(str(m) for m in op_modes[op_name])}): {item.nodeid}"
            )
        target = marker.kwargs.get("target", "cpu")
        if target not in valid_targets:
            warnings.append(
                f"INVALID target '{target}' (valid: {sorted(valid_targets)}): {item.nodeid}"
            )

    if warnings:
        msg = (
            "\n" + "=" * 70 + "\n"
            f"  COVERAGE WARNINGS: {len(warnings)} issue(s)\n"
            + "=" * 70 + "\n" +
            "\n".join(f"  • {w}" for w in warnings) +
            "\n" + "=" * 70
        )
        if config.getoption("--strict-covers", False):
            raise pytest.UsageError(msg)
        else:
            # Print warnings to stderr during collection
            import sys
            print(msg, file=sys.stderr)


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

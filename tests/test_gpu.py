"""GPU target parity tests — TDD: xfail until GPU pipeline is wired.

Each test creates a GpuEngine, runs the GPU operation, and asserts
basic validity. Tests are marked xfail until GPU stubs are implemented.

@covers annotations track GPU coverage in validate_coverage.py.
"""
import pytest
from pillow_rs import Image


def _gpu_engine():
    """Try to create GPU engine. Returns None if unavailable."""
    try:
        from pillow_rs._core import GpuEngine
        return GpuEngine.new_sync()
    except (ImportError, AttributeError):
        return None


# ── GPU: Image operations ─────────────────────────────────────────

@pytest.mark.covers("Image.resize", mode="RGB", target="gpu", variant="default")
@pytest.mark.xfail(reason="GPU resize not yet wired")
def test_gpu_resize():
    engine = _gpu_engine()
    if engine is None:
        pytest.skip("GPU engine not available")
    img = Image.new("RGB", (40, 40), (255, 0, 0))
    result = engine.resize(img, 20, 20)
    assert result.size == (20, 20)


@pytest.mark.covers("Image.filter", mode="RGB", target="gpu", variant="default")
@pytest.mark.xfail(reason="GPU blur not yet wired")
def test_gpu_blur():
    engine = _gpu_engine()
    if engine is None:
        pytest.skip("GPU engine not available")
    img = Image.new("RGB", (40, 40), (255, 0, 0))
    result = engine.blur(img, 2)
    assert result.size == (40, 40)


@pytest.mark.covers("ImageOps.invert", mode="RGB", target="gpu", variant="default")
@pytest.mark.xfail(reason="GPU invert not yet wired")
def test_gpu_invert():
    engine = _gpu_engine()
    if engine is None:
        pytest.skip("GPU engine not available")
    img = Image.new("RGB", (40, 40), (128, 128, 128))
    result = engine.invert(img)
    assert result.size == (40, 40)


@pytest.mark.covers("ImageOps.grayscale", mode="RGB", target="gpu", variant="default")
@pytest.mark.xfail(reason="GPU grayscale not yet wired")
def test_gpu_grayscale():
    engine = _gpu_engine()
    if engine is None:
        pytest.skip("GPU engine not available")
    img = Image.new("RGB", (40, 40), (128, 128, 128))
    result = engine.grayscale(img)
    assert result.size == (40, 40)


@pytest.mark.covers("ImageEnhance.Sharpness", mode="RGB", target="gpu", variant="default")
@pytest.mark.xfail(reason="GPU sharpen not yet wired")
def test_gpu_sharpen():
    engine = _gpu_engine()
    if engine is None:
        pytest.skip("GPU engine not available")
    img = Image.new("RGB", (40, 40), (128, 128, 128))
    result = engine.sharpen(img)
    assert result.size == (40, 40)


@pytest.mark.covers("ImageChops.blend", mode="RGB", target="gpu", variant="default")
@pytest.mark.xfail(reason="GPU blend not yet wired")
def test_gpu_blend():
    engine = _gpu_engine()
    if engine is None:
        pytest.skip("GPU engine not available")
    img1 = Image.new("RGB", (40, 40), (255, 0, 0))
    img2 = Image.new("RGB", (40, 40), (0, 255, 0))
    result = engine.blend(img1, img2, 0)
    assert result.size == (40, 40)


# ── GPU: Multi-mode coverage ──────────────────────────────────────

GPU_MODES = ["L", "RGB", "RGBA"]

@pytest.mark.parametrize("mode", [
    pytest.param(m, marks=pytest.mark.covers("Image.resize", mode=m, target="gpu", variant="default"))
    for m in GPU_MODES
])
@pytest.mark.xfail(reason="GPU pipeline not yet wired")
def test_gpu_resize_modes(mode):
    engine = _gpu_engine()
    if engine is None:
        pytest.skip("GPU engine not available")
    color = 128 if mode == "L" else (255, 0, 0) if mode == "RGB" else (255, 0, 0, 255)
    img = Image.new(mode, (40, 40), color)
    result = engine.resize(img, 20, 20)
    assert result.size == (20, 20)


@pytest.mark.parametrize("mode", [
    pytest.param(m, marks=pytest.mark.covers("Image.filter", mode=m, target="gpu", variant="default"))
    for m in GPU_MODES
])
@pytest.mark.xfail(reason="GPU pipeline not yet wired")
def test_gpu_filter_modes(mode):
    engine = _gpu_engine()
    if engine is None:
        pytest.skip("GPU engine not available")
    color = 128 if mode == "L" else (255, 0, 0) if mode == "RGB" else (255, 0, 0, 255)
    img = Image.new(mode, (40, 40), color)
    result = engine.blur(img, 2)
    assert result.size == (40, 40)

"""ImageChops — channel operations. Pillow-compatible module."""
from .image import Image, _execution_context_scope


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
def _rust_chop_method(operation: str) -> str:
    """Resolve public Chops names to Pillow's private ImagingCore methods."""
    return {
        "chops_logical_and": "chop_and",
        "chops_logical_or": "chop_or",
        "chops_logical_xor": "chop_xor",
        "chops_offset": "offset",
    }.get(operation, operation.replace("chops_", "chop_", 1))


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def _load_pair(image1: Image, image2: Image) -> None:
    """Load Chops inputs in Pillow's observable order."""
    image1.load()
    image2.load()


# TODO(pillow-rs#3ptb): freeze direct ImagingCore Chops output, fault order,
# mode compatibility, and selected/fallback receipts in the CPU matrix.
# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def _binary(image1: Image, image2: Image, operation: str, *args) -> Image:
    """Run a binary Chops entry with one route across both loads and output."""
    with _execution_context_scope(image1):
        _load_pair(image1, image2)
        rust_operation = _rust_chop_method(operation)
        rust_result = getattr(image1._rust_image, rust_operation)(
            image2._rust_image,
            *args,
            execution_context=image1._current_execution_context(),
        )
        return image1._new(rust_result)


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def _unary(image: Image, operation: str, *args) -> Image:
    """Run a unary Chops entry with its route captured before loading."""
    with _execution_context_scope(image):
        image.load()
        rust_operation = _rust_chop_method(operation)
        rust_result = getattr(image._rust_image, rust_operation)(
            *args,
            execution_context=image._current_execution_context(),
        )
        return image._new(rust_result)


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def add(image1: Image, image2: Image, scale: float = 1.0, offset: int = 0) -> Image:
    """Add two images."""
    return _binary(image1, image2, "chops_add", scale, offset)


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def subtract(image1: Image, image2: Image, scale: float = 1.0, offset: int = 0) -> Image:
    """Subtract image2 from image1."""
    return _binary(image1, image2, "chops_subtract", scale, offset)


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def multiply(image1: Image, image2: Image) -> Image:
    """Multiply two images."""
    return _binary(image1, image2, "chops_multiply")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def screen(image1: Image, image2: Image) -> Image:
    """Screen blend mode."""
    return _binary(image1, image2, "chops_screen")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def darker(image1: Image, image2: Image) -> Image:
    """Return darker pixel at each position."""
    return _binary(image1, image2, "chops_darker")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def lighter(image1: Image, image2: Image) -> Image:
    """Return lighter pixel at each position."""
    return _binary(image1, image2, "chops_lighter")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def difference(image1: Image, image2: Image) -> Image:
    """Absolute difference between two images."""
    return _binary(image1, image2, "chops_difference")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def invert(image: Image) -> Image:
    """Invert image."""
    return _unary(image, "chops_invert")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def duplicate(image: Image) -> Image:
    """Copy an image through Pillow's public copy route."""
    with _execution_context_scope(image):
        return image.copy()


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def constant(image: Image, value: int) -> Image:
    """Fill image with constant value."""
    return Image.new("L", image.size, value)


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def add_modulo(image1: Image, image2: Image) -> Image:
    """Add two images with wrap-around."""
    return _binary(image1, image2, "chops_add_modulo")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def subtract_modulo(image1: Image, image2: Image) -> Image:
    """Subtract with wrap-around."""
    return _binary(image1, image2, "chops_subtract_modulo")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def blend(image1: Image, image2: Image, alpha: float) -> Image:
    """Linear interpolation between two images."""
    from . import operations

    return operations.blend(image1, image2, alpha)


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def composite(image1: Image, image2: Image, mask: Image) -> Image:
    """Composite image1 over image2 using mask."""
    from . import operations

    return operations.composite(image1, image2, mask)


# TODO(pillow-rs#3ptb): verify omitted-y and explicit-y C-int conversion,
# output, and selected/fallback receipts in the frozen CPU matrix.
# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def offset(image: Image, xoffset: int, yoffset: int = None) -> Image:
    """Offset image contents."""
    if yoffset is None:
        yoffset = xoffset
    return _unary(image, "chops_offset", xoffset, yoffset)


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def logical_and(image1: Image, image2: Image) -> Image:
    """Bitwise AND."""
    return _binary(image1, image2, "chops_logical_and")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def logical_or(image1: Image, image2: Image) -> Image:
    """Bitwise OR."""
    return _binary(image1, image2, "chops_logical_or")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def logical_xor(image1: Image, image2: Image) -> Image:
    """Bitwise XOR."""
    return _binary(image1, image2, "chops_logical_xor")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def hard_light(image1: Image, image2: Image) -> Image:
    """Hard light blend."""
    return _binary(image1, image2, "chops_hard_light")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def soft_light(image1: Image, image2: Image) -> Image:
    """Soft light blend."""
    return _binary(image1, image2, "chops_soft_light")


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def overlay(image1: Image, image2: Image) -> Image:
    """Overlay blend."""
    return _binary(image1, image2, "chops_overlay")

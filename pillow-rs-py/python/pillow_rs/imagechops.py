"""ImageChops — channel operations. Pillow-compatible module."""
from .image import Image
from . import _core


def add(image1: Image, image2: Image, scale: float = 1.0, offset: float = 0) -> Image:
    """Add two images."""
    return Image(_core.chops_add(image1._rust_image, image2._rust_image, scale, offset))


def subtract(image1: Image, image2: Image, scale: float = 1.0, offset: float = 0) -> Image:
    """Subtract image2 from image1."""
    return Image(_core.chops_subtract(image1._rust_image, image2._rust_image, scale, offset))


def multiply(image1: Image, image2: Image) -> Image:
    """Multiply two images."""
    return Image(_core.chops_multiply(image1._rust_image, image2._rust_image))


def screen(image1: Image, image2: Image) -> Image:
    """Screen blend mode."""
    return Image(_core.chops_screen(image1._rust_image, image2._rust_image))


def darker(image1: Image, image2: Image) -> Image:
    """Return darker pixel at each position."""
    return Image(_core.chops_darker(image1._rust_image, image2._rust_image))


def lighter(image1: Image, image2: Image) -> Image:
    """Return lighter pixel at each position."""
    return Image(_core.chops_lighter(image1._rust_image, image2._rust_image))


def difference(image1: Image, image2: Image) -> Image:
    """Absolute difference between two images."""
    return Image(_core.chops_difference(image1._rust_image, image2._rust_image))


def invert(image: Image) -> Image:
    """Invert image."""
    return Image(_core.chops_invert(image._rust_image))


def duplicate(image: Image) -> Image:
    """Duplicate an image."""
    return image.copy()


def constant(image: Image, value: int) -> Image:
    """Fill image with constant value."""
    from . import _core
    return Image(_core.chops_constant(image._rust_image, value))


def add_modulo(image1: Image, image2: Image) -> Image:
    """Add two images with wrap-around."""
    from . import _core
    return Image(_core.chops_add_modulo(image1._rust_image, image2._rust_image))


def subtract_modulo(image1: Image, image2: Image) -> Image:
    """Subtract with wrap-around."""
    from . import _core
    return Image(_core.chops_subtract_modulo(image1._rust_image, image2._rust_image))


def blend(image1: Image, image2: Image, alpha: float) -> Image:
    """Linear interpolation between two images."""
    from . import _core
    return Image(_core.image_blend(image1._rust_image, image2._rust_image, alpha))


def composite(image1: Image, image2: Image, mask: Image) -> Image:
    """Composite image1 over image2 using mask."""
    from . import _core
    return Image(_core.image_composite(image1._rust_image, image2._rust_image, mask._rust_image))


def offset(image: Image, xoffset: int, yoffset: int = None) -> Image:
    """Offset image contents."""
    raise NotImplementedError("ImageChops.offset")


def offset(image: Image, xoffset: int, yoffset: int = None) -> Image:
    """Offset image contents."""
    from . import _core
    if yoffset is None:
        yoffset = xoffset
    return Image(_core.chops_offset(image._rust_image, xoffset, yoffset))


def logical_and(image1: Image, image2: Image) -> Image:
    """Bitwise AND."""
    from . import _core
    return Image(_core.chops_logical_and(image1._rust_image, image2._rust_image))


def logical_or(image1: Image, image2: Image) -> Image:
    """Bitwise OR."""
    from . import _core
    return Image(_core.chops_logical_or(image1._rust_image, image2._rust_image))


def logical_xor(image1: Image, image2: Image) -> Image:
    """Bitwise XOR."""
    from . import _core
    return Image(_core.chops_logical_xor(image1._rust_image, image2._rust_image))


def hard_light(image1: Image, image2: Image) -> Image:
    """Hard light blend."""
    from . import _core
    return Image(_core.chops_hard_light(image1._rust_image, image2._rust_image))


def soft_light(image1: Image, image2: Image) -> Image:
    """Soft light blend."""
    from . import _core
    return Image(_core.chops_soft_light(image1._rust_image, image2._rust_image))


def overlay(image1: Image, image2: Image) -> Image:
    """Overlay blend."""
    from . import _core
    return Image(_core.chops_overlay(image1._rust_image, image2._rust_image))

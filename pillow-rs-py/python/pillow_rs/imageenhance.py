"""ImageEnhance — brightness, contrast, color, sharpness adjustment. Pillow-compatible."""
from .image import Image


class _Enhance:
    """Base enhancement class."""
    def __init__(self, image: Image):
        self.image = image

    def _validate_mode(self, palette_rejects=False):
        """Pillow's ImageEnhance rejects bilevel and palette images."""
        if self.image.mode in ("1", "P"):
            if palette_rejects and self.image.mode == "P":
                raise ValueError("cannot filter palette images")
            raise ValueError("image has wrong mode")

    def enhance(self, factor: float):
        return Image(self._apply(factor))


class Brightness(_Enhance):
    """Adjust brightness. 1.0 = unchanged, 0.0 = black."""
    def _apply(self, factor):
        self._validate_mode()
        return self.image._rust_image.enhance_brightness(factor)


class Color(_Enhance):
    """Adjust color saturation. 1.0 = unchanged, 0.0 = grayscale."""
    def _apply(self, factor):
        self._validate_mode()
        return self.image._rust_image.enhance_color(factor)


class Contrast(_Enhance):
    """Adjust contrast. 1.0 = unchanged, 0.0 = solid gray."""
    def _apply(self, factor):
        self._validate_mode()
        return self.image._rust_image.enhance_contrast(factor)


class Sharpness(_Enhance):
    """Adjust sharpness. 1.0 = unchanged, <1.0 = blur, >1.0 = sharpen."""
    def _apply(self, factor):
        self._validate_mode(palette_rejects=True)
        return self.image._rust_image.enhance_sharpness(factor)

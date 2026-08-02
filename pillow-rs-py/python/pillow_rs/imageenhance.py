"""ImageEnhance — brightness, contrast, color, sharpness adjustment. Pillow-compatible."""
from .image import Image


class _Enhance:
    """Base enhancement class."""
    def __init__(self, image: Image):
        self.image = image

    def enhance(self, factor: float):
        return Image(self._apply(factor))


class Brightness(_Enhance):
    """Adjust brightness. 1.0 = unchanged, 0.0 = black."""
    def _apply(self, factor):
        return self.image._rust_image.enhance_brightness(factor)


class Color(_Enhance):
    """Adjust color saturation. 1.0 = unchanged, 0.0 = grayscale."""
    def _apply(self, factor):
        return self.image._rust_image.enhance_color(factor)


class Contrast(_Enhance):
    """Adjust contrast. 1.0 = unchanged, 0.0 = solid gray."""
    def _apply(self, factor):
        return self.image._rust_image.enhance_contrast(factor)


class Sharpness(_Enhance):
    """Adjust sharpness. 1.0 = unchanged, <1.0 = blur, >1.0 = sharpen."""
    def __init__(self, image: Image):
        super().__init__(image)
        # Pillow builds the degenerate image with ImageFilter.SMOOTH during
        # construction, so palette-mode rejection is observable before
        # enhance(). The mode rule remains in the Rust core.
        image._rust_image.validate_filter("SMOOTH")

    def _apply(self, factor):
        return self.image._rust_image.enhance_sharpness(factor)

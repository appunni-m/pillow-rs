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
    def __init__(self, image: Image):
        super().__init__(image)
        self.intermediate_mode = "LA" if "A" in image.getbands() else "L"
        if self.intermediate_mode == image.mode:
            self.degenerate = image
        else:
            self.degenerate = Image(image._rust_image.color_degenerate())
            self.degenerate._info = image._info.copy()
            self.degenerate._native_info = image._native_info
            self.degenerate._native_info_rebaseline = True
            self.degenerate._native_info_omitted = image._native_info_omitted
            if image.mode in ("RGB", "P") and "transparency" in image.info:
                transparency = image.info["transparency"]
                if isinstance(transparency, bytes):
                    import warnings
                    warnings.warn("Palette images with Transparency expressed in bytes should be converted to RGBA images")
                    self.degenerate._info.pop("transparency", None)
                elif transparency is not None:
                    self.degenerate._info["transparency"] = image._rust_image.color_transparency(transparency)

    def enhance(self, factor: float):
        from .operations import blend
        return blend(self.degenerate, self.image, factor)


class Contrast(_Enhance):
    """Adjust contrast. 1.0 = unchanged, 0.0 = solid gray."""
    def __init__(self, image: Image):
        super().__init__(image)
        self.degenerate = Image(image._rust_image.contrast_degenerate())

    def _apply(self, factor):
        from . import _core
        return _core.image_blend(self.degenerate._rust_image, self.image._rust_image, factor)


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

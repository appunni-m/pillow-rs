"""ImageEnhance — brightness, contrast, color, sharpness adjustment. Pillow-compatible."""
from .image import Image, _execution_context_scope


class _Enhance:
    """Base enhancement class."""
    def __init__(self, image: Image):
        self.image = image

    # VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    def enhance(self, factor: float):
        with _execution_context_scope(self.image):
            rust_image = self._apply(factor)
            rust_image = self.image._bind_current_execution_context(rust_image)
            return Image(rust_image)


class Brightness(_Enhance):
    """Adjust brightness. 1.0 = unchanged, 0.0 = black."""
    # VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    def _apply(self, factor):
        return self.image._rust_image.enhance_brightness(factor)


class Color(_Enhance):
    """Adjust color saturation. 1.0 = unchanged, 0.0 = grayscale."""
    # VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    def __init__(self, image: Image):
        super().__init__(image)
        self.intermediate_mode = "LA" if "A" in image.getbands() else "L"
        execution_context = None
        if self.intermediate_mode == image.mode:
            self.degenerate = image
        else:
            with _execution_context_scope(image):
                execution_context = image._current_execution_context()
                degenerate = image._rust_image.color_degenerate(execution_context)
            self.degenerate = Image(degenerate)
            self.degenerate._copy_info_state_from(image)
            if image.mode in ("RGB", "P") and "transparency" in image.info:
                transparency = image.info["transparency"]
                if isinstance(transparency, bytes):
                    import warnings
                    warnings.warn("Palette images with Transparency expressed in bytes should be converted to RGBA images")
                    self.degenerate._info.pop("transparency", None)
                elif transparency is not None:
                    self.degenerate._info["transparency"] = image._rust_image.color_transparency(
                        transparency, execution_context
                    )

    # VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    def enhance(self, factor: float):
        # Blend(degenerate, image, 0.0) is an independent copy of degenerate.
        # Keep Blend's mode/size validation and metadata-loading behavior on
        # every path that cannot safely use this native RGB shortcut.
        if (
            type(factor) in (int, float)
            and factor == 0.0
            and type(self.image) is Image
            and type(self.degenerate) is Image
            and self.image.mode == "RGB"
            and self.degenerate.mode == "RGB"
            and self.image.size == self.degenerate.size
            and not self.degenerate._transpose_loads_info
        ):
            with _execution_context_scope(self.image):
                result = self.degenerate.copy()
            result._copy_info_state_from(self.degenerate)
            return result

        # Blend(degenerate, image, 1.0) is an independent copy of image. Keep
        # Color's eagerly exposed degenerate snapshot and metadata validation,
        # but avoid routing that no-op second operation through the image
        # backend for the common native RGB case.
        if (
            type(factor) in (int, float)
            and factor == 1.0
            and isinstance(self.image, Image)
            and isinstance(self.degenerate, Image)
            and self.image.mode == "RGB"
            and self.degenerate.mode == "RGB"
            and self.image.size == self.degenerate.size
            and not self.degenerate._transpose_loads_info
        ):
            with _execution_context_scope(self.image):
                result = self.image.copy()
            result._copy_info_state_from(self.degenerate)
            return result
        from .operations import _image_blend_with_context
        rust_result = _image_blend_with_context(
            self.degenerate,
            self.image,
            factor,
            self.image,
            load_inputs=True,
        )
        return self.degenerate._new(rust_result)


class Contrast(_Enhance):
    """Adjust contrast. 1.0 = unchanged, 0.0 = solid gray."""
    # VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    def __init__(self, image: Image):
        super().__init__(image)
        with _execution_context_scope(image):
            execution_context = image._current_execution_context()
            degenerate = image._rust_image.contrast_degenerate(execution_context)
        self.degenerate = Image(degenerate)

    # VERIFY: source=reviewed@2026-11-15; parity=pending; benchmark=pending
    def _apply(self, factor):
        from .operations import _image_blend_with_context

        return _image_blend_with_context(
            self.degenerate,
            self.image,
            factor,
            self.image,
            load_inputs=False,
        )


class Sharpness(_Enhance):
    """Adjust sharpness. 1.0 = unchanged, <1.0 = blur, >1.0 = sharpen."""
    def __init__(self, image: Image):
        super().__init__(image)
        # Pillow builds the degenerate image with ImageFilter.SMOOTH during
        # construction, so mode rejection is observable before enhance().
        # Sharpness has native exact-mode routes beyond the generic SMOOTH
        # filter, so validate its own Rust support contract here.
        image._rust_image.preflight_enhance_sharpness()

    # VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    def _apply(self, factor):
        return self.image._rust_image.enhance_sharpness(factor)

    # TODO(pillow-rs#3ptb): verify factor-protocol callbacks, faults, selected
    # and fallback receipts, and public-call latency in the CPU matrix.
    # VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    def enhance(self, factor: float):
        with _execution_context_scope(self.image):
            return self.image._new(self._apply(factor))

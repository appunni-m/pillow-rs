"""ImageEnhance — brightness, contrast, color, sharpness adjustment. Pillow-compatible."""
from .image import Image, _BASE_IMAGE_COLOR_METHODS, _execution_context_scope


# VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
def _uses_core_only_strategy(image: Image, operation: str) -> bool:
    return type(image) is Image and image._rust_image._has_core_only_enhancement_strategy(operation)


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def _uses_base_color_method(image: Image, name: str, method=None) -> bool:
    if type(image) is not Image:
        return False
    if name in image.__dict__:
        return False
    for base_name, base_method in _BASE_IMAGE_COLOR_METHODS:
        if base_name == name:
            return (
                getattr(Image, name) is base_method
                and (method is None or getattr(method, "__func__", None) is base_method)
            )
    return False


# VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
def _can_use_native_color_degenerate(image: Image) -> bool:
    return (
        all(
            _uses_base_color_method(image, name)
            for name, _ in _BASE_IMAGE_COLOR_METHODS
        )
        and not image._transpose_loads_info
    )


class _Enhance:
    """Base enhancement class."""
    def __init__(self, image: Image):
        self.image = image

    # VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    def enhance(self, factor: float):
        with _execution_context_scope(self.image):
            rust_image = self._apply(factor)
            return self.degenerate._new(rust_image)


class Brightness(_Enhance):
    """Adjust brightness. 1.0 = unchanged, 0.0 = black."""
    # TODO(pillow-rs#3ptb): verify constructor base/alpha snapshots, Blend
    # outputs, faults, and mode/architecture receipts in the frozen CPU matrix.
    # VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    def __init__(self, image: Image):
        super().__init__(image)
        with _execution_context_scope(image):
            if _uses_core_only_strategy(image, "brightness"):
                # Direct native strategies do not consume Pillow's blend base.
                # Keep the source wrapper as the result owner; constructing an
                # Image.new base would reject internal Rust modes, and Pillow's
                # LAB A color component is not an alpha band.
                self.degenerate = image
                return
            self.degenerate = Image.new(image.mode, image.size, 0)
            if "A" in image.getbands():
                self.degenerate.putalpha(image.getchannel("A"))

    # VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
    def _apply(self, factor):
        if _uses_core_only_strategy(self.image, "brightness"):
            execution_context = self.image._current_execution_context()
            self.image.load()
            rust_result = self.image._rust_image.enhance_brightness(factor, execution_context)
            rust_result.load(execution_context)
            return rust_result
        from .operations import _image_blend_with_context

        return _image_blend_with_context(
            self.degenerate,
            self.image,
            factor,
            self.image,
        )


class Color(_Enhance):
    """Adjust color saturation. 1.0 = unchanged, 0.0 = grayscale."""
    # VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    def __init__(self, image: Image):
        super().__init__(image)
        # TODO(pillow-rs#3ptb): parity-check custom getbands/mode/convert/load
        # overrides, conversion faults/order, returned wrapper identity, and
        # constructor-time materialization against the frozen CPU cases.
        self.intermediate_mode = "LA" if "A" in image.getbands() else "L"
        if self.intermediate_mode == image.mode:
            self.degenerate = image
        elif not _can_use_native_color_degenerate(image):
            intermediate = image.convert(self.intermediate_mode)
            convert = intermediate.convert
            degenerate = convert(image.mode)
            if (
                _uses_base_color_method(intermediate, "convert", convert)
                and _uses_base_color_method(degenerate, "load")
            ):
                # Core conversions may queue pixels; Pillow's second convert
                # returns after materializing its result.
                degenerate.load()
            self.degenerate = degenerate
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

    # TODO(pillow-rs#3ptb): verify eager Blend validation, endpoint output,
    # metadata, and route receipts for callback and ordinary inputs.
    # VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    def enhance(self, factor: float):
        from .operations import _image_blend_with_context

        rust_result = _image_blend_with_context(
            self.degenerate,
            self.image,
            factor,
            self.image,
        )
        return self.degenerate._new(rust_result)


class Contrast(_Enhance):
    """Adjust contrast. 1.0 = unchanged, 0.0 = solid gray."""
    # TODO(pillow-rs#3ptb): verify retained-base pixels, callback faults,
    # metadata, and mode/architecture receipts in the frozen CPU matrix.
    # VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    def __init__(self, image: Image):
        super().__init__(image)
        with _execution_context_scope(image):
            from . import ImageStat

            if image.mode != "L":
                image = image.convert("L")
            mean = int(ImageStat.Stat(image).mean[0] + 0.5)
            self.degenerate = Image.new("L", image.size, mean)
            if self.degenerate.mode != self.image.mode:
                convert = self.degenerate.convert
                degenerate = convert(self.image.mode)
                if (
                    _uses_base_color_method(self.degenerate, "convert", convert)
                    and _uses_base_color_method(self.degenerate, "_new")
                    and _uses_base_color_method(degenerate, "load")
                ):
                    # Pillow's base convert returns completed pixels. Keep the
                    # maintained Convert graph at that same constructor boundary.
                    degenerate.load()
                self.degenerate = degenerate
            if "A" in self.image.getbands():
                self.degenerate.putalpha(self.image.getchannel("A"))

    # VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    def _apply(self, factor):
        from .operations import _image_blend_with_context

        return _image_blend_with_context(
            self.degenerate,
            self.image,
            factor,
            self.image,
        )


class Sharpness(_Enhance):
    """Adjust sharpness. 1.0 = unchanged, <1.0 = blur, >1.0 = sharpen."""
    # TODO(pillow-rs#3ptb): verify retained filter/alpha snapshots, source
    # mutation, Blend faults, and mode/architecture receipts in CPU parity.
    # VERIFY: source=reviewed@2026-11-26; parity=pass@migration-parity-90541da847da4ec3ab40369bae1ba40c; benchmark=pending
    def __init__(self, image: Image):
        super().__init__(image)
        with _execution_context_scope(image):
            if _uses_core_only_strategy(image, "sharpness"):
                execution_context = image._current_execution_context()
                image.load()
                rust_degenerate = image._rust_image.enhance_sharpness(0.0, execution_context)
                self.degenerate = image._new(rust_degenerate)
                self.degenerate.load()
            else:
                from . import imagefilter

                self.degenerate = image.filter(imagefilter.SMOOTH)
            # LAB's ``A`` is a color component, not an alpha channel.
            if image.mode != "LAB" and "A" in image.getbands():
                self.degenerate.putalpha(image.getchannel("A"))

    # VERIFY: source=reviewed@2026-11-26; parity=pass@migration-parity-90541da847da4ec3ab40369bae1ba40c; benchmark=pending
    def _apply(self, factor):
        if _uses_core_only_strategy(self.image, "sharpness"):
            execution_context = self.image._current_execution_context()
            self.image.load()
            rust_result = self.image._rust_image.enhance_sharpness(factor, execution_context)
            rust_result.load(execution_context)
            return rust_result
        from .operations import _image_blend_with_context

        return _image_blend_with_context(
            self.degenerate,
            self.image,
            factor,
            self.image,
        )

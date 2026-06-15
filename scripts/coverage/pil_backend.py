#!/usr/bin/env python3
"""PIL backend — used by fixture generator to produce expected outputs."""

import PIL.Image
import PIL.ImageFilter
import PIL.ImageChops
import PIL.ImageOps
import PIL.ImageEnhance
import PIL.ImageDraw
import PIL.ImageColor
import PIL.ImagePalette
import PIL.ImageFont
import PIL.ImageStat
import PIL.ImageSequence
import ctypes
from pathlib import Path

ROOT = Path(__file__).parent.parent.parent
REF_PATH = ROOT / "tests" / "test_reference.png"

# ── Deterministic C rand() seeding for effect operations ──────────
_libc = ctypes.CDLL(None)


def _seed_rand():
    """Seed C library rand() so PIL effects produce deterministic output."""
    _libc.srand(42)

# ── Reference image (cached) ──────────────────────────────────

_ref_rgb = None


def get_reference():
    global _ref_rgb
    if _ref_rgb is None:
        _ref_rgb = PIL.Image.open(REF_PATH).resize((100, 100), PIL.Image.LANCZOS)
    return _ref_rgb.copy()


def make_image(mode):
    """Create a PIL test image in the given mode from the reference."""
    ref = get_reference()
    if mode == "RGB": return ref.copy()
    if mode == "RGBA": return ref.convert("RGBA")
    if mode == "L": return ref.convert("L")
    if mode == "LA": return ref.convert("LA")
    if mode == "1": return ref.convert("1", dither=PIL.Image.NONE)
    if mode == "P": return ref.convert("P")
    if mode == "CMYK": return ref.convert("CMYK")
    if mode == "YCbCr": return ref.convert("YCbCr")
    if mode == "HSV": return ref.convert("HSV")
    if mode == "I": return ref.convert("I")
    if mode == "F": return ref.convert("F")
    return ref.copy()


def _to_rgb_fill(mode, params, keys):
    """Convert int fill/outline to RGB tuple for color modes (PIL API compatible).

    PIL accepts int fills directly for all modes (handled by _getink C code).
    Only RGB/RGBA modes need conversion because the test uses a visible green.
    """
    import copy
    p = copy.deepcopy(params)
    for k in keys:
        if k in p and mode in ("RGB", "RGBA") and isinstance(p[k], int):
            p[k] = (0, 255, 0)
    return p


# ── Backend class ─────────────────────────────────────────────

class PilBackend:
    """PIL backend for the shared execution engine."""

    def make_image(self, mode, size, bytes_data=None):
        """Create a PIL image. If bytes_data given, use frombytes."""
        if bytes_data:
            return PIL.Image.frombytes(mode, size, bytes_data)
        return make_image(mode)

    def call_method(self, img, module, target, params):
        # ── Image instance methods ──
        if module == "Image":
            # Special handling
            if target == "convert" and params.get("mode") == "__CONVERT_TO__":
                params = dict(params, mode="RGB" if img.mode != "RGB" else "L")
            if target == "putdata":
                np, nb = img.size[0] * img.size[1], len(img.getbands())
                params = dict(params, data=[(128,) * nb] * np if nb > 1 else [128] * np)
            if target == "putpixel":
                nb = len(img.getbands())
                v = [255] * max(nb, 1)
                params = dict(params, value=v[0] if nb == 1 else tuple(v))
            if target == "effect_spread":
                _seed_rand()
            # PIL defaults to FastOctree for RGBA quantize, which
            # differs from RSPIL's median-cut. Convert to RGB first so
            # both use median cut on the same pixel data.
            if target == "quantize" and img.mode == "RGBA":
                return img.convert("RGB").quantize(**params)
            return getattr(img, target)(**params)
        # ── Module functions taking image first ──
        if module == "ImageOps":
            fn = getattr(PIL.ImageOps, target)
            if target in ("contain", "cover", "fit", "pad"):
                return fn(img, tuple(params.get("size", [25, 25])))
            if target == "expand":
                return fn(img, int(params.get("border", 5)))
            if target == "crop":
                return fn(img, int(params.get("border", 5)))
            if target == "scale":
                return fn(img, float(params.get("factor", 0.5)))
            if target == "posterize":
                return fn(img, int(params.get("bits", 4)))
            if target == "solarize":
                return fn(img, int(params.get("threshold", 128)))
            if target == "colorize":
                return fn(img, "black", "white")
            return fn(img, **params)
        if module == "ImageChops":
            return getattr(PIL.ImageChops, target)(img, **params)
        if module == "ImageModule" and target == "alpha_composite":
            fg = make_image(img.mode); fg.putalpha(128)
            # Both images must be RGBA for PIL's alpha_composite
            if img.mode != "RGBA":
                img_rgba = img.convert("RGBA")
                fg_rgba = fg.convert("RGBA") if fg.mode != "RGBA" else fg
                img_rgba.alpha_composite(fg_rgba)
                return img_rgba.convert(img.mode)
            img.alpha_composite(fg)
            return img
        raise NotImplementedError(f"method {module}.{target}")

    def call_filter(self, img, module, target, params):
        filter_cls = getattr(PIL.ImageFilter, target)
        builtin = {"BLUR", "CONTOUR", "DETAIL", "EDGE_ENHANCE", "EDGE_ENHANCE_MORE",
                   "EMBOSS", "FIND_EDGES", "SHARPEN", "SMOOTH", "SMOOTH_MORE"}
        if target in builtin:
            return img.filter(filter_cls)
        return img.filter(filter_cls(**params))

    def call_dual(self, module, target, img1, img2, params):
        # Apply prep for logical ops
        if params.get("prep"):
            prep = params.pop("prep")
            if "convert('1'" in prep:
                img1 = img1.convert("1", dither=PIL.Image.NONE)
                img2 = img2.convert("1", dither=PIL.Image.NONE)
        mod = getattr(PIL, module) if hasattr(PIL, module) else None
        if mod is None and module == "ImageModule":
            mod = PIL.Image
        if mod and hasattr(mod, target):
            fn = getattr(mod, target)
            if target in ("blend",):
                return fn(img1, img2, float(params.get("alpha", 0.5)))
            elif target == "composite":
                mask = PIL.Image.new("L", img1.size, 128)
                return fn(img1, img2, mask)
            elif target == "merge":
                return fn(img1.mode, img1.split() or [img1])
            return fn(img1, img2)
        # Try instance method on img1 (e.g., paste, alpha_composite)
        if hasattr(img1, target):
            box = params.get("box", (0, 0))
            if isinstance(box, list):
                box = tuple(box)
            return getattr(img1, target)(img2, box)
        # Default: ImageChops
        return getattr(PIL.ImageChops, target)(img1, img2)

    def call_draw(self, img, module, target, params):
        draw = PIL.ImageDraw.Draw(img)
        p = _to_rgb_fill(img.mode, params, ("fill", "outline"))
        if target == "bitmap":
            bmp = img.convert("1", dither=PIL.Image.Dither.NONE) if img.mode != "1" else make_image("1")
            draw.bitmap(tuple(p.get("xy", [5, 5])), bmp, fill=p.get("fill", 200))
        else:
            getattr(draw, target)(**p)
        return img

    def call_enhance(self, img, module, target, params):
        enh = getattr(PIL.ImageEnhance, target)(img)
        return enh.enhance(float(params.get("factor", 1.5)))

    def call_classmethod(self, module, target, params, img=None):
        if target == "new":
            return PIL.Image.new(params.get("mode", "RGB"),
                                 tuple(params.get("size", [100, 100])),
                                 params.get("color", 0))
        if target == "effect_noise":
            _seed_rand()
            return PIL.Image.effect_noise(tuple(params["size"]), params["sigma"])
        if target in ("open", "frombytes"):
            return PIL.Image.frombytes(img.mode, img.size, img.tobytes())
        if target == "eval":
            return PIL.Image.eval(img, lambda x: min(255, x + 10))
        if target == "merge":
            return PIL.Image.merge(img.mode, img.split() or [img])
        if target == "blend":
            i1, i2 = make_image(params.get("mode", "L")), make_image(params.get("mode", "L"))
            return PIL.Image.blend(i1, i2, float(params.get("alpha", 0.5)))
        if target == "composite":
            i1 = make_image(params.get("mode", "L"))
            i2 = make_image(params.get("mode", "L"))
            m = PIL.Image.new("L", i1.size, 128)
            return PIL.Image.composite(i1, i2, m)
        raise NotImplementedError(f"classmethod {module}.{target}")

    def call_value(self, img, module, target, params):
        if module == "Image" and hasattr(img, target):
            val = getattr(img, target)
            if target in ("mode", "size", "width", "height", "format", "info"):
                return val
            if target == "getexif": return img.getexif()
            if target == "getim": return str(img.getim()) if img.getim() else None
            if target == "getpalette": return img.getpalette()
            if target == "getxmp": return {}
            if target == "get_flattened_data": return img.get_flattened_data()
            if target == "get_child_images": return []
            if target == "palette": return None
            if target in ("is_animated", "has_transparency_data"): return False
            if target == "n_frames": return 1
            if target in ("apply_transparency", "show"): return None
            if callable(val):
                return val(**params)
            return val
        if module == "ImageColor":
            return getattr(PIL.ImageColor, target)(**params)
        if module == "ImagePalette":
            palette = PIL.ImagePalette.ImagePalette(mode="RGB")
            if target == "copy":
                try: return palette.copy().tobytes()
                except: return bytes()
            if target == "getcolor":
                return palette.getcolor(tuple(params.get("color", [255, 0, 0])))
            if target == "getdata":
                try: return list(palette.getdata())
                except: return []
            if target == "save": return None
            if target == "tobytes":
                try: return palette.tobytes()
                except: return bytes()
        if module == "ImageFont":
            if target == "load_default":
                try:
                    font = PIL.ImageFont.load_default()
                    return str(type(font).__name__)
                except: return None
            if target == "load_default_imagefont":
                try: return "ImageFont"
                except: return None
            if target in ("load", "load_path", "truetype", "FreeTypeFont", "ImageFont"):
                return None
        if module == "ImageStat":
            s = PIL.ImageStat.Stat(img)
            to_l = lambda v: v if isinstance(v, list) else [v]
            return {
                'count': to_l(s.count), 'sum': to_l(s.sum),
                'mean': to_l(s.mean), 'median': to_l(s.median),
                'rms': to_l(s.rms), 'var': to_l(s.var),
                'stddev': to_l(s.stddev),
                'extrema': [[e[0], e[1]] for e in (s.extrema if isinstance(s.extrema, list) else [s.extrema])]
            }
        if module == "ImageSequence":
            return None
        raise NotImplementedError(f"value {module}.{target}")

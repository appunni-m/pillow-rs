#!/usr/bin/env python3
"""Shared operation registry — single source of truth for fixture generation and testing.

Both generate_fixtures.py and test_fixture_parity.py import this module.
Each entry defines: what to call, with what parameters, and how to prepare inputs.

Format:
    "Module.function": {
        "type": "image" | "dual" | "filter" | "draw" | "value" | "module",
        "method": "resize",           # Image instance method name (for "image" type)
        "params": {"size": [50, 50]}, # Default parameters
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'], ["L", "LA", "RGB", "RGBA", "1", "P", "F"], ["L", "LA", "RGB", "RGBA"], # Modes to test
        "prep": "convert('1', ...)",  # Optional input prep for dual-image ops
    }
"""

REGISTRY = {
    # ═══════════════════════════════════════════════════════════════
    # Image instance methods — call img.method(**params)
    # ═══════════════════════════════════════════════════════════════
    "Image.resize": {
        "type": "image",
        "method": "resize",
        "params": {"size": [50, 50]},
        "modes": ["L", "LA", "RGB", "RGBA", "1", "P", "CMYK", "YCbCr", "HSV", "I", "F"],
    },
    "Image.crop": {
        "type": "image",
        "method": "crop",
        "params": {"box": [25, 25, 75, 75]},
        "modes": ["L", "LA", "RGB", "RGBA", "1", "P", "CMYK", "YCbCr", "HSV", "I", "F"],
    },
    "Image.rotate": {
        "type": "image",
        "method": "rotate",
        "params": {"angle": 90},
        "modes": ["L", "LA", "RGB", "RGBA", "1", "P", "CMYK"],
    },
    "Image.transpose": {
        "type": "image",
        "method": "transpose",
        "params": {"method": 0},  # FLIP_LEFT_RIGHT
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.copy": {
        "type": "image",
        "method": "copy",
        "params": {},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.tobytes": {
        "type": "value",
        "method": "tobytes",
        "params": {},
        "modes": ["L", "LA", "RGB", "RGBA", "1", "P"],
    },
    "Image.thumbnail": {
        "type": "image",
        "method": "thumbnail",
        "params": {"size": [50, 50]},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.quantize": {
        "type": "image",
        "method": "quantize",
        "params": {"colors": 16},
        "modes": ['CMYK', 'L', 'LA', 'RGB', 'RGBA', 'YCbCr'],
    },
    "Image.filter": {
        "type": "image",
        "method": "filter",
        "params": {"filter": "BLUR"},
        "modes": ["L", "LA", "RGB", "RGBA"],
    },
    "Image.convert": {
        "type": "image",
        "method": "convert",
        # Special: target_mode depends on the current image mode
        "params": {"mode": "__CONVERT_TO__"},  # filled at runtime
        "modes": ["L", "LA", "RGB", "RGBA", "1", "P", "CMYK", "YCbCr", "HSV", "I", "F"],
    },
    "Image.split": {
        "type": "value",
        "method": "split",
        "params": {},
        "modes": ["L", "LA", "RGB", "RGBA"],
    },
    "Image.getbands": {
        "type": "value",
        "method": "getbands",
        "params": {},
        "modes": ["L", "LA", "RGB", "RGBA", "1", "P"],
    },
    "Image.getbbox": {
        "type": "value",
        "method": "getbbox",
        "params": {},
        "modes": ["L", "LA", "RGB", "RGBA"],
    },
    "Image.getextrema": {
        "type": "value",
        "method": "getextrema",
        "params": {},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.histogram": {
        "type": "value",
        "method": "histogram",
        "params": {},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.getpixel": {
        "type": "value",
        "method": "getpixel",
        "params": {"xy": [50, 50]},
        "modes": ["L", "LA", "RGB", "RGBA", "1", "P"],
    },
    "Image.getcolors": {
        "type": "value",
        "method": "getcolors",
        "params": {"maxcolors": 256},
        "modes": ['1', 'L', 'P', 'RGB', 'RGBA'],
    },
    "Image.getdata": {
        "type": "value",
        "method": "getdata",
        "params": {"band": None},
        "modes": ["L", "LA", "RGB", "RGBA"],
    },
    "Image.getprojection": {
        "type": "value",
        "method": "getprojection",
        "params": {},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "Image.entropy": {
        "type": "value",
        "method": "entropy",
        "params": {},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.getchannel": {
        "type": "image",
        "method": "getchannel",
        "params": {"channel": 0},
        "modes": ["L", "LA", "RGB", "RGBA"],
    },
    "Image.load": {
        "type": "value",
        "method": "load",
        "params": {},
        "modes": ["1", "L", "LA", "P", "RGB", "RGBA"],
    },
    "Image.verify": {
        "type": "value",
        "method": "verify",
        "params": {},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "Image.seek": {
        "type": "value",
        "method": "seek",
        "params": {"frame": 0},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.tell": {
        "type": "value",
        "method": "tell",
        "params": {},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.draft": {
        "type": "image",
        "method": "draft",
        "params": {"mode": "RGB", "size": [50, 50]},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.putalpha": {
        "type": "image",
        "method": "putalpha",
        "params": {"alpha": 128},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.putpixel": {
        "type": "image",
        "method": "putpixel",
        "params": {"xy": [50, 50], "value": [255, 255, 255, 255]},
        "modes": ["1", "L", "LA", "P", "RGB", "RGBA"],
    },
    "Image.putdata": {
        "type": "image",
        "method": "putdata",
        "params": {"data": [128]},  # expanded at runtime based on band count
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.reduce": {
        "type": "image",
        "method": "reduce",
        "params": {"factor": 2},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.effect_spread": {
        "type": "image",
        "method": "effect_spread",
        "params": {"distance": 2},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.transform": {
        "type": "image",
        "method": "transform",
        "params": {"size": (50, 50), "method": 0, "data": [1, 0, 0, 0, 1, 0]},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "Image.remap_palette": {
        "type": "image",
        "method": "remap_palette",
        "params": {"dest_map": [0, 1]},
        "modes": ['L', 'P'],
    },
    "Image.tobitmap": {
        "type": "value",
        "method": "tobitmap",
        "params": {},
        "modes": ["1"],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageFilter — create filter object, then img.filter(obj)
    # ═══════════════════════════════════════════════════════════════
    "ImageFilter.BLUR": {
        "type": "filter",
        "name": "BLUR",
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "ImageFilter.CONTOUR": {
        "type": "filter",
        "name": "CONTOUR",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.DETAIL": {
        "type": "filter",
        "name": "DETAIL",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.EDGE_ENHANCE": {
        "type": "filter",
        "name": "EDGE_ENHANCE",
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "ImageFilter.EDGE_ENHANCE_MORE": {
        "type": "filter",
        "name": "EDGE_ENHANCE_MORE",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.EMBOSS": {
        "type": "filter",
        "name": "EMBOSS",
        "modes": ['L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "ImageFilter.FIND_EDGES": {
        "type": "filter",
        "name": "FIND_EDGES",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.SHARPEN": {
        "type": "filter",
        "name": "SHARPEN",
        "modes": ['1', 'CMYK', 'F', 'HSV', 'I', 'L', 'LA', 'P', 'RGB', 'RGBA', 'YCbCr'],
    },
    "ImageFilter.SMOOTH": {
        "type": "filter",
        "name": "SMOOTH",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.SMOOTH_MORE": {
        "type": "filter",
        "name": "SMOOTH_MORE",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.BoxBlur": {
        "type": "filter",
        "name": "BoxBlur",
        "params": {"radius": 2},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.GaussianBlur": {
        "type": "filter",
        "name": "GaussianBlur",
        "params": {"radius": 2},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.UnsharpMask": {
        "type": "filter",
        "name": "UnsharpMask",
        "params": {"radius": 2, "percent": 150, "threshold": 3},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.MaxFilter": {
        "type": "filter",
        "name": "MaxFilter",
        "params": {"size": 3},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.MinFilter": {
        "type": "filter",
        "name": "MinFilter",
        "params": {"size": 3},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.MedianFilter": {
        "type": "filter",
        "name": "MedianFilter",
        "params": {"size": 3},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.ModeFilter": {
        "type": "filter",
        "name": "ModeFilter",
        "params": {"size": 3},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.RankFilter": {
        "type": "filter",
        "name": "RankFilter",
        "params": {"size": 3, "rank": 2},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageFilter.Kernel": {
        "type": "filter",
        "name": "Kernel",
        "params": {"size": [3, 3], "kernel": [1, 1, 1, 1, 1, 1, 1, 1, 1], "scale": 9, "offset": 0},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageChops — dual-image operations
    # ═══════════════════════════════════════════════════════════════
    "ImageChops.add": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.subtract": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.multiply": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.screen": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.darker": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.lighter": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.difference": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.add_modulo": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.subtract_modulo": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.hard_light": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.soft_light": {
        "type": "dual",
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageChops.overlay": {
        "type": "dual",
        "modes": ['L', 'RGB'],
    },
    "ImageChops.logical_and": {
        "type": "dual",
        "prep": "convert('1', dither='NONE')",
        "modes": ["1"],
    },
    "ImageChops.logical_or": {
        "type": "dual",
        "prep": "convert('1', dither='NONE')",
        "modes": ["1"],
    },
    "ImageChops.logical_xor": {
        "type": "dual",
        "prep": "convert('1', dither='NONE')",
        "modes": ["1"],
    },
    "ImageChops.blend": {
        "type": "dual",
        "modes": ['L', 'P', 'RGB'],
    },
    "ImageChops.composite": {
        "type": "dual",
        "modes": ['L', 'P', 'RGB'],
    },
    "ImageChops.invert": {
        "type": "image",
        "method": "invert",
        "modes": ['L', 'P', 'RGB'],
    },
    "ImageChops.duplicate": {
        "type": "image",
        "method": "copy",
        "modes": ['L', 'P', 'RGB'],
    },
    "ImageChops.constant": {
        "type": "image",
        "method": "constant",
        "params": {"value": 128},
        "modes": ['L', 'P', 'RGB'],
    },
    "ImageChops.offset": {
        "type": "image",
        "method": "offset",
        "params": {"xoffset": 5, "yoffset": 5},
        "modes": ['L', 'RGB', 'RGBA'],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageOps — instance methods on the image
    # ═══════════════════════════════════════════════════════════════
    "ImageOps.autocontrast": {
        "type": "image",
        "method": "autocontrast",
        "params": {"cutoff": 0},
        "modes": ['L', 'RGB', 'RGBA'],
    },
    "ImageOps.equalize": {
        "type": "image",
        "method": "equalize",
        "params": {},
        "modes": ['L', 'RGB', 'RGBA'],
    },
    "ImageOps.invert": {
        "type": "image",
        "method": "invert",
        "params": {},
        "modes": ['L', 'RGB', 'RGBA'],
    },
    "ImageOps.flip": {
        "type": "image",
        "method": "flip",
        "params": {},
        "modes": ['L', 'RGB', 'RGBA'],
    },
    "ImageOps.mirror": {
        "type": "image",
        "method": "mirror",
        "params": {},
        "modes": ['L', 'RGB', 'RGBA'],
    },
    "ImageOps.posterize": {
        "type": "image",
        "method": "posterize",
        "params": {"bits": 4},
        "modes": ["L", "RGB"],
    },
    "ImageOps.solarize": {
        "type": "image",
        "method": "solarize",
        "params": {"threshold": 128},
        "modes": ["L", "RGB"],
    },
    "ImageOps.grayscale": {
        "type": "image",
        "method": "convert",
        "params": {},
        "modes": ['L', 'LA', 'RGB', 'RGBA'],
    },
    "ImageOps.colorize": {
        "type": "image",
        "method": "colorize",
        "params": {"black": "black", "white": "white"},
        "modes": ["L", "RGB"],
    },
    "ImageOps.expand": {
        "type": "image",
        "method": "expand",
        "params": {"border": 5},
        "modes": ["L", "RGB"],
    },
    "ImageOps.crop": {
        "type": "image",
        "method": "crop",
        "params": {"border": 5},
        "modes": ["L", "RGB"],
    },
    "ImageOps.scale": {
        "type": "image",
        "method": "scale",
        "params": {"factor": 0.5},
        "modes": ["L", "RGB"],
    },
    "ImageOps.contain": {
        "type": "image",
        "method": "contain",
        "params": {"size": [25, 25]},
        "modes": ["L", "RGB"],
    },
    "ImageOps.cover": {
        "type": "image",
        "method": "cover",
        "params": {"size": [25, 25]},
        "modes": ["L", "RGB"],
    },
    "ImageOps.fit": {
        "type": "image",
        "method": "fit",
        "params": {"size": [25, 25]},
        "modes": ["L", "RGB"],
    },
    "ImageOps.pad": {
        "type": "image",
        "method": "pad",
        "params": {"size": [25, 25]},
        "modes": ["L", "RGB"],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageEnhance
    # ═══════════════════════════════════════════════════════════════
    "ImageEnhance.Brightness": {
        "type": "enhance",
        "name": "Brightness",
        "params": {"factor": 1.5},
        "modes": ["L", "RGB"],
    },
    "ImageEnhance.Color": {
        "type": "enhance",
        "name": "Color",
        "params": {"factor": 1.5},
        "modes": ["L", "RGB"],
    },
    "ImageEnhance.Contrast": {
        "type": "enhance",
        "name": "Contrast",
        "params": {"factor": 1.5},
        "modes": ["L", "RGB"],
    },
    "ImageEnhance.Sharpness": {
        "type": "enhance",
        "name": "Sharpness",
        "params": {"factor": 1.5},
        "modes": ["L", "RGB"],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageModule — class methods on Image
    # ═══════════════════════════════════════════════════════════════
    "ImageModule.new": {
        "type": "module",
        "function": "Image.new",
        "params": {"mode": "RGB", "size": [100, 100], "color": 0},
        "modes": ['1', 'CMYK', 'F', 'HSV', 'I', 'L', 'LA', 'P', 'RGB', 'RGBA', 'YCbCr'],
    },
    "ImageModule.open": {
        "type": "module",
        "function": "Image.open",
        "params": {},  # uses input bytes
        "modes": ["L", "RGB"],
    },
    "ImageModule.frombytes": {
        "type": "module",
        "function": "Image.frombytes",
        "params": {},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "ImageModule.blend": {
        "type": "dual",
        "function": "Image.blend",
        "params": {"alpha": 0.5},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "ImageModule.composite": {
        "type": "dual",
        "function": "Image.composite",
        "params": {},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "ImageModule.merge": {
        "type": "module",
        "function": "Image.merge",
        "params": {},
        "modes": ["L", "LA", "RGB", "RGBA"],
    },
    "ImageModule.eval": {
        "type": "module",
        "function": "Image.eval",
        "params": {},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },
    "ImageModule.alpha_composite": {
        "type": "image",
        "method": "alpha_composite",
        "params": {"fg_alpha": 128},
        "modes": ["L", "RGB"],
    },
    "ImageModule.effect_noise": {
        "type": "module",
        "function": "Image.effect_noise",
        "params": {"size": [100, 100], "sigma": 10.0},
        "modes": ['1', 'CMYK', 'L', 'LA', 'P', 'RGB', 'RGBA'],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageDraw — draw on image, return modified image
    # ═══════════════════════════════════════════════════════════════
    "ImageDraw.line": {
        "type": "draw",
        "draw": "line",
        "params": {"xy": [[10, 10], [40, 40]], "fill": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.circle": {
        "type": "draw",
        "draw": "circle",
        "params": {"xy": [25, 25], "radius": 15, "fill": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.rectangle": {
        "type": "draw",
        "draw": "rectangle",
        "params": {"xy": [10, 10, 40, 40], "outline": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.ellipse": {
        "type": "draw",
        "draw": "ellipse",
        "params": {"xy": [10, 10, 40, 40], "outline": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.polygon": {
        "type": "draw",
        "draw": "polygon",
        "params": {"xy": [[10, 10], [40, 10], [25, 40]], "outline": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.arc": {
        "type": "draw",
        "draw": "arc",
        "params": {"xy": [10, 10, 40, 40], "start": 0, "end": 180, "fill": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.chord": {
        "type": "draw",
        "draw": "chord",
        "params": {"xy": [10, 10, 40, 40], "start": 0, "end": 180, "fill": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.pieslice": {
        "type": "draw",
        "draw": "pieslice",
        "params": {"xy": [10, 10, 40, 40], "start": 0, "end": 180, "fill": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.point": {
        "type": "draw",
        "draw": "point",
        "params": {"xy": [25, 25], "fill": 200},
        "modes": ['1', 'L', 'LA', 'P', 'RGB', 'RGBA', 'CMYK'],
    },
    "ImageDraw.regular_polygon": {
        "type": "draw",
        "draw": "regular_polygon",
        "params": {"bounding_circle": [25, 25, 15], "n_sides": 5, "fill": 200},
        "modes": ["L", "RGB", "RGBA"],
    },
    "ImageDraw.rounded_rectangle": {
        "type": "draw",
        "draw": "rounded_rectangle",
        "params": {"xy": [10, 10, 40, 40], "radius": 5, "outline": 200},
        "modes": ["L", "RGB", "RGBA"],
    },
    "ImageDraw.bitmap": {
        "type": "draw",
        "draw": "bitmap",
        "params": {"xy": [5, 5], "fill": 200},
        "modes": ["L", "RGB", "RGBA"],
    },
    "ImageDraw.text": {
        "type": "draw",
        "draw": "text",
        "params": {"xy": [5, 5], "text": "Hello", "fill": 200},
        "modes": ["L", "RGB", "RGBA"],
    },
    "ImageDraw.multiline_text": {
        "type": "draw",
        "draw": "multiline_text",
        "params": {"xy": [5, 5], "text": "Hello", "fill": 200},
        "modes": ["L", "RGB", "RGBA"],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageColor — return color values
    # ═══════════════════════════════════════════════════════════════
    "ImageColor.getcolor": {
        "type": "value",
        "function": "ImageColor.getcolor",
        "params": {"color": "red", "mode": "RGB"},
        "modes": ["L", "RGB"],
    },
    "ImageColor.getrgb": {
        "type": "value",
        "function": "ImageColor.getrgb",
        "params": {"color": "red"},
        "modes": ["L", "RGB"],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImagePalette
    # ═══════════════════════════════════════════════════════════════
    "ImagePalette.copy": {
        "type": "value",
        "function": "ImagePalette.copy",
        "params": {},
        "modes": ["L", "RGB"],
    },
    "ImagePalette.getcolor": {
        "type": "value",
        "function": "ImagePalette.getcolor",
        "params": {"color": [255, 0, 0]},
        "modes": ["L", "RGB"],
    },
    "ImagePalette.getdata": {
        "type": "value",
        "function": "ImagePalette.getdata",
        "params": {},
        "modes": ["L", "RGB"],
    },
    "ImagePalette.save": {
        "type": "value",
        "function": "ImagePalette.save",
        "params": {},
        "modes": ["L", "RGB"],
    },
    "ImagePalette.tobytes": {
        "type": "value",
        "function": "ImagePalette.tobytes",
        "params": {},
        "modes": ["L", "RGB"],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageFont
    # ═══════════════════════════════════════════════════════════════
    "ImageFont.load_default": {
        "type": "value",
        "function": "ImageFont.load_default",
        "params": {},
        "modes": ["L", "RGB"],
    },
    "ImageFont.load_default_imagefont": {
        "type": "value",
        "function": "ImageFont.load_default_imagefont",
        "params": {},
        "modes": ["L", "RGB"],
    },
    "ImageFont.load": {
        "type": "value",
        "function": "ImageFont.load",
        "params": {},
        "modes": ["L", "RGB"],
    },
    "ImageFont.load_path": {
        "type": "value",
        "function": "ImageFont.load_path",
        "params": {},
        "modes": ["L", "RGB"],
    },
    "ImageFont.truetype": {
        "type": "value",
        "function": "ImageFont.truetype",
        "params": {},
        "modes": ["L", "RGB"],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageStat
    # ═══════════════════════════════════════════════════════════════
    "ImageStat.Stat": {
        "type": "value",
        "function": "ImageStat.Stat",
        "params": {},
        "modes": ["L", "RGB"],
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageSequence
    # ═══════════════════════════════════════════════════════════════
    "ImageSequence.Iterator": {
        "type": "value",
        "function": "ImageSequence.Iterator",
        "params": {},
        "modes": ["L", "RGB"],
    },

    # ═══════════════════════════════════════════════════════════════
    # Image properties (value-returning)
    # ═══════════════════════════════════════════════════════════════
    "Image.getexif": {"type": "value", "property": "getexif", "modes": ["L", "RGB"]},
    "Image.getim": {"type": "value", "property": "getim", "modes": ["L", "RGB"]},
    "Image.getpalette": {"type": "value", "property": "getpalette", "modes": ["L", "RGB"]},
    "Image.getxmp": {"type": "value", "property": "getxmp", "modes": ["L", "RGB"]},
    "Image.get_flattened_data": {"type": "value", "property": "get_flattened_data", "modes": ["L", "RGB"]},
    "Image.get_child_images": {"type": "value", "property": "get_child_images", "modes": ["L", "RGB"]},
    "Image.apply_transparency": {"type": "value", "property": "apply_transparency", "modes": ["L", "RGB"]},

    # ── Missing testable ops added for complete coverage ──
    "Image.point": {
        "type": "image",
        "method": "point",
        "params": {"lut": list(range(256))},
        "modes": ["L", "RGB"],
    },

    # ═══════════════════════════════════════════════════════════════
    # Image instance methods — dual (paste)
    # ═══════════════════════════════════════════════════════════════
    "Image.paste": {
        "type": "dual",
        "method": "paste",
        "params": {"box": [0, 0]},
        "modes": ["L", "RGB", "RGBA"],
        # NOTE: paste modifies in-place and returns None. "dual" creates a second
        # image and calls PIL.Image.paste(img1, img2). The default call_dual does
        # NOT forward params (no box applied). Both backends need "dual" handling
        # for module="Image". Fixture stores expected=None (weak no-crash test).
    },

    # ═══════════════════════════════════════════════════════════════
    # Image instance methods — palette modification
    # ═══════════════════════════════════════════════════════════════
    "Image.putpalette": {
        "type": "image",
        "method": "putpalette",
        "params": {"data": list(range(48))},  # 16-colour RGB palette (48 flat values)
        "modes": ["P"],
        # NOTE: modifies palette in-place, returns None. Requires "P" mode image.
        # Fixture stores expected=None (weak no-crash test). Both backends handle
        # via img.putpalette(**params).
    },

    # ═══════════════════════════════════════════════════════════════
    # ImageFilter — parametric filter requiring table data
    # ═══════════════════════════════════════════════════════════════

    # ═══════════════════════════════════════════════════════════════
    # ImageOps — operations requiring complex params or decorators
    # ═══════════════════════════════════════════════════════════════

    # ═══════════════════════════════════════════════════════════════
    # ImageDraw — value-returning methods (not draw/mutating operations)
    # ═══════════════════════════════════════════════════════════════

    # ═══════════════════════════════════════════════════════════════
    # ImageFont — instance methods on font objects (FreeTypeFont/ImageFont)
    # ═══════════════════════════════════════════════════════════════
    # These require a font OBJECT, not an Image. The current execution engine
    # passes Images to call_value/call_method; it cannot provide a font instance.
    # Both backends need rework to support font-object operations.

    # ═══════════════════════════════════════════════════════════════
    # ImageModule — classmethods requiring external dependencies
    # ═══════════════════════════════════════════════════════════════
}

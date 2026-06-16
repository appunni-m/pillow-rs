# Test Infrastructure DSL Redesign v2

Date: 2026-06-16
Status: Draft

## 1. Separation of Concerns

| Concern | Where it lives | Why |
|---------|---------------|-----|
| API surface (which ops exist) | `manifest.yaml` | Canonical catalog of all operations |
| Status (implemented/stub/ignored) | `manifest.yaml` | Coverage tracking |
| Supported modes per operation | `manifest.yaml` | Coverage per-mode tracking |
| Parameter variations to test | `fixtures/*.json` cases array | Test data, not API metadata |
| How to execute an operation | `engine.py` get_call_style() | Derived from module name, 35-line function |
| How to compare outputs | `fixtures/*.json` assert.method | Declared per case |
| Reference output data | `fixtures/output/images/*.png` | Viewable, pixel-exact |

## 2. Core Principles

1. **Fixture = array of cases** — One JSON file per operation contains ALL mode/param variations as an array. Adding a variant = append to the array.
2. **No per-operation code anywhere** — The engine is a generic dispatch loop. Adding a new operation requires zero Python changes.
3. **No parameter transformation** — PyO3/wasm-bindgen glue layer provides PIL-identical Python/JS API. Params pass straight through.
4. **manifest.yaml = API catalog only** — Tracks which operations exist, their status, and supported modes. Does NOT contain test parameters or execution metadata.
5. **Viewable reference outputs** — Output images saved as files (PNG/bin), not hex blobs. Open to inspect.
6. **Explicit assertion methods** — Each case declares how to compare. No type-inference branching.

## 2. Why No Parameter Transformation Is Needed

RSPIL's Python bindings (PyO3) and JS bindings (wasm-bindgen) already present a PIL-identical API at the Python/JavaScript level. The glue layer handles list→tuple coercion, int→enum mapping, etc. The Rust core may use different internal types, but that's irrelevant — the test engine only talks to the Python/JS API, which is already PIL-compatible.

```
JSON params ──→ Python binding (PyO3) ──→ Rust core (internal types)
     │                    │
     │              list→tuple, int→enum, etc.
     │              (handled by pyo3 glue, not tests)
     │
PIL params ──→ PIL C code
```

Both paths accept the same parameter format. The test engine just passes JSON params directly.

## 3. What Gets Deleted

| File | Lines | Why |
|------|-------|-----|
| `tests/rspil_backend.py` | 415 | 70 per-op branches → 0 (generic engine) |
| `scripts/coverage/pil_backend.py` | 361 | 50 per-op branches → 0 (generic engine) |
| `scripts/coverage/ops_registry.py` | 353 | 178 FIXTURE_META → 0 (manifest is source of truth for API, fixtures for params) |
| `scripts/coverage/execution_engine.py` | 47 | Inlined |
| `tests/test_fixture_parity.py` | 157 | Replaced by generic runner |
| `scripts/coverage/generate_fixtures.py` | 123 | Replaced by generic generator |

**~1,500 lines deleted. Replaced by ~150 lines of generic engine.**

## 4. manifest.yaml Role

manifest.yaml is the **API coverage catalog**. It tracks:
- **Which operations exist** and their `status` (implemented, stub, ignored)
- **Which modes** each operation supports — for coverage computation per mode
- **Signature / edge cases** — documentation, not execution data

```yaml
modules:
  Image:
    methods:
      - name: resize
        status: implemented
        supported_modes: [L, LA, RGB, RGBA, CMYK, P, 1, I, F]
        signature: "resize(size, resample=None, box=None, ...)"
      - name: crop
        status: implemented
        supported_modes: [L, LA, RGB, RGBA, CMYK, P, 1]
```

What manifest does NOT contain:
- ❌ Default test parameters — param variations live in fixture `cases[]`
- ❌ Execution type metadata — call_style derived from module name by `get_call_style()`
- ❌ Fixture file references — discovered by scanning `tests/fixtures/*.json`

**Coverage flow:**
```
manifest.yaml ──→ which (op, mode) combos should exist
fixtures/*.json ──→ which (op, mode) combos actually have tests
compute_coverage.py ──→ cross-reference → trust report
```

## 5. Fixture JSON Format v2

### 5.1 Minimal fixture (one operation, multiple param variations)

```json
{
  "format_version": 2,
  "operation": {
    "module": "Image",
    "target": "resize"
  },
  "cases": [
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "params": {"size": [50, 50]},
      "assert": {"method": "image", "reference": "output/images/Image_resize_L_50x50.png"}
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "params": {"size": [25, 25]},
      "assert": {"method": "image", "reference": "output/images/Image_resize_L_25x25.png"}
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "RGB"},
      "params": {"size": [50, 50]},
      "assert": {"method": "image", "reference": "output/images/Image_resize_RGB_50x50.png"}
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "RGBA"},
      "params": {"size": [50, 50]},
      "assert": {"method": "image", "reference": "output/images/Image_resize_RGBA_50x50.png"}
    }
  ]
}
```

### 5.2 Value-returning operation (array of (mode, param) combos)

```json
{
  "format_version": 2,
  "operation": {
    "module": "ImageColor",
    "target": "getcolor"
  },
  "cases": [
    {
      "input": null,
      "params": {"color": "red", "mode": "RGB"},
      "assert": {"method": "exact", "value": [255, 0, 0]}
    },
    {
      "input": null,
      "params": {"color": "red", "mode": "L"},
      "assert": {"method": "exact", "value": [255]}
    },
    {
      "input": null,
      "params": {"color": "#00ff00", "mode": "RGB"},
      "assert": {"method": "exact", "value": [0, 255, 0]}
    },
    {
      "input": null,
      "params": {"color": "blue", "mode": "RGBA"},
      "assert": {"method": "exact", "value": [0, 0, 255, 255]}
    }
  ]
}
```

### 5.3 Draw operation (multiple fill variations in one file)

```json
{
  "format_version": 2,
  "operation": {
    "module": "ImageDraw",
    "target": "text"
  },
  "cases": [
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "params": {"xy": [5, 5], "text": "Hello", "fill": 200},
      "assert": {"method": "image", "reference": "output/images/ImageDraw_text_L_fill200.png"}
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "params": {"xy": [5, 5], "text": "Hello", "fill": 128},
      "assert": {"method": "image", "reference": "output/images/ImageDraw_text_L_fill128.png"}
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "RGB"},
      "params": {"xy": [5, 5], "text": "Hello", "fill": 200},
      "assert": {"method": "image", "reference": "output/images/ImageDraw_text_RGB_fill200.png"}
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "LA"},
      "params": {"xy": [5, 5], "text": "Hello", "fill": 200},
      "assert": {"method": "image", "reference": "output/images/ImageDraw_text_LA_fill200.png"}
    }
  ]
}
```

### 5.4 Dual-input operation

```json
{
  "format_version": 2,
  "operation": {
    "module": "ImageChops",
    "target": "blend"
  },
  "cases": [
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "input2": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "params": {"alpha": 0.5},
      "assert": {"method": "image", "reference": "output/images/ImageChops_blend_L.png"}
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "RGB"},
      "input2": {"source": "reference_rgb", "size": [100, 100], "mode": "RGB"},
      "params": {"alpha": 0.3},
      "assert": {"method": "image", "reference": "output/images/ImageChops_blend_RGB_alpha03.png"}
    }
  ]
}
```

### 5.5 Mutate-in-place operation

```json
{
  "format_version": 2,
  "operation": {
    "module": "Image",
    "target": "putpixel"
  },
  "cases": [
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "params": {"xy": [50, 50], "value": 255},
      "assert": {"method": "image", "reference": "output/images/Image_putpixel_L.png"}
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "RGB"},
      "params": {"xy": [50, 50], "value": [255, 0, 0]},
      "assert": {"method": "image", "reference": "output/images/Image_putpixel_RGB.png"}
    }
  ]
}
```

### 5.6 Split operation (multi-image result)

```json
{
  "format_version": 2,
  "operation": {
    "module": "Image",
    "target": "split"
  },
  "cases": [
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "RGB"},
      "params": {},
      "assert": {
        "method": "image_list",
        "references": [
          "output/images/Image_split_RGB_band0.png",
          "output/images/Image_split_RGB_band1.png",
          "output/images/Image_split_RGB_band2.png"
        ]
      }
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "RGBA"},
      "params": {},
      "assert": {
        "method": "image_list",
        "references": [
          "output/images/Image_split_RGBA_band0.png",
          "output/images/Image_split_RGBA_band1.png",
          "output/images/Image_split_RGBA_band2.png",
          "output/images/Image_split_RGBA_band3.png"
        ]
      }
    }
  ]
}
```

### 5.7 Stat (dict result)

```json
{
  "format_version": 2,
  "operation": {
    "module": "ImageStat",
    "target": "Stat"
  },
  "cases": [
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "params": {},
      "assert": {
        "method": "json",
        "value": {
          "count": [10000],
          "sum": [1275000.0],
          "mean": [127.5],
          "median": [127.0],
          "rms": [150.3],
          "var": [5500.2],
          "stddev": [74.16],
          "extrema": [[3, 252]]
        }
      }
    },
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "RGB"},
      "params": {},
      "assert": {
        "method": "json",
        "value": {
          "count": [10000, 10000, 10000],
          "sum": [1275000.0, 1280000.0, 1265000.0],
          "mean": [127.5, 128.0, 126.5],
          "median": [127.0, 128.0, 126.0],
          "rms": [150.3, 151.0, 149.5],
          "var": [5500.2, 5600.0, 5400.5],
          "stddev": [74.16, 74.8, 73.5],
          "extrema": [[3, 252], [5, 250], [2, 254]]
        }
      }
    }
  ]
}
```

### 5.8 Error case

```json
{
  "format_version": 2,
  "operation": {
    "module": "Image",
    "target": "resize"
  },
  "cases": [
    {
      "input": {"source": "reference_rgb", "size": [100, 100], "mode": "L"},
      "params": {"size": [-1, -1]},
      "assert": {"method": "error", "exception": "ValueError"}
    }
  ]
}
```

## 6. Input Specification

```json
// Default — from reference PNG
{"source": "reference_rgb", "size": [100, 100], "mode": "L"}

// Constant color
{"source": "constant", "size": [100, 100], "mode": "RGB", "color": 128}

// Exact bytes (for frombytes tests)
{"source": "bytes", "size": [100, 100], "mode": "L", "bytes": "hex..."}

// No input (for module functions like ImageColor.getcolor)
null
```

`create_input(backend, spec)` handles all four cases. Identical for PIL and RSPIL.

## 7. The Generic Engine

### 7.1 Module → call_style mapping (35 lines, defined once)

```python
SINGLE_CHOPS = {"invert", "duplicate", "constant", "offset"}
MUTATE_TARGETS = {"putpixel", "putdata", "thumbnail", "putalpha"}
DUAL_TARGETS = {"paste", "alpha_composite"}
VALUE_TARGETS = {
    "tobytes", "split", "getbands", "getbbox", "getextrema", "histogram",
    "getpixel", "getcolors", "getdata", "getprojection", "entropy",
    "load", "verify", "seek", "tell", "tobitmap",
    "has_transparency_data", "getexif", "getim", "getpalette", "getxmp",
    "get_flattened_data", "get_child_images", "apply_transparency",
    "close", "save", "mode", "size", "width", "height",
    "format", "info", "is_animated", "n_frames", "palette",
}
DRAW_VALUE_TARGETS = {"textlength", "textbbox", "multiline_textbbox", "getfont"}

def get_call_style(module, target):
    """Pure data lookup. Never needs new entries for new operations."""
    if module == "Image":
        if target in DUAL_TARGETS:      return "instance_method_dual"
        if target in MUTATE_TARGETS:    return "instance_method_mutate"
        if target in VALUE_TARGETS:     return "instance_method_value"
        return "instance_method"
    if module == "ImageOps":            return "module_function"
    if module == "ImageChops":
        if target in SINGLE_CHOPS:      return "module_function"
        return "module_function_dual"
    if module == "ImageDraw":
        if target in DRAW_VALUE_TARGETS: return "draw_value"
        return "draw"
    if module == "ImageFilter":         return "filter"
    if module == "ImageEnhance":        return "enhance"
    if module == "ImageModule":         return "classmethod"
    if module in ("ImageColor", "ImagePalette", "ImageFont", "ImageSequence"):
        return "module_function_value"
    if module == "ImageStat":           return "stat"
    raise ValueError(f"Unknown module: {module}")
```

### 7.2 Call style implementations (14 lambdas, ~40 lines)

```python
CALL_STYLE = {
    "instance_method":        lambda b, img, img2, tgt, p: getattr(img, tgt)(**p),
    "instance_method_value":  lambda b, img, img2, tgt, p: getattr(img, tgt)(**p),
    "instance_method_mutate": lambda b, img, img2, tgt, p: (getattr(img, tgt)(**p), img)[1],
    "instance_method_dual":   lambda b, img, img2, tgt, p: getattr(img, tgt)(img2, **p),
    "draw":       lambda b, img, img2, tgt, p: (_draw(b, img, tgt, p), img)[1],
    "draw_value": lambda b, img, img2, tgt, p: _draw(b, img, tgt, p),
    "filter":     lambda b, img, img2, tgt, p: img.filter(_make_filter(b, tgt, p)),
    "enhance":    lambda b, img, img2, tgt, p: getattr(b.ImageEnhance, tgt)(img).enhance(p.pop("factor", 1.0)),
    "module_function":       lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, **p),
    "module_function_dual":  lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, **p),
    "module_function_value": lambda b, img, img2, tgt, p: _call_mod(b, tgt)(**p),
    "classmethod":           lambda b, img, img2, tgt, p: _call_mod(b, tgt)(**p),
    "stat": lambda b, img, img2, tgt, p: _stat_to_dict(getattr(b.ImageStat, tgt)(img)),
}

def _draw(backend, img, target, params):
    draw = backend.ImageDraw.Draw(img)
    return getattr(draw, target)(**params)

def _call_mod(backend, target):
    """Resolve target function from backend's module hierarchy.
    Walk the backend object tree looking for the target function.
    Uses the same attribute structure for both PIL and RSPIL backends."""
    # Try each module in the backend
    for mod_name in ["ImageOps", "ImageChops", "ImageColor", "ImagePalette",
                     "ImageFont", "ImageSequence", "Image", "ImageDraw"]:
        mod = getattr(backend, mod_name, None)
        if mod and hasattr(mod, target):
            return getattr(mod, target)
    # Try ImageFilter classes
    if hasattr(backend, "ImageFilter"):
        f = getattr(backend.ImageFilter, target, None)
        if f: return f
    # Try ImageEnhance classes
    if hasattr(backend, "ImageEnhance"):
        e = getattr(backend.ImageEnhance, target, None)
        if e: return e
    raise ValueError(f"Cannot resolve target: {target}")

def _make_filter(backend, target, params):
    """Construct a filter object. Builtins take no constructor params."""
    filter_cls = getattr(backend.ImageFilter, target)
    BUILTINS = {"BLUR", "CONTOUR", "DETAIL", "EDGE_ENHANCE", "EDGE_ENHANCE_MORE",
                "EMBOSS", "FIND_EDGES", "SHARPEN", "SMOOTH", "SMOOTH_MORE"}
    return filter_cls if target in BUILTINS else filter_cls(**params)

def _stat_to_dict(stat):
    to_l = lambda v: v if isinstance(v, list) else [v]
    return {
        "count": to_l(stat.count), "sum": to_l(stat.sum),
        "mean": to_l(stat.mean), "median": to_l(stat.median),
        "rms": to_l(stat.rms), "var": to_l(stat.var),
        "stddev": to_l(stat.stddev),
        "extrema": [[e[0], e[1]] for e in (stat.extrema if isinstance(stat.extrema, list) else [stat.extrema])]
    }
```

### 7.3 Assert method implementations (7 methods)

```python
def _load_reference(path):
    full = OUTPUT_DIR / path
    if path.endswith('.png'):
        return Image.open(str(full))
    return open(str(full), 'rb').read()

def _sha(data):
    if hasattr(data, 'tobytes'): return hashlib.sha256(data.tobytes()).hexdigest()
    return hashlib.sha256(data).hexdigest()

def _to_json_compat(val):
    """Convert any result type to JSON-serializable form."""
    if val is None: return None
    if isinstance(val, (int, float, str, bool)): return val
    if isinstance(val, bytes): return val.hex()
    if isinstance(val, (tuple, list)): return [_to_json_compat(v) for v in val]
    if isinstance(val, dict): return {str(k): _to_json_compat(v) for k, v in val.items()}
    if hasattr(val, 'tobytes'): return _sha(val)
    if hasattr(val, '__iter__') and not isinstance(val, (str, bytes)):
        return [_to_json_compat(v) for v in val]
    return repr(val)

ASSERT = {
    "image": lambda case, result:
        _sha(result) == _sha(_load_reference(case["reference"])),
    "image_list": lambda case, result:
        all(_sha(band) == _sha(_load_reference(ref))
            for band, ref in zip(result, case["references"])),
    "exact": lambda case, result:
        result == case["value"],
    "json": lambda case, result:
        json.dumps(_to_json_compat(result)) == json.dumps(case["value"]),
    "string": lambda case, result:
        str(result).startswith(case.get("prefix", "")) or repr(result) == case.get("value", ""),
    "float": lambda case, result:
        abs(result - case["value"]) < case.get("tolerance", 0.0001),
    "error": lambda case, result:
        isinstance(result, Exception)
        and case.get("exception", "") in type(result).__name__
        and case.get("message_contains", "") in str(result),
}
```

### 7.4 Test runner (~25 lines)

```python
# tests/test_parity.py
import json, pytest
from pathlib import Path
from engine import CALL_STYLE, ASSERT, create_input, get_call_style, OUTPUT_DIR
import pillow_rs as rspil

FIXTURES_DIR = Path(__file__).parent / "fixtures"

def _discover():
    for fpath in sorted(FIXTURES_DIR.glob("*.json")):
        yield pytest.param(fpath.name, id=fpath.stem)

@pytest.mark.parametrize("fixture_file", _discover())
def test_parity(fixture_file):
    fx = json.loads((FIXTURES_DIR / fixture_file).read_text())
    op = fx["operation"]
    call_style = get_call_style(op["module"], op["target"])

    for i, case in enumerate(fx["cases"]):
        img = create_input(rspil, case.get("input"))
        img2 = create_input(rspil, case.get("input2"))
        params = dict(case.get("params", {}))

        try:
            result = CALL_STYLE[call_style](rspil, img, img2, op["target"], params)
        except Exception as e:
            if case["assert"]["method"] == "error":
                assert ASSERT["error"](case["assert"], e), f"[{i}] error mismatch"
                continue
            raise

        assert ASSERT[case["assert"]["method"]](case["assert"], result), \
            f"[{i}] {case['assert']['method']} mismatch"
```

### 7.5 Fixture generator (~50 lines)

```python
# scripts/generate_fixtures.py
import json, hashlib, sys
from pathlib import Path
import PIL.Image, PIL.ImageDraw, PIL.ImageFilter, PIL.ImageChops
import PIL.ImageOps, PIL.ImageEnhance, PIL.ImageColor, PIL.ImagePalette
import PIL.ImageFont, PIL.ImageStat, PIL.ImageSequence

# Same engine, but with PIL modules as the backend
sys.path.insert(0, str(Path(__file__).parent.parent / "tests"))
from engine import CALL_STYLE, get_call_style, create_input, _to_json_compat

class PilBackend:
    Image = PIL.Image; ImageFilter = PIL.ImageFilter; ImageChops = PIL.ImageChops
    ImageOps = PIL.ImageOps; ImageEnhance = PIL.ImageEnhance; ImageDraw = PIL.ImageDraw
    ImageColor = PIL.ImageColor; ImagePalette = PIL.ImagePalette
    ImageFont = PIL.ImageFont; ImageStat = PIL.ImageStat; ImageSequence = PIL.ImageSequence

OUTPUT_DIR = Path(__file__).parent.parent / "tests" / "fixtures" / "output"
pil = PilBackend()

def generate(fixture_spec, fixture_path):
    op = fixture_spec["operation"]
    call_style = get_call_style(op["module"], op["target"])

    for case in fixture_spec["cases"]:
        img = create_input(pil, case.get("input"))
        img2 = create_input(pil, case.get("input2"))
        params = dict(case.get("params", {}))
        result = CALL_STYLE[call_style](pil, img, img2, op["target"], params)

        method = case["assert"]["method"]
        if method == "image":
            ref_path = OUTPUT_DIR / case["assert"]["reference"]
            ref_path.parent.mkdir(parents=True, exist_ok=True)
            result.save(str(ref_path))
        elif method == "image_list":
            for i, band in enumerate(result):
                ref_path = OUTPUT_DIR / case["assert"]["references"][i]
                ref_path.parent.mkdir(parents=True, exist_ok=True)
                band.save(str(ref_path))
        elif method in ("exact", "float"):
            case["assert"]["value"] = result
        elif method == "json":
            case["assert"]["value"] = _to_json_compat(result)
        elif method == "string":
            case["assert"]["value"] = repr(result)
        # error case: value stays as-is (exception type + message pattern)

    with open(fixture_path, 'w') as f:
        json.dump(fixture_spec, f, indent=2)
```

## 8. Directory Structure

```
tests/
├── test_reference.png
├── test_parity.py              ← generic test runner (~25 lines)
├── engine.py                   ← get_call_style + CALL_STYLE + ASSERT + create_input (~130 lines)
├── fixtures/
│   ├── Image_resize.json       ← all modes + param variations for resize
│   ├── ImageDraw_text.json     ← all modes + fill variations for text
│   ├── ImageColor_getcolor.json
│   ├── ...
│   └── output/
│       ├── images/
│       │   ├── Image_resize_L_50x50.png
│       │   ├── Image_resize_L_25x25.png
│       │   ├── Image_resize_RGB_50x50.png
│       │   └── ...
│       └── raw/
│           └── Image_convert_F.bin

scripts/
└── generate_fixtures.py        ← fixture generator (~50 lines)
```

## 9. File Count Reduction

| Aspect | Before | After |
|--------|--------|-------|
| Fixture files | ~670 JSON files (one per op+mode) | ~178 JSON files (one per op, array of mode/param cases) |
| Output images | Embedded as hex in JSON | ~500 viewable PNGs + ~5 .bin files |
| Engine files | 5 files, ~1,500 lines | 2 files, ~180 lines |
| Per-op branches | ~120 if/elif across backends | 0 (data-driven lookup) |

## 10. What Is NOT in the Engine (intentional)

- ❌ No per-operation parameter handling
- ❌ No type coercion (PIL/RSPIL Python APIs are already identical)
- ❌ No sentinel value expansion (`__CONVERT_TO__`, `__IDENTITY_LUT__`, `__SIMPLE__`)
- ❌ No draw-context lifecycle management (it's just `Draw(img).op(params)`)
- ❌ No temp-file management (output comparison uses in-memory bytes)
- ❌ No RNG seeding (non-deterministic ops use `source: "bytes"` input captured from PIL)

## 11. Migration Plan

### Phase 1: Build engine
1. Create `tests/engine.py` (get_call_style + CALL_STYLE + ASSERT + create_input)
2. Create `scripts/generate_fixtures.py` v2 (imports engine, uses PIL backend)
3. Create `tests/test_parity.py` v2 (imports engine, uses RSPIL backend)
4. Create `tests/fixtures/output/images/` and `tests/fixtures/output/raw/`

### Phase 2: Write and generate fixtures
1. For each operation in manifest, write the fixture JSON skeleton (operation + cases with input/params/assert stubs)
2. Run generate_fixtures.py → populates assert values, saves reference images
3. Run test_parity.py → verify all cases pass or xfail cleanly
4. Iterate: fix Rust bugs, regenerate fixtures, re-test

### Phase 3: Delete old infra
1. Remove `rspil_backend.py`, `pil_backend.py`, `ops_registry.py`, `execution_engine.py`
2. Remove `test_fixture_parity.py`
3. Remove FIXTURE_META from ops_registry (the file can stay if coverage tool still needs REGISTRY)

### Phase 4: Fill gaps
1. Add missing operations' fixtures (25 currently unfixtured)
2. Add edge-case param variations to existing fixtures
3. Verify 100% operation coverage with genuine RSPIL-vs-PIL comparisons

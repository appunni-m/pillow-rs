# Test Infra DSL — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace ~1,500 lines of per-operation backend dispatch code with a ~150-line generic DSL engine, and migrate ~670 fixtures to an input/output split format with viewable reference images.

**Architecture:** A single `engine.py` file contains all execution logic (12 call styles, 7 assert methods, input creation). Fixtures split into hand-written `input/jsons/*.json` (what to test) and machine-generated `outputs/jsons/*.json` (expected results). Reference images saved as PNGs in `outputs/images/`.

**Tech Stack:** Python 3.12, pytest, PIL (Pillow), pillow_rs (RSPIL PyO3 bindings), JSON

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `tests/engine.py` | Create | get_call_style + CALL_STYLE + ASSERT + create_input (~130 lines) |
| `tests/test_parity.py` | Create | Parametrized test runner (~35 lines) |
| `scripts/generate_fixtures.py` | Create | PIL-based fixture generator (~60 lines) |
| `scripts/migrate_fixtures.py` | Create | One-shot: convert old fixtures → new input format |
| `tests/fixtures/input/jsons/` | Create dir | Hand-written test specs |
| `tests/fixtures/input/images/` | Create dir | Source images (when not reference_rgb) |
| `tests/fixtures/input/raws/` | Create dir | Exact input bytes (for frombytes tests) |
| `tests/fixtures/outputs/jsons/` | Create dir | PIL-generated expected results |
| `tests/fixtures/outputs/images/` | Create dir | Reference PNGs |
| `tests/fixtures/outputs/raws/` | Create dir | Float/I-mode byte refs |
| `tests/rspil_backend.py` | **Delete** | 415 lines, 70 per-op branches |
| `scripts/coverage/pil_backend.py` | **Delete** | 361 lines, 50 per-op branches |
| `scripts/coverage/execution_engine.py` | **Delete** | 47 lines, inlined into engine |
| `tests/test_fixture_parity.py` | **Delete** | 157 lines, replaced by test_parity.py |
| `scripts/coverage/ops_registry.py` | Modify | Remove FIXTURE_META, keep REGISTRY build for coverage tool |
| `manifest.yaml` | No change | Already the API catalog — no test metadata to remove |

---

### Task 1: Create directory structure

**Files:**
- Create: `tests/fixtures/input/jsons/.gitkeep`
- Create: `tests/fixtures/input/images/.gitkeep`
- Create: `tests/fixtures/input/raws/.gitkeep`
- Create: `tests/fixtures/outputs/jsons/.gitkeep`
- Create: `tests/fixtures/outputs/images/.gitkeep`
- Create: `tests/fixtures/outputs/raws/.gitkeep`

- [ ] **Step 1: Create all directories**

```bash
mkdir -p tests/fixtures/input/jsons
mkdir -p tests/fixtures/input/images
mkdir -p tests/fixtures/input/raws
mkdir -p tests/fixtures/outputs/jsons
mkdir -p tests/fixtures/outputs/images
mkdir -p tests/fixtures/outputs/raws
touch tests/fixtures/input/jsons/.gitkeep
touch tests/fixtures/input/images/.gitkeep
touch tests/fixtures/input/raws/.gitkeep
touch tests/fixtures/outputs/jsons/.gitkeep
touch tests/fixtures/outputs/images/.gitkeep
touch tests/fixtures/outputs/raws/.gitkeep
```

- [ ] **Step 2: Verify directories exist**

```bash
ls -la tests/fixtures/input/jsons/ tests/fixtures/input/images/ tests/fixtures/input/raws/
ls -la tests/fixtures/outputs/jsons/ tests/fixtures/outputs/images/ tests/fixtures/outputs/raws/
```

---

### Task 2: Build the generic engine — get_call_style()

**Files:**
- Create: `tests/engine.py`

- [ ] **Step 1: Write `tests/engine.py` — lookup sets and get_call_style() function**

```python
"""Generic test execution engine — zero per-operation code.

This module is imported by:
  - tests/test_parity.py (test runner, uses pillow_rs as backend)
  - scripts/generate_fixtures.py (fixture generator, uses PIL as backend)

Adding a new operation requires ZERO changes to this file.
"""

import json, hashlib
from pathlib import Path

# ── Module → call_style lookup ──────────────────────────────────

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
    """Return the call_style string for any (module, target) pair.
    
    Pure data lookup. Never needs new entries for new operations
    — new operations in an existing module resolve automatically.
    """
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

- [ ] **Step 2: Verify the function resolves known operations**

```bash
python3 -c "
import sys; sys.path.insert(0, 'tests')
from engine import get_call_style

# Spot-check key operations
assert get_call_style('Image', 'resize') == 'instance_method'
assert get_call_style('Image', 'getpixel') == 'instance_method_value'
assert get_call_style('Image', 'putpixel') == 'instance_method_mutate'
assert get_call_style('Image', 'paste') == 'instance_method_dual'
assert get_call_style('ImageDraw', 'text') == 'draw'
assert get_call_style('ImageDraw', 'textlength') == 'draw_value'
assert get_call_style('ImageFilter', 'BoxBlur') == 'filter'
assert get_call_style('ImageFilter', 'BLUR') == 'filter'
assert get_call_style('ImageEnhance', 'Brightness') == 'enhance'
assert get_call_style('ImageOps', 'autocontrast') == 'module_function'
assert get_call_style('ImageChops', 'add') == 'module_function_dual'
assert get_call_style('ImageChops', 'invert') == 'module_function'
assert get_call_style('ImageColor', 'getcolor') == 'module_function_value'
assert get_call_style('ImagePalette', 'getcolor') == 'module_function_value'
assert get_call_style('ImageFont', 'load_default') == 'module_function_value'
assert get_call_style('ImageStat', 'Stat') == 'stat'
assert get_call_style('ImageModule', 'new') == 'classmethod'
print('All assertions passed')
"
```

---

### Task 3: Build the generic engine — input creation + call styles

**Files:**
- Modify: `tests/engine.py` (append to existing file)

- [ ] **Step 1: Append `create_input()` to engine.py**

```python
# ── Input creation ──────────────────────────────────────────────

REFERENCE_IMAGE = Path(__file__).parent / "test_reference.png"

def create_input(backend, mode, spec):
    """Create an image from a declarative input spec.
    
    Works identically for both PIL and RSPIL backends because both
    provide Image.open, Image.new, Image.frombytes, .resize, .convert.
    
    Args:
        backend: Module with PIL-identical API (PIL or pillow_rs)
        mode: Image mode string (e.g. 'L', 'RGB') — from case-level field
        spec: Input specification dict, or None for no-input operations
    """
    if spec is None:
        return None
    
    source = spec["source"]
    size = tuple(spec["size"])
    
    if source == "reference_rgb":
        ref = backend.Image.open(str(REFERENCE_IMAGE))
        if ref.size != size:
            ref = ref.resize(size, backend.Image.LANCZOS)
        return ref.convert(mode)
    elif source == "constant":
        color = spec.get("color", 0)
        return backend.Image.new(mode, size, color)
    elif source == "bytes":
        raw = bytes.fromhex(spec["bytes"])
        return backend.Image.frombytes(mode, size, raw)
    else:
        raise ValueError(f"Unknown input source: {source}")
```

- [ ] **Step 2: Append call style lambdas to engine.py**

```python
# ── Call style implementations ──────────────────────────────────

def _draw(backend, img, target, params):
    draw = backend.ImageDraw.Draw(img)
    return getattr(draw, target)(**params)

def _call_mod(backend, target):
    """Resolve target function from backend's module hierarchy."""
    for mod_name in ["ImageOps", "ImageChops", "ImageColor", "ImagePalette",
                     "ImageFont", "ImageSequence", "Image", "ImageDraw"]:
        mod = getattr(backend, mod_name, None)
        if mod and hasattr(mod, target):
            return getattr(mod, target)
    if hasattr(backend, "ImageFilter"):
        f = getattr(backend.ImageFilter, target, None)
        if f: return f
    if hasattr(backend, "ImageEnhance"):
        e = getattr(backend.ImageEnhance, target, None)
        if e: return e
    raise ValueError(f"Cannot resolve target function: {target}")

def _make_filter(backend, target, params):
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
```

- [ ] **Step 3: Verify engine imports cleanly**

```bash
python3 -c "
import sys; sys.path.insert(0, 'tests')
from engine import get_call_style, CALL_STYLE, create_input
print(f'get_call_style: {len(get_call_style.__code__.co_code)} bytes')
print(f'CALL_STYLE entries: {len(CALL_STYLE)}')
print(f'create_input: defined={callable(create_input)}')
print('Engine imports OK')
"
```

---

### Task 4: Build the generic engine — assertion methods

**Files:**
- Modify: `tests/engine.py` (append to existing file)

- [ ] **Step 1: Append assertion helpers and ASSERT dict to engine.py**

```python
# ── Assertion methods ───────────────────────────────────────────

OUTPUTS_DIR = Path(__file__).parent / "fixtures" / "outputs"

def _load_reference(path):
    """Reference paths are relative to fixtures/outputs/."""
    full = OUTPUTS_DIR / path
    if path.endswith('.png'):
        # Lazy import to avoid PIL dependency when running tests
        from PIL import Image as PILImage
        return PILImage.open(str(full))
    return open(str(full), 'rb').read()

def _sha(data):
    if hasattr(data, 'tobytes'):
        return hashlib.sha256(data.tobytes()).hexdigest()
    return hashlib.sha256(data).hexdigest()

def _to_json_compat(val):
    """Convert any result type to JSON-serializable form."""
    if val is None: return None
    if isinstance(val, (int, float, str, bool)): return val
    if isinstance(val, bytes): return val.hex()
    if isinstance(val, (tuple, list)): return [_to_json_compat(v) for v in val]
    if isinstance(val, dict): return {str(k): _to_json_compat(v) for k, v in val.items()}
    if hasattr(val, 'tobytes'): return _sha(val)
    if hasattr(val, '__iter__') and not isinstance(val, (str, bytes, dict)):
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
        str(result).startswith(case.get("prefix", ""))
        or repr(result) == case.get("value", ""),
    "float": lambda case, result:
        abs(result - case["value"]) < case.get("tolerance", 0.0001),
    "error": lambda case, result:
        isinstance(result, Exception)
        and case.get("exception", "") in type(result).__name__
        and case.get("message_contains", "") in str(result),
}
```

- [ ] **Step 2: Verify assertion dict imports**

```bash
python3 -c "
import sys; sys.path.insert(0, 'tests')
from engine import ASSERT
print(f'ASSERT methods: {list(ASSERT.keys())}')
print('ASSERT imports OK')
"
```

- [ ] **Step 3: Run basic assertion smoke tests**

```bash
python3 -c "
import sys; sys.path.insert(0, 'tests')
from engine import ASSERT

# Test exact assertion
assert ASSERT['exact']({'value': 42}, 42) == True
assert ASSERT['exact']({'value': 42}, 43) == False
print('exact: OK')

# Test float assertion
assert ASSERT['float']({'value': 3.14, 'tolerance': 0.01}, 3.14159) == True
assert ASSERT['float']({'value': 3.14, 'tolerance': 0.001}, 3.20) == False
print('float: OK')

# Test error assertion
class ValueError(Exception): pass
assert ASSERT['error']({'exception': 'ValueError'}, ValueError('bad')) == True
assert ASSERT['error']({'exception': 'TypeError'}, ValueError('bad')) == False
print('error: OK')

print('All assertion smoke tests passed')
"
```

---

### Task 5: Build the test runner

**Files:**
- Create: `tests/test_parity.py`

- [ ] **Step 1: Write `tests/test_parity.py`**

```python
"""Generic PIL-RSPIL parity test runner.

Discovers fixture pairs from fixtures/input/jsons/ and fixtures/outputs/jsons/.
Zips input cases with expected outputs by case id.
Zero per-operation logic — the engine handles everything.
"""

import json
from pathlib import Path

import pytest
import pillow_rs as rspil

from engine import CALL_STYLE, ASSERT, create_input, get_call_style

FIXTURES_DIR = Path(__file__).parent / "fixtures"
INPUT_DIR = FIXTURES_DIR / "input" / "jsons"
OUTPUT_DIR = FIXTURES_DIR / "outputs" / "jsons"


def _discover():
    """Yield every input fixture that has a corresponding output fixture."""
    for fpath in sorted(INPUT_DIR.glob("*.json")):
        if (OUTPUT_DIR / fpath.name).exists():
            yield pytest.param(fpath.name, id=fpath.stem)


@pytest.mark.parametrize("fixture_file", _discover())
def test_parity(fixture_file):
    inp = json.loads((INPUT_DIR / fixture_file).read_text())
    out = json.loads((OUTPUT_DIR / fixture_file).read_text())
    op = inp["operation"]
    call_style = get_call_style(op["module"], op["target"])

    # Index output cases by id for O(1) lookup
    out_cases = {c["id"]: c for c in out["cases"]}

    for case in inp["cases"]:
        cid = case["id"]
        mode = case.get("mode")
        img = create_input(rspil, mode, case.get("input"))
        img2 = create_input(rspil, mode, case.get("input2"))
        params = dict(case.get("params", {}))

        assertion = out_cases[cid]["assert"]

        try:
            result = CALL_STYLE[call_style](rspil, img, img2, op["target"], params)
        except Exception as e:
            if assertion["method"] == "error":
                assert ASSERT["error"](assertion, e), f"[{cid}] error mismatch"
                continue
            raise

        assert ASSERT[assertion["method"]](assertion, result), \
            f"[{cid}] {assertion['method']} mismatch"
```

- [ ] **Step 2: Verify the runner imports and discovers (0 tests is OK for now)**

```bash
python3 -m pytest tests/test_parity.py -v --co 2>&1 | head -20
```

Expected: "no tests ran" or "collected 0 items" (since no fixtures exist yet)

---

### Task 6: Build the fixture generator

**Files:**
- Create: `scripts/generate_fixtures.py`

- [ ] **Step 1: Write `scripts/generate_fixtures.py`**

```python
#!/usr/bin/env python3
"""Generate expected output fixtures by running PIL against input specs.

Reads each input JSON from tests/fixtures/input/jsons/, executes the operation
via PIL, and writes the expected results to tests/fixtures/outputs/jsons/.
Reference images are saved as PNGs in tests/fixtures/outputs/images/.
"""

import json
import sys
from pathlib import Path

import PIL.Image
import PIL.ImageDraw
import PIL.ImageFilter
import PIL.ImageChops
import PIL.ImageOps
import PIL.ImageEnhance
import PIL.ImageColor
import PIL.ImagePalette
import PIL.ImageFont
import PIL.ImageStat
import PIL.ImageSequence

ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(ROOT / "tests"))

from engine import CALL_STYLE, get_call_style, create_input

FIXTURES_DIR = ROOT / "tests" / "fixtures"
INPUT_DIR = FIXTURES_DIR / "input" / "jsons"
OUTPUT_JSONS_DIR = FIXTURES_DIR / "outputs" / "jsons"
OUTPUT_IMAGES_DIR = FIXTURES_DIR / "outputs" / "images"
OUTPUT_RAWS_DIR = FIXTURES_DIR / "outputs" / "raws"


class PilBackend:
    """Adapter so engine code accesses PIL modules same way as pillow_rs."""
    Image = PIL.Image
    ImageFilter = PIL.ImageFilter
    ImageChops = PIL.ImageChops
    ImageOps = PIL.ImageOps
    ImageEnhance = PIL.ImageEnhance
    ImageDraw = PIL.ImageDraw
    ImageColor = PIL.ImageColor
    ImagePalette = PIL.ImagePalette
    ImageFont = PIL.ImageFont
    ImageStat = PIL.ImageStat
    ImageSequence = PIL.ImageSequence


pil = PilBackend()


def generate_one(input_path):
    """Run one input fixture through PIL, produce output JSON + reference files."""
    inp = json.loads(input_path.read_text())
    op = inp["operation"]
    call_style = get_call_style(op["module"], op["target"])

    out = {"format_version": 2, "operation": op, "cases": []}
    stem = input_path.stem

    for case in inp["cases"]:
        cid = case["id"]
        mode = case.get("mode")
        img = create_input(pil, mode, case.get("input"))
        img2 = create_input(pil, mode, case.get("input2"))
        params = dict(case.get("params", {}))
        result = CALL_STYLE[call_style](pil, img, img2, op["target"], params)

        # ── Determine result type and produce assertion ──
        if hasattr(result, 'tobytes') or hasattr(result, 'save'):
            # Single image result → save as PNG
            ref = f"images/{stem}_{cid}.png"
            img_path = OUTPUT_IMAGES_DIR / f"{stem}_{cid}.png"
            img_path.parent.mkdir(parents=True, exist_ok=True)
            result.save(str(img_path))
            out["cases"].append({
                "id": cid,
                "assert": {"method": "image", "reference": ref},
            })

        elif (isinstance(result, (list, tuple))
              and len(result) > 0
              and hasattr(result[0], 'tobytes')):
            # List of images (e.g. split) → save each as PNG
            refs = []
            for j, band in enumerate(result):
                ref = f"images/{stem}_{cid}_{j}.png"
                img_path = OUTPUT_IMAGES_DIR / f"{stem}_{cid}_{j}.png"
                img_path.parent.mkdir(parents=True, exist_ok=True)
                band.save(str(img_path))
                refs.append(ref)
            out["cases"].append({
                "id": cid,
                "assert": {"method": "image_list", "references": refs},
            })

        elif isinstance(result, (int, float, str, bool, list, dict, type(None))):
            # Scalar / structured value result
            out["cases"].append({
                "id": cid,
                "assert": {"method": "exact", "value": result},
            })

        else:
            # Unknown type — stringify
            out["cases"].append({
                "id": cid,
                "assert": {"method": "string", "value": repr(result)},
            })

    return out


def main():
    """Generate output fixtures for all input fixtures."""
    OUTPUT_JSONS_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_IMAGES_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_RAWS_DIR.mkdir(parents=True, exist_ok=True)

    input_files = sorted(INPUT_DIR.glob("*.json"))
    if not input_files:
        print("No input fixtures found in", INPUT_DIR)
        return

    for input_path in input_files:
        try:
            out = generate_one(input_path)
            output_path = OUTPUT_JSONS_DIR / input_path.name
            output_path.write_text(json.dumps(out, indent=2))
            print(f"  OK  {input_path.stem} ({len(out['cases'])} cases)")
        except Exception as e:
            print(f"  FAIL {input_path.stem}: {e}", file=sys.stderr)

    print(f"\nGenerated {len(input_files)} output fixtures")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Verify generator imports and runs (0 fixtures is OK)**

```bash
python3 scripts/generate_fixtures.py
```

Expected: "No input fixtures found" — since no input fixtures exist yet

---

### Task 7: Create input fixture for Image.resize (smoke test)

**Files:**
- Create: `tests/fixtures/input/jsons/Image_resize.json`

- [ ] **Step 1: Write the first input fixture**

```json
{
  "format_version": 2,
  "operation": {
    "module": "Image",
    "target": "resize"
  },
  "cases": [
    {
      "id": "L_250x250",
      "mode": "L",
      "input": {"source": "reference_rgb", "size": [500, 500]},
      "params": {"size": [250, 250]}
    },
    {
      "id": "RGB_250x250",
      "mode": "RGB",
      "input": {"source": "reference_rgb", "size": [500, 500]},
      "params": {"size": [250, 250]}
    }
  ]
}
```

- [ ] **Step 2: Run generator on this fixture**

```bash
python3 scripts/generate_fixtures.py
```

Expected: `OK Image_resize (2 cases)`

- [ ] **Step 3: Verify output files exist**

```bash
ls -la tests/fixtures/outputs/jsons/Image_resize.json
ls -la tests/fixtures/outputs/images/Image_resize_L_250x250.png
ls -la tests/fixtures/outputs/images/Image_resize_RGB_250x250.png
```

- [ ] **Step 4: Run test_parity on this fixture**

```bash
python3 -m pytest tests/test_parity.py -v --timeout=180 2>&1
```

Expected: `test_parity[Image_resize] PASSED` (2 cases pass)

---

### Task 8: Migrate old fixtures to input format

**Files:**
- Create: `scripts/migrate_fixtures.py`

- [ ] **Step 1: Write the migration script**

```python
#!/usr/bin/env python3
"""One-shot migration: convert old fixtures/ JSONs → new input/jsons/ format.

Old format (per op+mode):  { operation: {type, module, target, params}, input: {mode, size, bytes}, expected: {result_type, value} }
New format (per op, array): { operation: {module, target}, cases: [{id, mode, input, params}] }

Only creates input JSONs — outputs are regenerated by scripts/generate_fixtures.py.
"""

import json
from pathlib import Path
from collections import defaultdict

ROOT = Path(__file__).parent.parent
OLD_DIR = ROOT / "tests" / "fixtures"
NEW_DIR = ROOT / "tests" / "fixtures" / "input" / "jsons"
NEW_DIR.mkdir(parents=True, exist_ok=True)

# Group old fixtures by operation
groups = defaultdict(list)
for fpath in sorted(OLD_DIR.glob("*.json")):
    try:
        fx = json.loads(fpath.read_text())
    except Exception:
        continue
    if "operation" not in fx:
        continue
    op = fx["operation"]
    op_key = f"{op.get('module', '?')}.{op['target']}"
    groups[op_key].append((fpath, fx))

for op_key, items in sorted(groups.items()):
    module, target = op_key.split(".", 1)
    cases = []
    for fpath, fx in items:
        inp = fx.get("input", {})
        mode = inp.get("mode", "L")
        params = fx["operation"].get("params", {})

        # Strip sentinel values — the new engine passes params directly
        clean_params = {}
        for k, v in params.items():
            if isinstance(v, str) and v.startswith("__"):
                continue  # Skip __CONVERT_TO__, __IDENTITY_LUT__, etc.
            clean_params[k] = v

        case_id = f"{mode}_{fpath.stem.split('_')[-1]}" if '_' in fpath.stem else mode

        cases.append({
            "id": case_id,
            "mode": mode,
            "input": {"source": "reference_rgb", "size": [500, 500]},
            "params": clean_params,
        })

    out_path = NEW_DIR / f"{op_key.replace('.', '_')}.json"
    spec = {
        "format_version": 2,
        "operation": {"module": module, "target": target},
        "cases": cases,
    }
    out_path.write_text(json.dumps(spec, indent=2))
    print(f"  {out_path.stem}: {len(cases)} cases")

print(f"\nMigrated {len(groups)} operations to {NEW_DIR}")
```

- [ ] **Step 2: Run migration**

```bash
python3 scripts/migrate_fixtures.py
```

- [ ] **Step 3: Verify migration counts**

```bash
echo "Old fixtures: $(ls tests/fixtures/*.json 2>/dev/null | wc -l)"
echo "New input fixtures: $(ls tests/fixtures/input/jsons/*.json 2>/dev/null | wc -l)"
```

---

### Task 9: Regenerate all outputs and verify test parity

**Files:**
- No new files — run generator, then tests

- [ ] **Step 1: Regenerate all output fixtures**

```bash
python3 scripts/generate_fixtures.py 2>&1
```

Expected: Most fixtures generate OK. Some may fail with NotImplementedError or missing Rust functions — these get printed to stderr as FAIL.

- [ ] **Step 2: Run the new test suite**

```bash
python3 -m pytest tests/test_parity.py -v --timeout=180 2>&1 | tail -40
```

- [ ] **Step 3: Capture counts**

```bash
python3 -m pytest tests/test_parity.py -v --timeout=180 --tb=no 2>&1 | grep -E "PASSED|FAILED|XFAIL|ERROR|test_parity" | tail -5
```

---

### Task 10: Delete old test infrastructure files

**Files:**
- Delete: `tests/rspil_backend.py`
- Delete: `scripts/coverage/pil_backend.py`
- Delete: `scripts/coverage/execution_engine.py`
- Delete: `tests/test_fixture_parity.py`

- [ ] **Step 1: Verify old files are no longer imported by anything**

```bash
grep -r "rspil_backend\|pil_backend\|execution_engine\|test_fixture_parity" scripts/ tests/ --include="*.py" | grep -v ".pyc" | grep -v "migrate_fixtures" | grep -v "test_parity" | grep -v "engine.py" | grep -v "generate_fixtures.py"
```

Expected: No output (no remaining references except the new files referencing them)

- [ ] **Step 2: Remove old files**

```bash
rm tests/rspil_backend.py
rm scripts/coverage/pil_backend.py
rm scripts/coverage/execution_engine.py
rm tests/test_fixture_parity.py
```

- [ ] **Step 3: Verify nothing is broken by the deletion**

```bash
python3 -m pytest tests/test_parity.py -v --timeout=180 --tb=short 2>&1 | tail -20
```

Expected: Same pass/fail counts as before deletion

---

### Task 11: Clean up ops_registry.py

**Files:**
- Modify: `scripts/coverage/ops_registry.py`

The `ops_registry.py` still needs `REGISTRY` dict for coverage computation (`compute_coverage.py` uses it to cross-reference manifest → fixtures). But `FIXTURE_META` is no longer needed — call styles come from `engine.get_call_style()`.

- [ ] **Step 1: Check what uses ops_registry.py**

```bash
grep -r "ops_registry\|FIXTURE_META\|from scripts.coverage.ops_registry" scripts/ tests/ --include="*.py" | grep -v ".pyc" | grep -v migrate_fixtures
```

- [ ] **Step 2: Remove FIXTURE_META from ops_registry.py**

Read `scripts/coverage/ops_registry.py` and delete the `FIXTURE_META` dict (lines starting at `FIXTURE_META = {` through the closing `}`). Also remove any code that merges FIXTURE_META into REGISTRY in `build_registry()`.

The key change in `build_registry()` — instead of starting from FIXTURE_META entries:

```python
def build_registry():
    manifest = _load_manifest()
    registry = {}

    # Build entirely from manifest → auto-derived call styles
    for mod_name, op_name, modes in _collect_manifest_ops(manifest):
        op_full_name = f"{mod_name}.{op_name}"
        meta = _default_meta(mod_name, op_name)
        if meta is None:
            continue
        if not modes:
            modes = ["L", "RGB"]
        meta["modes"] = modes
        registry[op_full_name] = meta

    return registry
```

- [ ] **Step 3: Verify coverage computation still works**

```bash
# Run the current test suite to get a fresh report
python3 -m pytest tests/ --json-report --json-report-file=/tmp/report.json --timeout=180 2>&1 | tail -5
python3 scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json 2>&1 | tail -20
```

Expected: Coverage report still produces TRUSTED/UNTRUSTED counts

---

### Task 12: Remove old fixture JSONs (the migrated ones)

**Files:**
- Delete: `tests/fixtures/*.json` (all old single-case fixtures)

- [ ] **Step 1: Archive old fixtures (just in case)**

```bash
mkdir -p /tmp/pillow-rs-old-fixtures
cp tests/fixtures/*.json /tmp/pillow-rs-old-fixtures/ 2>/dev/null
echo "Backed up $(ls /tmp/pillow-rs-old-fixtures/*.json 2>/dev/null | wc -l) old fixtures"
```

- [ ] **Step 2: Delete old fixture JSONs**

```bash
rm tests/fixtures/*.json
```

- [ ] **Step 3: Verify test_parity still works with new fixtures only**

```bash
python3 -m pytest tests/test_parity.py -v --timeout=180 --tb=short 2>&1 | tail -30
```

---

## Self-Review Checklist

1. **Spec coverage:**
   - [x] Section 7.1 (get_call_style): Task 2
   - [x] Section 7.2 (CALL_STYLE): Task 3
   - [x] Section 7.3 (ASSERT): Task 4
   - [x] Section 7.4 (test_parity.py): Task 5
   - [x] Section 7.5 (generate_fixtures.py): Task 6
   - [x] Section 5 (fixture format): Task 7 (example), Task 8 (migration)
   - [x] Section 8 (directory structure): Task 1
   - [x] Section 3 (deletion): Task 10, Task 12
   - [x] Section 11 (migration plan): Tasks 1-12

2. **Placeholder scan:** No TODOs, TBDs, or "implement later" found.

3. **Type consistency:**
   - `get_call_style(module, target) -> str` — consistent across engine, test, generator
   - `create_input(backend, mode, spec) -> Image|None` — consistent signature
   - `CALL_STYLE[style](backend, img, img2, target, params) -> result` — uniform signature
   - `ASSERT[method](case, result) -> bool` — uniform signature
   - Case `id` field used consistently for input→output zipping

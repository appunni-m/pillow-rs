# String Assertion No-Op Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 201 test cases that currently pass without validating any output by making the `string` assertion compare actual values, adding a `tuple` assertion for mixed-type results, and normalizing PIL-internal objects to standard types in the fixture generator.

**Architecture:** The `string` assertion short-circuits via `"".startswith("")` → always `True`. Fix: swap the `or` order so `repr(result) == value` runs first. For PIL-internal returns (ImagingCore, PixelAccess, fonts), normalize them to standard types (Image, bytes, list) in the fixture generator so the existing assertion dispatch picks the right comparison method. For operations where PIL and RSPIL return fundamentally different types (getim, load, getdata), change the shared call_style to return `img.tobytes()` so both sides produce comparable bytes. For font constructors, change call_style to render `getmask("A")` and return Image. A new `tuple` assertion dispatches element-wise to existing assertions for mixed-type tuples like getmask2's `(Image, offset)`.

**Tech Stack:** Python (tests/engine.py, scripts/generate_fixtures.py), PIL for fixture generation, pytest for testing.

---

### Task 1: Fix `string` assertion order in engine.py

**Files:**
- Modify: `tests/engine.py:508-510`

- [ ] **Step 1: Swap the `or` operands**

Replace:
```python
    "string": lambda case, result:
        str(result).startswith(case.get("prefix", ""))
        or repr(result) == case.get("value", ""),
```
With:
```python
    "string": lambda case, result:
        repr(result) == case.get("value", "")
        or str(result).startswith(case.get("prefix", "")),
```

- [ ] **Step 2: Verify syntax**

Run: `python -c "import tests.engine; print('OK')"`
Expected: `OK` (no syntax errors)

- [ ] **Step 3: Run targeted test to confirm Category 2 tests now fail with real mismatch**

Run: `cd tests && python -m pytest test_parity.py -k "getbands or getbbox" -x --timeout=60 2>&1 | tail -20`
Expected: Tests fail because fixture still has `"string"` assertion with `repr()` values that don't match RSPIL's `_to_json_compat` output format. This is expected — we'll fix fixtures in Task 7.

---

### Task 2: Add `tuple` assertion to engine.py

**Files:**
- Modify: `tests/engine.py:498-517`

- [ ] **Step 1: Add `tuple` assertion entry**

Insert after `"json"` assertion (after line 507), before `"string"` assertion:

```python
    "tuple": lambda case, result:
        all(ASSERT[item["method"]](item, result[i])
            for i, item in enumerate(case["items"])),
```

The `ASSERT` dict must be fully defined before the lambda runs. Since this is inside the `ASSERT = {...}` literal itself, and Python dict literals evaluate values left-to-right within the same scope, and `ASSERT` isn't bound yet during dict construction, this should use `ASSERT` as a closure variable that will resolve at call time because lambdas are lazy. 

Wait — this doesn't work. The name `ASSERT` isn't in scope inside a dict comprehension within the dict literal. We need to reference a local that's captured by closure.

Fix: all other entries in ASSERT are self-contained lambdas. The `tuple` assertion needs ASSERT. Use a nested dict lookup pattern instead:

```python
    "tuple": lambda case, result:
        all(
            {
                "image": lambda c, r: _sha(r) == _sha(_load_reference(c["reference"])),
                "image_list": lambda c, r: all(_sha(b) == _sha(_load_reference(ref)) for b, ref in zip(r, c["references"])),
                "exact": lambda c, r: r == c["value"],
                "json": lambda c, r: json.dumps(_to_json_compat(r)) == json.dumps(c["value"]),
                "string": lambda c, r: repr(r) == c.get("value", "") or str(r).startswith(c.get("prefix", "")),
                "float": lambda c, r: abs(r - c["value"]) < c.get("tolerance", 0.0001),
                "error": lambda c, r: isinstance(r, Exception) and c.get("exception", "") in type(r).__name__ and c.get("message_contains", "") in str(r),
            }[item["method"]](item, result[i])
            for i, item in enumerate(case["items"])
        ),
```

That's too large. Better approach: define `ASSERT` as a plain dict first, then add the `tuple` entry that references it:

```python
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
        repr(result) == case.get("value", "")
        or str(result).startswith(case.get("prefix", "")),
    "float": lambda case, result:
        abs(result - case["value"]) < case.get("tolerance", 0.0001),
    "error": lambda case, result:
        isinstance(result, Exception)
        and case.get("exception", "") in type(result).__name__
        and case.get("message_contains", "") in str(result),
}
# Add tuple assertion that dispatches element-wise to ASSERT entries.
# Must be added after ASSERT is defined so the closure captures it correctly.
ASSERT["tuple"] = lambda case, result: all(
    ASSERT[item["method"]](item, result[i])
    for i, item in enumerate(case["items"])
)
```

- [ ] **Step 2: Verify syntax**

Run: `python -c "import tests.engine; print('OK')"`
Expected: `OK`

---

### Task 3: Add `font_constructor` call style to engine.py

**Files:**
- Modify: `tests/engine.py:29-31` (FONT_METHOD_TARGETS — remove if needed)
- Modify: `tests/engine.py:75-82` (get_call_style routing)
- Modify: `tests/engine.py:413-451` (CALL_STYLE dict)
- Add: new `_render_font_mask` function in engine.py

- [ ] **Step 1: Add `_render_font_mask` helper function**

Insert after `_font_truetype` (after line 258):

```python
def _render_font_mask(backend, font, text="A"):
    """Render a single glyph and return as an Image for comparison.

    PIL fonts return ImagingCore; RSPIL fonts return Image.
    Normalize both to Image for uniform comparison.
    """
    from PIL import Image as PILImage
    mask = font.getmask(text)
    if type(mask).__name__ == 'ImagingCore':
        b = bytes(mask)
        return PILImage.frombytes(mask.mode, mask.size, b)
    # RSPIL or PIL Image — may already have tobytes
    if hasattr(mask, 'tobytes'):
        return mask
    if hasattr(mask, 'size') and hasattr(mask, 'mode'):
        b = bytes(mask)
        return PILImage.frombytes(mask.mode, mask.size, b)
    return mask
```

- [ ] **Step 2: Add `_font_constructor` call style function**

Insert after `_render_font_mask`:

```python
def _font_constructor(backend, img, target, params):
    """Create a font via constructor, render 'A', return Image for comparison."""
    font = _call_mod(backend, target)(**params)
    return _render_font_mask(backend, font)
```

- [ ] **Step 3: Add new call style entries to CALL_STYLE dict**

Insert after `"font_truetype"` entry (after line 424):

```python
    "font_constructor": lambda b, img, img2, tgt, p: _font_constructor(b, img, tgt, p),
```

- [ ] **Step 4: Route font constructors to `font_constructor` in get_call_style**

In the `ImageFont` section (lines 75-82), change:

From:
```python
    if module == "ImageFont":
        if target in FONT_METHOD_TARGETS:          return "font_method"
        if target in ("truetype", "load", "load_path"):
            return "font_truetype"
        if target == "TransposedFont":             return "transposed_font"
        if target in ("FreeTypeFont", "ImageFont"):
            return "module_function_value"
        return "module_function_value"
```

To:
```python
    if module == "ImageFont":
        if target in FONT_METHOD_TARGETS:          return "font_method"
        if target in ("truetype", "load", "load_path"):
            return "font_truetype"
        if target == "TransposedFont":             return "transposed_font"
        if target in ("FreeTypeFont",):
            return "font_constructor"
        if target in ("load_default", "load_default_imagefont"):
            return "font_constructor"
        if target in ("ImageFont",):
            return "module_function_value"
        return "module_function_value"
```

Note: `ImageFont` constructor stays as `module_function_value` because `ImageFont()` without a font file can't render `getmask("A")` — it will use `string` assertion with prefix matching.

- [ ] **Step 5: Route `getfont` away from draw_value**

Remove `"getfont"` from DRAW_VALUE_TARGETS (line 27):

From:
```python
DRAW_VALUE_TARGETS = {"textlength", "textbbox", "multiline_textbbox", "getfont"}
```
To:
```python
DRAW_VALUE_TARGETS = {"textlength", "textbbox", "multiline_textbbox"}
```

In `ImageDraw` routing (lines 57-61), add getfont dispatch before the draw_value check:

From:
```python
    if module == "ImageDraw":
        if target in DRAW_VALUE_TARGETS: return "draw_value"
        if target == "bitmap":           return "draw_bitmap"
        if target == "shape":            return "draw_shape"
        return "draw"
```

To:
```python
    if module == "ImageDraw":
        if target in DRAW_VALUE_TARGETS: return "draw_value"
        if target == "getfont":          return "draw_getfont"
        if target == "bitmap":           return "draw_bitmap"
        if target == "shape":            return "draw_shape"
        return "draw"
```

- [ ] **Step 6: Add `draw_getfont` call style**

Add `_draw_getfont` function:

```python
def _draw_getfont(backend, img, target, params):
    """Get default font from ImageDraw and render 'A' for comparison."""
    draw = backend.ImageDraw.Draw(img)
    font = getattr(draw, target)(**params)
    return _render_font_mask(backend, font)
```

Add to CALL_STYLE dict (after `"draw_value"` entry, after line 419):

```python
    "draw_getfont":lambda b, img, img2, tgt, p: _draw_getfont(b, img, tgt, p),
```

- [ ] **Step 7: Verify syntax**

Run: `python -c "import tests.engine; print('OK')"`
Expected: `OK`

---

### Task 4: Add getim/load/getdata call styles (compare parent image bytes)

**Files:**
- Modify: `tests/engine.py:39-49` (get_call_style routing for Image module)
- Modify: `tests/engine.py:413-451` (CALL_STYLE dict)

- [ ] **Step 1: Override getim, load, getdata routing**

In the Image module routing (lines 39-49), add overrides BEFORE the VALUE_TARGETS check:

From:
```python
    if module == "Image":
        if target in IMAGE_CLASSMETHOD_TARGETS:  return "classmethod"
        if target in ("save",):                  return "file_save"
        if target in ("filter",):                return "filter"
        if target in ("open",):                  return "file_open"
        if target in ("toqimage", "toqpixmap"):  return "instance_method_value"
        if target in ("frombytes",):             return "frombytes_instance"
        if target in DUAL_TARGETS:               return "instance_method_dual"
        if target in MUTATE_TARGETS:             return "instance_method_mutate"
        if target in VALUE_TARGETS:              return "instance_method_value"
        return "instance_method"
```

To:
```python
    if module == "Image":
        if target in IMAGE_CLASSMETHOD_TARGETS:  return "classmethod"
        if target in ("save",):                  return "file_save"
        if target in ("filter",):                return "filter"
        if target in ("open",):                  return "file_open"
        if target in ("toqimage", "toqpixmap"):  return "instance_method_value"
        if target in ("frombytes",):             return "frombytes_instance"
        if target in ("getim", "load", "getdata"): return "instance_method_bytes"
        if target in DUAL_TARGETS:               return "instance_method_dual"
        if target in MUTATE_TARGETS:             return "instance_method_mutate"
        if target in VALUE_TARGETS:              return "instance_method_value"
        return "instance_method"
```

Note: `getim`, `load`, and `getdata` stay in VALUE_TARGETS for documentation but the explicit check above takes priority.

- [ ] **Step 2: Add `instance_method_bytes` call style**

Insert in CALL_STYLE dict (after `"instance_method_value"` line 415):

```python
    "instance_method_bytes":lambda b, img, img2, tgt, p: (getattr(img, tgt)(**p), img.tobytes())[1],
```

This calls the target method (getim/load/getdata) for its side effects, then returns `img.tobytes()` for comparison. Both PIL and RSPIL paths produce bytes → `image` assertion works.

- [ ] **Step 3: Verify syntax**

Run: `python -c "import tests.engine; print('OK')"`
Expected: `OK`

---

### Task 5: Add normalization function to fixture generator

**Files:**
- Modify: `scripts/generate_fixtures.py:125-220` (assertion dispatch section)

- [ ] **Step 1: Add `_normalize_pil_result` function**

Insert before `generate_one()` (before line 80):

```python
def _normalize_pil_result(result):
    """Convert PIL internal types to standard types for assertion dispatch.

    - ImagingCore (font mask, getdata) → Image or list
    - Tuple containing ImagingCore (getmask2) → normalize each element
    - Font objects (FreeTypeFont, ImageFont with .font) → rendered mask Image
    - PixelAccess (load) → pass through (handled by call_style change)

    Returns (normalized_result, changed) tuple.
    """
    # Tuple: recursively normalize each element
    if isinstance(result, tuple):
        normalized = []
        any_changed = False
        for r in result:
            nr, ch = _normalize_pil_result(r)
            normalized.append(nr)
            any_changed = any_changed or ch
        if any_changed:
            return tuple(normalized), True
        return result, False

    # ImagingCore
    if type(result).__name__ == 'ImagingCore':
        # Try bytes() — works for font masks (always L-mode)
        try:
            b = bytes(result)
            import PIL.Image
            img = PIL.Image.frombytes(result.mode, result.size, b)
            return img, True
        except (TypeError, ValueError):
            pass
        # Fallback: try iterating (getdata)
        try:
            return list(result), True
        except TypeError:
            pass
        return result, False

    # Font objects with rendering capability
    if hasattr(result, 'getmask'):
        try:
            mask = result.getmask("A")
            mr, _ = _normalize_pil_result(mask)
            return mr, True
        except Exception:
            pass
        return result, False

    return result, False
```

- [ ] **Step 2: Insert normalization call before isinstance dispatch**

In `generate_one()`, after the Qt handling block (after line 142: `result = bytes(ptr)`), but before the `isinstance(result, bytes)` check (line 143), add:

```python
        # Normalize PIL internal types to standard types for comparison
        result, _ = _normalize_pil_result(result)
```

The exact insertion point is after line 142 (end of Qt block) and before line 143 (`if isinstance(result, bytes):`).

- [ ] **Step 3: Verify syntax**

Run: `python -c "import scripts.generate_fixtures; print('OK')"`
Expected: May fail with ImportError due to missing PIL/Qt — use instead:

Run: `python -c "import ast; ast.parse(open('scripts/generate_fixtures.py').read()); print('OK')"`
Expected: `OK`

---

### Task 6: Verify empty test run (pre-regeneration sanity)

**Files:**
- None (read-only verification)

- [ ] **Step 1: Run a sample test to confirm engine changes load**

Run: `cd /home/appunni/work/pil-wasm && python -c "
import sys; sys.path.insert(0, 'tests')
from engine import CALL_STYLE, ASSERT, get_call_style
print('CALL_STYLE keys:', sorted(CALL_STYLE.keys()))
print('ASSERT keys:', sorted(ASSERT.keys()))
print('tuple in ASSERT:', 'tuple' in ASSERT)
print('font_constructor in CALL_STYLE:', 'font_constructor' in CALL_STYLE)
print('instance_method_bytes in CALL_STYLE:', 'instance_method_bytes' in CALL_STYLE)
print('getim call_style:', get_call_style('Image', 'getim'))
print('load call_style:', get_call_style('Image', 'load'))
print('getdata call_style:', get_call_style('Image', 'getdata'))
print('FreeTypeFont call_style:', get_call_style('ImageFont', 'FreeTypeFont'))
print('load_default call_style:', get_call_style('ImageFont', 'load_default'))
print('getfont call_style:', get_call_style('ImageDraw', 'getfont'))
print('ImageFont call_style:', get_call_style('ImageFont', 'ImageFont'))
print('getmask call_style:', get_call_style('ImageFont', 'getmask'))
"
`
Expected output shows all new call styles and the `tuple` assertion key.

---

### Task 7: Regenerate all 28 affected fixture files

**Files:**
- Regenerate: `tests/fixtures/outputs/jsons/` (28 files)
- Regenerate: `tests/fixtures/outputs/images/` (new PNG files)
- Regenerate: `tests/fixtures/outputs/raws/` (new .bin files)

- [ ] **Step 1: Run fixture generator**

Run: `cd /home/appunni/work/pil-wasm && python scripts/generate_fixtures.py --suite 0 2>&1 | tail -40`
Expected: All 28 string-assertion fixtures regenerated with new assertion types. No errors.

- [ ] **Step 2: Check a regenerated fixture to confirm changes**

Run: `python -c "
import json
# Check getmask — should be 'image' now
f = json.loads(open('tests/fixtures/outputs/jsons/ImageFont.getmask.json').read())
for c in f['cases']:
    print(f'{c[\"id\"]}: method={c[\"assert\"][\"method\"]}')
print()

# Check getmask2 — should be 'tuple' now
f2 = json.loads(open('tests/fixtures/outputs/jsons/ImageFont.getmask2.json').read())
for c in f2['cases']:
    a = c['assert']
    if a['method'] == 'tuple':
        items = ', '.join(i['method'] for i in a['items'])
        print(f'{c[\"id\"]}: method=tuple items=[{items}]')
    else:
        print(f'{c[\"id\"]}: method={a[\"method\"]}')

# Check getbands — should be 'json' now (was 'string')
f3 = json.loads(open('tests/fixtures/outputs/jsons/Image.getbands.json').read())
for c in f3['cases']:
    print(f'{c[\"id\"]}: method={c[\"assert\"][\"method\"]} value={c[\"assert\"].get(\"value\", c[\"assert\"].get(\"reference\", \"?\"))}')

# Check getim — should be 'image' now
f4 = json.loads(open('tests/fixtures/outputs/jsons/Image.getim.json').read())
for c in f4['cases']:
    print(f'{c[\"id\"]}: method={c[\"assert\"][\"method\"]} ref={c[\"assert\"].get(\"reference\", \"?\")}')
"
`
Expected: `getmask` → `image`, `getmask2` → `tuple` with `[image, json]` items, `getbands` → `json`, `getim` → `image`.

- [ ] **Step 3: Verify NO fixture still uses String with memory address**

Run: `grep -r 'ImagingCore object at 0x' tests/fixtures/outputs/jsons/`
Expected: No output (zero matches). All memory-address strings eliminated.

- [ ] **Step 4: Verify only ImageFont.ImageFont still uses String**

Run: `grep -r '"method": "string"' tests/fixtures/outputs/jsons/`
Expected: Only `ImageFont.ImageFont.json` contains string assertions. All others migrated to `json`, `image`, or `tuple`.

---

### Task 8: Run full test suite

**Files:**
- None (test execution)

- [ ] **Step 1: Run suite 0 tests**

Run: `cd /home/appunni/work/pil-wasm && bash scripts/build_and_test.sh 2>&1 | tail -50`
Expected: Tests pass. Previously no-op tests now validate actual data. Some may fail if RSPIL output differs from PIL — that's a feature, not a bug: we now catch real differences.

- [ ] **Step 2: If failures, analyze and fix**

For any failures, check if the failure is:
a. **Assertion method mismatch** — e.g., fixture has `json` but result needs different handling → fix normalization or call_style
b. **Genuine RSPIL parity gap** — output differs from PIL → mark as xfail, file bug
c. **Fixture generation error** — normalize produced wrong type → fix normalization logic

- [ ] **Step 3: Run suite 1 tests**

Run: `cd /home/appunni/work/pil-wasm && bash scripts/build_and_test.sh 1 2>&1 | tail -50`
Expected: Similar analysis as Step 2.

---

### Task 9: Final verification

**Files:**
- None (verification commands)

- [ ] **Step 1: Count assertion types in output fixtures**

Run: `cd /home/appunni/work/pil-wasm && grep -roh '"method": "[^"]*"' tests/fixtures/outputs/jsons/ | sort | uniq -c | sort -rn`
Expected: Shows distribution of assertion methods. `string` should appear only for `ImageFont.ImageFont.json`.

- [ ] **Step 2: Verify no broken backward compatibility**

Run: `grep -rn 'ImagingCore object at' tests/fixtures/outputs/`
Expected: Zero results — all memory address references gone.

- [ ] **Step 3: Run lint.sh**

Run: `cd /home/appunni/work/pil-wasm && bash scripts/lint.sh 2>&1 | tail -20`
Expected: Clean lint pass (or pre-existing warnings unrelated to our changes).

---

### Change Summary

| File | Lines changed | What |
|------|---------------|------|
| `tests/engine.py:508-510` | 2 | Swap `string` assertion `or` order |
| `tests/engine.py:517+` | 4 | Add `tuple` assertion after ASSERT dict |
| `tests/engine.py:27` | 1 | Remove `getfont` from DRAW_VALUE_TARGETS |
| `tests/engine.py:39-49` | 1 | Route getim/load/getdata → `instance_method_bytes` |
| `tests/engine.py:57-61` | 1 | Route getfont → `draw_getfont` |
| `tests/engine.py:75-82` | 4 | Route font constructors → `font_constructor` |
| `tests/engine.py:251+` | 25 | Add `_render_font_mask`, `_font_constructor`, `_draw_getfont` |
| `tests/engine.py:415-451` | 3 | Add `font_constructor`, `draw_getfont`, `instance_method_bytes` to CALL_STYLE |
| `scripts/generate_fixtures.py:80+` | 50 | Add `_normalize_pil_result` function |
| `scripts/generate_fixtures.py:142+` | 2 | Insert normalization call |
| 28 fixture JSONs | auto | Regenerated with new assertion types |
| Reference images/raws | auto | New PNG/bin files for image assertions |

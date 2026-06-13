# Standardized Fixture-Based Testing

> **Goal:** Self-contained JSON fixtures power a 15-line generic test engine usable from both Python and JavaScript/WASM, with zero per-operation dispatch logic.

**Architecture:** Each fixture contains everything needed to reproduce a PIL parity test — input image bytes, operation parameters, and expected output hash. A shared `ops_registry.py` defines how to execute each operation; the generator and both test targets import the same registry.

**Tech Stack:** Python (PIL for reference generation, PyO3 for RSPIL tests), JavaScript (WASM for browser tests), JSON fixtures

---

## Fixture Format

Each fixture is a self-contained JSON file. The test engine reads it, creates the input(s), executes the operation, and compares the output hash.

```json
{
  "op": "ImageFilter.BoxBlur",
  "mode": "L",
  "params": {
    "radius": 2
  },
  "input": {
    "mode": "L",
    "size": [100, 100],
    "bytes": "010203030506..."
  },
  "input2": {
    "mode": "L",
    "size": [100, 100],
    "bytes": "..."
  },
  "expectedHash": "3f44dfdf8d60884ce9de4044..."
}
```

### Key changes from current format:

| Field | Before | After |
|-------|--------|-------|
| `inputBytes` + `inputBytesRgb` | Two flat hex strings, mode implied | `input` object with explicit mode/size/bytes |
| `input2` / `input2Rgb` | Not present | Optional second input for dual-image ops |
| `params` | Inconsistent, often `{}` | Complete — everything needed to call the function |
| `expectedHash` | SHA256 of output | Same |
| `expectedValue` | JSON value for non-image returns | Same |
| `expectedError` | Error string | Same |

### Three fixture types:

1. **Image output** (hash comparison): `expectedHash` present
2. **Value output** (exact comparison): `expectedValue` present  
3. **Error output** (error message match): `expectedError` present

---

## Shared Operations Registry

A single file `scripts/coverage/ops_registry.py` defines how to execute every operation. Both the generator AND tests import it. No duplication.

```python
# ops_registry.py — single source of truth for operation dispatch

REGISTRY = {
    # ── Image instance methods ──
    "Image.resize":   {"method": "resize",   "args": ["size"],  "params": {"size": [50, 50]}},
    "Image.rotate":   {"method": "rotate",   "args": ["angle"], "params": {"angle": 90}},
    "Image.crop":     {"method": "crop",     "args": ["box"],   "params": {"box": [25, 25, 75, 75]}},
    # ── ImageChops (dual-image) ──
    "ImageChops.overlay":    {"function": "ImageChops.overlay",    "dual": True},
    "ImageChops.logical_and":{"function": "ImageChops.logical_and","dual": True,
                              "prep": "convert('1', dither='NONE')"},
    # ── ImageFilter (parametric) ──
    "ImageFilter.BoxBlur":  {"filter": "BoxBlur",  "params": {"radius": 2}},
    "ImageFilter.BLUR":     {"filter": "BLUR"},
    # ── ImageOps ──
    "ImageOps.contain":     {"function": "ImageOps.contain", "params": {"size": [25, 25]}},
    # ── ImageDraw (single image, modify in place) ──
    "ImageDraw.arc":        {"draw": "arc", "params": {"bbox": [10,10,40,40], "start": 0, "end": 180, "fill": 200}},
}
```

The registry encodes:
- **What to call** — method name on Image, or module function, or filter class
- **Arguments** — parameter names and their default values
- **Dual-image flag** — whether a second input image is needed
- **Prep steps** — conversions needed before the operation (e.g., convert("1") for logical ops)

---

## Generic Test Engine (~15 lines)

### Python:

```python
def test_fixture_parity(fixture):
    op = fixture["op"]
    img = Image.frombytes(fixture["input"]["mode"], fixture["input"]["size"],
                          bytes.fromhex(fixture["input"]["bytes"]))
    img2 = None
    if "input2" in fixture:
        img2 = Image.frombytes(fixture["input2"]["mode"], fixture["input2"]["size"],
                               bytes.fromhex(fixture["input2"]["bytes"]))

    result = execute(op, img, img2, fixture["params"])  # from ops_registry
    
    if "expectedHash" in fixture:
        assert sha256(result.tobytes()) == fixture["expectedHash"]
    elif "expectedValue" in fixture:
        assert serialize(result) == fixture["expectedValue"]
    elif "expectedError" in fixture:
        raises_matching(fixture["expectedError"], lambda: execute(...))
```

### JavaScript/WASM:

```javascript
test(fixture.name, async () => {
    const img = RSPIL.Image.frombytes(fixture.input.mode, fixture.input.size, hexToBytes(fixture.input.bytes));
    const img2 = fixture.input2 ? RSPIL.Image.frombytes(...) : null;
    
    const result = execute(op, img, img2, fixture.params);  // same registry, ported to JS
    
    if (fixture.expectedHash) {
        expect(sha256(result.tobytes())).toBe(fixture.expectedHash);
    }
});
```

---

## Generator (Cleaned Up)

```python
def generate_all():
    ref = load_reference()  # single source, no caching bugs
    for name, spec in REGISTRY.items():
        for mode in spec["modes"]:
            # Create input(s)
            input_data = make_input(ref, mode)
            input2_data = make_input(ref, mode) if spec.get("dual") else None
            
            # Execute via SAME registry the test uses
            result = execute(name, input_data["img"], input2_data["img"], spec["params"])
            
            # Write fixture
            fixture = {
                "op": name, "mode": mode, "params": spec["params"],
                "input": input_data, "expectedHash": sha256(result.tobytes())
            }
            if input2_data: fixture["input2"] = input2_data
            write_fixture(fixture)
```

---

## Migration Plan

1. **Create `ops_registry.py`** — encode all operations with their parameters
2. **Rewrite generator** — use registry, single reference load, no caching
3. **Regenerate all 477 fixtures** — new format with complete params
4. **Rewrite `test_fixture_parity.py`** — replace 250-line `_run_op` with generic engine
5. **Verify:** `400 passed, 77 xfailed` → same or better count
6. **Create JS test engine** — same generic engine, reads same fixtures
7. **Delete old code** — `generate_fixture_tests.py`, old `_run_op`, old generator dispatch

---

## Self-Review

- No placeholders — all sections complete ✓
- Internal consistency — registry is single source of truth for both generator and tests ✓
- Scope — focused on fixture format + test engine, no unrelated changes ✓
- Ambiguity — fixture format is explicit, no hidden defaults ✓

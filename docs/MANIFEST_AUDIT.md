# Manifest.yaml Audit — 2026-06-16

## Verification Methodology

A correct `manifest.yaml` must satisfy 5 invariants:

| # | Invariant | How to Verify |
|---|-----------|---------------|
| **I1** | Every implemented public API function is in manifest | Compare manifest names against `grep "pub fn" pillow-rs/src/image.rs` + `grep "def " pillow-rs-py/python/pillow_rs/image.py` |
| **I2** | Every manifest entry marked "implemented" actually works | Check that Python methods delegate to real Rust implementations (not stubs/no-ops) |
| **I3** | `supported_targets` matches reality | Cross-reference with `compute/registry.rs` — if GPU shader exists, `gpu` must be in targets |
| **I4** | `status` field is accurate | `implemented` = full PIL parity with tests; `stub` = placeholder only; `ignored` = intentionally skipped |
| **I5** | No Python code violates thin-client rule | `grep -E "for|while|import math|\[.*for.*in" pillow-rs-py/python/pillow_rs/*.py` must return empty |

### Automated Verification Script

```bash
#!/bin/bash
# verify_manifest.sh — run after any manifest change
echo "=== I1: Rust pub fn not in manifest ==="
python3 scripts/verify_manifest_i1.py

echo "=== I2: Manifest 'implemented' entries that are Python stubs ==="
python3 scripts/verify_manifest_i2.py

echo "=== I3: Manifest targets vs registry ==="
python3 scripts/verify_manifest_i3.py

echo "=== I5: Python thin-client violations ==="
python3 scripts/verify_thin_client.py
```

---

## Issues Found (2026-06-16)

### 🔴 SEVERE: Thin-Client Rule Violations (I5)

3 functions implemented in Python `operations.py` with loops/arithmetic/list comprehensions:

| Function | File:Line | Violation |
|----------|-----------|-----------|
| `effect_mandelbrot` | `operations.py:141` | Nested `for` loops, complex arithmetic |
| `linear_gradient` | `operations.py:108` | List comprehension generating pixel bytes |
| `radial_gradient` | `operations.py:119` | Nested `for` loops, `**0.5`, `min()`, `int()` |

**Fix:** Move to `pillow-rs/src/`, add `PipelineOp` variants, delegate from Python.

---

### 🟡 MEDIUM: Status Field Wrong (I4)

5 manifest entries say `implemented` but are Python stubs/no-ops:

| Function | Python Code | Should Be |
|----------|------------|------------|
| `Image.apply_transparency` | `pass` (no-op) | `stub` |
| `Image.get_child_images` | `return []` | `stub` |
| `Image.getexif` | Returns hardcoded minimal bytes | `stub` |
| `Image.getim` | Returns placeholder string | `stub` |
| `Image.getxmp` | `return {}` | `stub` |

---

### 🟡 MEDIUM: Missing GPU Targets (I3)

4 functions are pipeline-able but manifest says `cpu` only:

| Function | Current targets | Should Have |
|----------|----------------|-------------|
| `Image.remap_palette` | `[cpu]` | `[cpu, gpu, wasm, wasm_gpu]` |
| `ImageModule.effect_mandelbrot` | `[cpu]` | `[cpu, gpu, wasm, wasm_gpu]` |
| `ImageModule.linear_gradient` | `[cpu]` | `[cpu, gpu, wasm, wasm_gpu]` |
| `ImageModule.radial_gradient` | `[cpu]` | `[cpu, gpu, wasm, wasm_gpu]` |

---

### 🟢 LOW: Ignored/Stub With Full Targets (I3)

4 ignored/stub functions have misleading multi-backend targets:

| Function | Status | Current Targets | Should Be |
|----------|--------|----------------|-----------|
| `ImageDraw.textlength` | `ignored` | `[cpu, gpu, wasm, wasm_gpu]` | `[cpu]` |
| `ImageDraw.getfont` | `ignored` | `[cpu, gpu, wasm, wasm_gpu]` | `[cpu]` |
| `ImageOps.exif_transpose` | `ignored` | `[cpu, gpu, wasm, wasm_gpu]` | `[cpu]` |
| `ImageOps.deform` | `ignored` | `[cpu, gpu, wasm, wasm_gpu]` | `[cpu]` |

---

### 🟢 LOW: Missing From Manifest (I1)

| Function | Location |
|----------|----------|
| `Image.stat_formatted()` | `image.rs:556` — returns PIL-formatted StatResult |

Should be added to `ImageStat.Stat` or `Image` in manifest.

---

### ✅ VERIFIED CORRECT

| Check | Result |
|-------|--------|
| All public Pillow API methods covered | ✓ No false gaps (missing items are PIL internals) |
| Duplicate names across modules | ✓ Legitimate (e.g., `Image.crop` vs `ImageOps.crop`) |
| Python convenience methods | ✓ `blend`, `composite`, `eval`, `merge` on Image class correctly delegate to ImageModule |
| `Image.save` = `ignored` | ✓ Correct — save is I/O, no GPU benefit |
| `Image.close`, `Image.seek`, `Image.tell`, `Image.load` = CPU only | ✓ Correct — state management |
| `ImageFont.*` all CPU only | ✓ Correct — font rendering is CPU-bound |
| `ImagePalette.*` all CPU only | ✓ Correct — palette ops are metadata |
| ImageFilter all implemented with GPU | ✓ Correct |
| ImageEnhance all implemented with GPU | ✓ Correct |
| ImageChops all implemented with GPU | ✓ Correct |

---

## How to Verify Going Forward

### Pre-Commit Checklist

```bash
# 1. Check thin-client violations
grep -nE "^\s+(for|while) " pillow-rs-py/python/pillow_rs/*.py
grep -nE "\[.*for.*in" pillow-rs-py/python/pillow_rs/*.py

# 2. Check manifest consistency
python3 -c "
import yaml
with open('manifest.yaml') as f:
    m = yaml.safe_load(f)
for mod in m['modules']:
    for item in m['modules'][mod].get('methods', []) + m['modules'][mod].get('class_methods', []) + m['modules'][mod].get('functions', []):
        if item['status'] == 'implemented' and 'gpu' not in item.get('supported_targets', []):
            name = item['name']
            # Check if this function produces images (pipeline-able)
            if name in {'remap_palette', 'effect_mandelbrot', 'linear_gradient', 'radial_gradient'}:
                print(f'WARNING: {mod}.{name} is implemented but cpu-only — should be GPU')
"

# 3. Check for stubs marked as implemented
grep -A3 "def (apply_transparency|get_child_images|getexif|getim|getxmp)" pillow-rs-py/python/pillow_rs/image.py
```

### When Adding a New Function

1. First add manifest entry with `status: stub`
2. Implement in Rust
3. Add Python binding (thin wrapper only!)
4. Write PIL parity test
5. Update manifest: `status: implemented`, set correct `supported_targets`
6. Run the verification checklist above

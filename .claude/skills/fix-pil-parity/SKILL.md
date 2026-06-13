---
name: fix-pil-parity
description: This skill should be used when the user asks to "fix more tests", "continue fixing xfailed", "fix the algorithm", "research and fix", "make this test pass", or when working through xfailed_tracker.txt entries. Encodes the proven research→implement→validate cycle for fixing pillow-rs PIL parity test failures.
version: 0.1.0
---

# Fix PIL Parity Test

Encode a proven cycle for fixing pillow-rs PIL parity tests: research PIL's actual C/Python source code, implement the exact algorithm in Rust, validate with a single test comparison, then update the tracker and commit.

## When to Use

- The user asks to "fix more tests", "continue fixing xfailed", or "fix the algorithm"
- Working through entries in `xfailed_tracker.txt`
- Any single PIL parity test needs investigation and a Rust-level fix
- A test produces hash/value mismatches that require algorithmic correction

## Core Pattern

Three phases per test: research → implement → validate. Never combine multiple tests in one fix cycle.

### Research Phase

To find PIL's actual algorithm:

1. Search for PIL source with WebSearch: `"PIL Pillow <function_name> C source code implementation"`
2. Fetch the actual C or Python source from:
   - `https://raw.githubusercontent.com/python-pillow/Pillow/main/src/libImaging/<file>.c`
   - `https://raw.githubusercontent.com/python-pillow/Pillow/main/src/PIL/<module>.py`
3. Get the EXACT runtime values with Python — documentation is often wrong:
   ```bash
   python3 -c "from PIL import X; print(X.Y.filterargs)" 2>&1
   ```
4. Compare with the current Rust implementation at `pillow-rs-core/src/`

### Implement Phase

To implement the fix:

1. Read the current Rust code in `pillow-rs-core/src/image.rs` or `pillow-rs-core/src/ops/*.rs`
2. Edit to match PIL's algorithm exactly — same math, same rounding, same border handling
3. Build: `maturin develop --manifest-path pillow-rs-py/Cargo.toml --release`

### Validate Phase

To validate the fix:

1. Run only the single test:
   ```bash
   PYTHONPATH=/home/appunni/work/pil-wasm:$PYTHONPATH python -m pytest "tests/test_fixture_parity.py::test_fixture_parity[<test_name>]" -v
   ```
2. If XFAIL: compare RSPIL vs PIL output directly (see `references/debug-patterns.md`)
3. If fixture hash needs regeneration (PIL algorithm was correct but fixture was stale):
   ```bash
   python3 -c "
   from PIL import Image as PILImage, ImageFilter as PILF
   import json, hashlib
   for mode in ['L','RGB']:
       with open(f'tests/fixtures/<Name>_{mode}.json') as f: fx = json.load(f)
       raw = bytes.fromhex(fx['input']['bytes'])
       pil = PILImage.frombytes(mode, (100,100), raw).filter(PILF.<FILTER>)
       fx['expected']['value'] = hashlib.sha256(pil.tobytes()).hexdigest()
       with open(f'tests/fixtures/<Name>_{mode}.json', 'w') as f: json.dump(fx, f, indent=2)
   "
   ```
4. Mark test `[x]` in `xfailed_tracker.txt`
5. Commit: `git add -A && git commit -m "fix: <description>"`

## Proven Algorithm Patterns

Reference for algorithms already validated:

| Operation | PIL Source | Rust Fix | Key Detail |
|-----------|-----------|----------|------------|
| BoxBlur | `libImaging/BoxBlur.c` | Separable 2-pass, fixed-point 24-bit | `ww = 2^24/window`, `bias = 2^23` |
| Equalize | `ImageOps.py` (Python) | Step-based formula | `step = (sum_nonzero - last_bin)/255` |
| Filter3x3/5x5 | `libImaging/Filter.c` | Bottom-to-top kernel order | `offset+0.5` then `clip8` truncation |
| Blend modes | `ImageChops.c` | 256×256 LUTs from PIL runtime | Use `python3 -c` to build LUT |

## Quick Reference

| Task | Command |
|------|---------|
| Pick next test | Read `xfailed_tracker.txt`, pick `[ ]` entry |
| Research | WebSearch + WebFetch PIL source |
| Build | `maturin develop --manifest-path pillow-rs-py/Cargo.toml --release` |
| Single test | `PYTHONPATH=... python -m pytest "tests/...::test...[name]" -v` |
| Debug diffs | See `references/debug-patterns.md` |
| Regen fixture | Use inline python3 script above |
| Full suite | `PYTHONPATH=... python -m pytest tests/test_fixture_parity.py -q --tb=no` |

## Additional Resources

### Reference Files

- **`references/debug-patterns.md`** — Patterns for debugging hash/value mismatches, including how to compare RSPIL vs PIL output pixel-by-pixel
- **`references/proven-fixes.md`** — Archive of all fixes applied, with PIL source locations and Rust implementations

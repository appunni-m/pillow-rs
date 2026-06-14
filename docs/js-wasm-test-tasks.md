# JS/WASM Test Infrastructure — Task List

Goal: Port the Python JSON-driven test infrastructure to JS/WASM, reusing the same 365 fixtures.

## Architecture

```
tests/fixtures/*.json (same 365 fixtures — NO changes)
    ↓
pillow-rs-js/tests/execution_engine.mjs  →  wasm_backend.mjs  →  WASM bindings (lib.rs)
    ↓
pillow-rs-js/tests/run_wasm_test.mjs (Node.js)
pillow-rs-js/tests/browser/wasm_browser.test.mjs (Puppeteer)
```

## Tasks

### Phase 1: Core Infrastructure (parallel)

**Task 1: Create execution_engine.mjs**
- File: `pillow-rs-js/tests/execution_engine.mjs`
- Direct port of `scripts/coverage/execution_engine.py`
- 7-type dispatch: method, filter, dual, draw, enhance, classmethod, value
- Pure routing, no WASM-specific logic

**Task 2: Create wasm_backend.mjs**
- File: `pillow-rs-js/tests/wasm_backend.mjs`
- Port of `tests/rspil_backend.py` for WASM
- Implements 7 handler methods calling WASM bindings
- make_image(mode, size, bytes) — uses Image.fromBytes() or new Image() fallback
- call_method, call_filter, call_dual, call_draw, call_enhance, call_classmethod, call_value
- Handles coordinate coercion, param conversion

**Task 3: Fill WASM binding gaps in lib.rs**
- Missing Image methods: is_animated, n_frames, has_transparency_data
- Missing filters: BoxBlur, ModeFilter, RankFilter, Kernel
- Missing ImageOps: contain, cover, fit, pad, scale, crop, colorize
- Missing classmethods: effect_noise, eval
- Fix ImageStat stub (actual stats)
- Fix ImagePalette stub
- Fix ImageFont.loadDefault (load default font)
- Add proper draw methods: bitmap, text with fill color
- Add convert with dither param
- Add rotate with expand/fill params

**Task 4: Rewrite run_wasm_test.mjs (Node.js)**
- File: `pillow-rs-js/tests/run_wasm_test.mjs`
- Delete old hardcoded createImage/runOp switch
- Load fixtures from tests/fixtures/
- Create images from fixture input bytes
- Dispatch via execution engine
- Compare results: hash match (with tolerance), value match, error match
- Report pass/fail/skip counts
- Exit code 1 if failures

### Phase 2: Browser Tests

**Task 5: Create browser WASM test**
- File: `pillow-rs-js/tests/browser/wasm_browser.test.mjs`
- Puppeteer-based test
- HTML page that loads WASM module
- Runs same fixtures through browser
- Compares hashes

### Phase 3: Build & Verify

**Task 6: Build WASM**
- `wasm-pack build --target web` from pillow-rs-js/
- Verify no compile errors

**Task 7: Run Node.js tests**
- `node pillow-rs-js/tests/run_wasm_test.mjs`
- Track pass/fail/skip
- Fix any failures

**Task 8: Run browser tests**
- Puppeteer-based test run
- Track results

## Missing WASM Bindings (Complete List)

### Image instance methods missing:
- [ ] seek(frame) — exists in core
- [ ] tell() — exists in core
- [ ] load() — exists in core
- [ ] verify() — exists in core
- [ ] tobitmap() — exists in core
- [ ] is_animated property
- [ ] n_frames property
- [ ] has_transparency_data property
- [ ] convert(mode, dither, palette, colors) — exists but no params beyond mode
- [ ] rotate(angle, expand, fill) — exists but no expand/fill params

### Filter bindings missing:
- [ ] BoxBlur(radius)
- [ ] ModeFilter(size)
- [ ] RankFilter(size, rank)
- [ ] Kernel(size, kernel, scale, offset)
- [ ] Built-in filter names beyond basic ones

### ImageOps bindings missing:
- [ ] contain(image, size)
- [ ] cover(image, size)
- [ ] fit(image, size)
- [ ] pad(image, size, color)
- [ ] scale(image, factor)
- [ ] crop(image, border)
- [ ] colorize(image, black, white)

### ImageStat (needs real implementation):
- [ ] count, sum, mean, median, rms, var, stddev, extrema

### ImagePalette (needs real implementation):
- [ ] copy, getcolor, getdata, save, tobytes

### ImageFont fixes:
- [ ] loadDefault() should work, not return error

### Class methods missing:
- [ ] Image.effect_noise(size, sigma)
- [ ] Image.eval(image, function/lut)

### Draw methods:
- [ ] bitmap (draw bitmap on image)
- [ ] text with fill color

## Deletion List
- [ ] Delete old run_wasm_test.mjs (rewrite entirely)
- [ ] Delete old wasm_browser.test.mjs (rewrite entirely)

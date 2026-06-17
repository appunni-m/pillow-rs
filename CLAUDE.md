# pillow-rs Development Instructions

## Project Overview

A full-featured reimplementation of Pillow in Rust targeting **Python** (via PyO3) and **WebAssembly/JavaScript** (via wasm-bindgen). Drop-in replacement: `from RSPIL import Image` works identically to `from PIL import Image`.

## Architecture

Workspace with three crates:
- `pillow-rs-core/` — Pure Rust, all image logic, ZERO binding dependencies
- `pillow-rs-py/` — PyO3 bindings, thin wrapper (~200 lines max)
- `pillow-rs-js/` — wasm-bindgen, thin wrapper (~200 lines max)

**Iron rule:** Core never touches Python objects, JS objects, file paths, or network. Core takes Rust primitives, returns Rust primitives. All I/O and type conversion in binding crates.

### Python Binding Rules (THIN CLIENTS)
Binding files in `pillow-rs-py/python/pillow_rs/` MUST be thin wrappers:
- **NO** `for`/`while` loops
- **NO** list comprehensions `[x for x in y]`
- **NO** `import math`, `import tempfile`, `import os`, `import subprocess`
- **NO** arithmetic (`+`, `-`, `*`, `/`, `min`, `max`, `sorted`, `sum`)
- **NO** `if/elif/else` beyond isinstance checks, None defaults, or mode dispatch
- All logic lives in `pillow-rs-core/src/`; bindings delegate via `_core.xxx()` or `_rust_image.xxx()`
- Coordinate parsing, font dispatch, text layout, palette search → ALL in Rust

## Reference Code

- **Puhu** (`puhu/` in this repo) — Rust-based Pillow subset. Reference for algorithms, Pillow compatibility quirks, lazy loading pattern. NOT for architecture (Puhu is monolithic PyO3-only).
- **Pillow** — authoritative API reference. All public names must match exactly.

## Development Workflow

### Rust Code Style
- Follow `rust-development` skill guidelines
- Run `cargo clippy --all-targets --all-features -- -D warnings` before commit
- Use `thiserror` for error types, never `unwrap()` or `expect()` outside tests
- Prefer `&str` over `String`, `&[T]` over `Vec<T>` in function parameters
- Prefer single-pass tight loops; GPU dispatch via wgpu when compute-intensive

### Naming Rules
- **Public API names match Pillow exactly** — `Image.open()`, `Image.resize()`, etc.
- **Internal Rust types** can use any naming convention (e.g., `RsImage`, `PyImage`)
- The import name is `RSPIL` (mirrors `PIL`)

### Testing
- Single Python test suite in `tests/` runs against both Pillow and pillow-rs
- Tests use `@pytest.mark.covers("function_name", mode=..., variant=...)` markers
- `manifest.yaml` is the single source of truth for the API surface
- Coverage is computed by `scripts/coverage/compute_coverage.py` after each test run

### Building
- Python: `maturin develop --release` (from `pillow-rs-py/`)
- WASM: `wasm-pack build --target web` (from `pillow-rs-js/`)
- Core tests: `cargo test --manifest-path pillow-rs-core/Cargo.toml`

## Drawing Architecture — Per-Mode Native Pixel Paths

**Iron rule: The draw module MUST NOT convert images to RGBA for drawing. Each mode draws directly in its native pixel format. Zero lossy conversions.**

### Architecture

```
ImageDraw.line() on L  → draw on Luma8 canvas (1 byte/pixel)  → return Luma8
ImageDraw.line() on RGB → draw on Rgb8 canvas  (3 bytes/pixel) → return Rgb8
ImageDraw.line() on F   → write f32 LE bytes   (4 bytes/pixel) → return Rgba8 with explicit_mode="F"
ImageDraw.line() on I   → write i32 LE bytes   (4 bytes/pixel) → return Rgba8 with explicit_mode="I"
```

### Conversion Hotspots That Must Be Removed

| File | Location | Anti-pattern | Fix |
|------|----------|-------------|-----|
| `draw/mod.rs` | Every draw fn starts with `img.to_rgba8()` | Forces RGBA canvas | Draw on native-format canvas instead |
| `draw/mod.rs` | `image_clone()` lines 671-741 | Big mode→conversion match table | Remove entirely — output already in correct format |
| `imagedraw.py` | `_sync()` line 26 | `drawn.convert(self._orig_mode)` | Remove — canvas already in correct mode |
| `effects.rs` | `op_paste` lines 180,218,220 | `to_rgba8()` on dest | Work on native format |
| `image.rs` | `getpixel`/`putpixel` lines 572-955 | Mode-specific to_rgba8 then convert back | Read/write native pixels directly |

### Drawing Functions That Need Per-Mode Paths

Every draw function (`line`, `rectangle`, `ellipse`, `polygon`, `arc`, `point`, `bitmap`, `text`) must dispatch on canvas type:

```
match canvas {
    Luma8(_)  → draw with 1-byte pixel writes
    LumaA8(_) → draw with 2-byte pixel writes  
    Rgb8(_)   → draw with 3-byte pixel writes
    Rgba8(_)  → draw with 4-byte pixel writes + alpha
}
```

### Mode-Specific Color Semantics

| Mode | Bytes/px | Color format | fill=200 means |
|------|----------|-------------|----------------|
| `1`  | 1 (packed) | 0 or 255 | 255 (non-zero→white) |
| `L`  | 1 | u8 luminance | pixel value 200 |
| `LA` | 2 | u8 lum + u8 alpha | (200, 255) |
| `RGB` | 3 | u8 red, green, blue | (200, 200, 200) |
| `RGBA` | 4 | u8 r,g,b + alpha | (200, 200, 200, 255) |
| `CMYK` | 4 | u8 c,m,y,k | (200, 0, 0, 0) — C channel only |
| `P`  | 1 | palette index | index 200 (or closest) |
| `I`  | 4 | i32 LE bytes | [200,0,0,0] LE |
| `F`  | 4 | f32 LE bytes | 200.0f32 LE |

### Canvas Initialization

```rust
// Instead of:
let mut canvas = img.to_rgba8();

// Dispatch on image color type:
let mut canvas = match img {
    DynamicImage::ImageLuma8(l)  => l.clone(),
    DynamicImage::ImageLumaA8(l) => l.clone(),
    DynamicImage::ImageRgb8(r)   => r.clone(),
    DynamicImage::ImageRgba8(r)  => r.clone(),
    _ => img.to_rgba8(), // fallback for unusual types
};
```

### What Gets Deleted

After this refactor, the following code becomes dead and should be removed:
1. `draw/mod.rs`: `image_clone()` method entirely
2. `draw/mod.rs`: All `to_rgba8()` calls at the start of draw functions
3. `imagedraw.py`: `_sync()` mode conversion logic
4. `imagedraw.py`: `_orig_mode` tracking


## Manifest-Driven Development

All work starts from `manifest.yaml`. To add a new function:
1. Add its entry to `manifest.yaml` (signature, modes, variants, edge cases)
2. Run `scripts/generate_stubs.py` to create the stub in core
3. Implement the function in `pillow-rs-core/src/ops/<module>.rs`
4. Add binding delegation in `pillow-rs-py/src/lib.rs`
5. Add Python wrapper in `pillow-rs-py/python/pillow_rs/` (Image class, or new module)
6. Register new module in `pillow-rs-py/python/pillow_rs/__init__.py`
7. Update `pillow-rs-core/src/ops/mod.rs` if new module added
8. Write PIL parity tests in `tests/` using `assert_images_equal()` or `assert_values_equal()`
9. **CRITICAL**: Add a JSON fixture in `tests/fixtures/` with `operation.module` + `operation.target`. Coverage mapping is auto-discovered from fixtures and `@pytest.mark.covers` markers — no separate mapping file needed.
10. Run `python -m pytest tests/ --json-report --json-report-file=/tmp/report.json`
11. Run `python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json` to verify coverage increased

### Building (correct commands)
- Python: `maturin develop --manifest-path pillow-rs-py/Cargo.toml` (from repo root)
- WASM: `wasm-pack build --target web` (from `pillow-rs-js/`)
- Core tests: `cargo test -p pillow-rs-core`

### Full build + test (single safe command)
- **ALWAYS use this script** — it handles read-only fixtures, regeneration, and cache clearing safely:
  - Suite0: `bash scripts/build_and_test.sh`
  - Suite1: `bash scripts/build_and_test.sh 1`
  - All suites: `bash scripts/build_and_test.sh all`
- **NEVER run `rm -rf` manually** — fixtures are read-only. The script does `chmod u+w` first.
- Output fixtures are read-only after generation — edit the GENERATOR, not the fixtures.

### Test Requirements
- **Every test must validate PIL-RSPIL parity** — same inputs, same operation, compare outputs
- Use `assert_images_equal(rs_img, pil_img)` for image output comparison (pixel-exact)
- Use `assert_values_equal(rs_val, pil_val)` for non-image output comparison
- For artistic/algorithm-specific operations (filters, drawing), test that output is valid (correct size, mode, no crash)
- Tests that verify signature existence or stub behavior are NOT parity tests — they don't count toward coverage

### Coverage System

**Methodology:** Trust-based binary coverage. A function is TRUSTED if it has ≥1 PIL parity test passing. No weighted formulas.

**Files involved:**
| File | Purpose |
|------|---------|
| `scripts/coverage/coverage_map.json` | Source of truth: 221 test→function mappings |
| `scripts/coverage/compute_coverage.py` | Reads JSON + `manifest.yaml` + pytest report → trust report |
| `scripts/coverage/generate_coverage_page.py` | Full COVERAGE.md generator with benchmarks |
| `manifest.yaml` | API surface definition with status per function |
| `tests/conftest.py` | PIL parity fixtures (`assert_images_equal`, `assert_values_equal`) |

**Flow:**
```
pytest tests/ --json-report --json-report-file=/tmp/report.json
        ↓
python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json
        ↓
    TRUST REPORT: 135/135 TRUSTED, 5 stubs, 0 untracked
```

**Adding a new test:**
1. Add test function in `tests/test_<module>.py`
2. Add entry to `scripts/coverage/coverage_map.json`: `"test_name": ["Module.function"]`
3. For tests inside classes: `"ClassName::test_name": ["Module.function"]`
4. For name collisions across files: `"file_name::test_name": ["Module.function"]`
5. Run coverage to verify: `python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json`
6. Verify 0 UNTRACKED tests and function is now TRUSTED

**Coverage guarantees:**
- Every TRUSTED function has a PIL parity test that creates identical inputs, runs the same operation on both PIL and RSPIL, and asserts binary-identical output
- Zero mocked tests, zero "signature exists" tests
- Untracked tests = test exists but no coverage_map.json entry → must add entry
- `@pytest.mark.covers()` decorators on tests for self-documentation (parsed by compute_coverage.py)
### Automated Linting (`.claude/settings.json` hooks)

| Hook | When | Action |
|------|------|--------|
| `PostToolUse` | After Write/Edit to Rust files | Auto-runs `cargo fmt` |
| `Stop` | Session end | Prints clippy + fmt status |

Manual lint check: `bash scripts/lint.sh` (fmt → clippy → tests → trust report)

### Rust Code Quality Checklist (per rust-development skill)
Before committing, verify:
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] No `unwrap()` or `expect()` outside `#[cfg(test)]`
- [ ] Import order: `std` → external crates → `crate` → `super` → `self`
- [ ] `///` doc comments on all public functions with `# Examples`
- [ ] `&str` over `String`, `&[T]` over `Vec<T>` in parameters
- [ ] No redundant `.clone()` — use borrowing where possible
- [ ] `#[derive(Debug)]` on all public types
- [ ] `thiserror` for error types, never bare `anyhow` in core library


never leave commit message as anthropic or fable \

run only failing test remove show function

don't give any task back to user, do it all yourself
add to tasklist whenever you get task and don't stop until it's done
always set timeout for tests (3min)

Strictly
use internet to research exact algo

If needed wrote separate code for each mode

NEVER use git without explicit user permission. Use `git diff`, `git log`, `git show` for read-only operations. Never `git checkout`, `git revert`, `git stash`, or `git commit` without asking first.

Never change fixture output/input json images or binaries as it's generared

pillow-rs-py must not contain any IF else in anyway all of it goes to core
# Contributing to pillow-rs

A comprehensive guide to setting up, developing, testing, and publishing pillow-rs. Covers both manual workflows and Claude-assisted development.

---

## Table of Contents

1. [Project Setup](#project-setup)
2. [Architecture & Thin Client Rules](#architecture--thin-client-rules)
3. [Manifest-Driven Development](#manifest-driven-development)
4. [How to Test](#how-to-test)
5. [How to Benchmark](#how-to-benchmark)
6. [How to Check Coverage](#how-to-check-coverage)
7. [Code Quality & Linting](#code-quality--linting)
8. [Claude-Assisted Development](#claude-assisted-development)
9. [Publishing to Registries](#publishing-to-registries)
10. [License](#license)

---

## Project Setup

### What you need

| Tool | Version | Why |
|------|---------|-----|
| Rust | 1.75+ | Core language. Install via [rustup](https://rustup.rs/) |
| Python | 3.8+ | Build & test Python bindings |
| Node.js | 20+ | Build & test WASM bindings, run browser benchmarks |
| maturin | latest | Python package build (`pip install maturin`) |
| wasm-pack | latest | WASM build (`cargo install wasm-pack`) |

The Rust toolchain is pinned by `rust-toolchain.toml` — `rustup` auto-selects `stable` with `clippy`, `rustfmt`, and the `wasm32-unknown-unknown` target.

### System packages (Linux only)

None required for building. For running the full benchmark suite (TrueType font benchmarks):

```bash
sudo apt-get install -y fonts-dejavu-core   # benchmarks only — not needed to build or test
```

### Install Python dev dependencies

```bash
pip install pillow-rs[dev]
# or individually:
pip install pillow numpy pyyaml pytest pytest-timeout pytest-json-report pytest-benchmark
```

| Package | Used by | Purpose |
|---------|---------|---------|
| `pillow` | tests + benchmarks | PIL reference — every parity test compares RSPIL output against Pillow |
| `pytest` | tests | Test runner |
| `pytest-timeout` | tests | Per-test timeout (300s) |
| `pytest-json-report` | tests + coverage | JSON report consumed by coverage scripts |
| `pyyaml` | tests + coverage + benchmarks | Parses `manifest.yaml` |
| `numpy` | tests + benchmarks | Generate array inputs for `fromarray()` parity tests |
| `pytest-benchmark` | benchmarks | Benchmark fixture |

### Clone and build

```bash
git clone https://github.com/pillow-rs/pillow-rs
cd pillow-rs

# Python — builds pillow-rs + pillow-rs-py in one command
cd pillow-rs-py && maturin develop --release

# WASM — builds pillow-rs + pillow-rs-js in one command
cd pillow-rs-js && wasm-pack build --target web

# Core tests only — compiles and tests pillow-rs
cargo test -p pillow-rs
```

`maturin develop` and `wasm-pack build` both compile the entire dependency tree. Core gets built automatically — no separate `cargo build` step.

### Verify everything works

```bash
# Quick sanity check
python -c "from RSPIL import Image; print(Image.new('RGB', (10, 10)))"

# Run all tests
python -m pytest tests/ -q --timeout=300
```

---

## Architecture & Thin Client Rules

### Three-crate architecture

```
pillow-rs/     Pure Rust image library — ZERO binding dependencies
pillow-rs-py/       PyO3 bindings — thin wrapper (~200 lines, all delegation)
pillow-rs-js/       wasm-bindgen — thin wrapper (~200 lines, all delegation)
```

### The iron rule

**Core never touches:** Python objects, JS objects, file paths, network, environment variables, or any platform-specific API. All I/O and type conversion live in the binding crates.

```
✅ pillow-rs/src/ops/filter.rs:
   pub fn blur(img: &Image, radius: f32) -> Result<Image>

✅ pillow-rs-py/src/lib.rs:
   #[pyfunction]
   fn blur(img: &PyImage, radius: f32) -> PyResult<PyImage> {
       Ok(PyImage { inner: pillow_rs::ops::filter::blur(&img.inner, radius)? })
   }

✅ pillow-rs-py/python/pillow_rs/image.py:
   def filter(self, filter_type):
       return _core.filter(self._image, filter_type)  # one line, pure delegation
```

### Python binding rules (non-negotiable)

Files in `pillow-rs-py/python/pillow_rs/` must be **thin wrappers**:

| ❌ Forbidden | ✅ Allowed |
|-------------|-----------|
| `for` / `while` loops | `isinstance(obj, bytes)` |
| list comprehensions `[x for x in y]` | `if img is None: img = Image.new(...)` |
| `import math`, `os`, `tempfile`, `subprocess` | `from ._core import Image as RustImage` |
| arithmetic: `+`, `-`, `*`, `/`, `min`, `max`, `sorted`, `sum` | `return _core.method(self._image, arg)` |
| `import numpy`, `import PIL` | Duck-typing: `hasattr(obj, 'tobytes')` |
| `if/elif/else` chains with logic | `if mode is None: mode = "L"` (simple default) |

**Why this matters:** The binding layer is ~200 lines per module because every decision lives in Rust. When a bug is fixed in Rust, it's fixed for Python, Node.js, and the browser simultaneously. When logic leaks into Python, it must be re-implemented for JS.

### Drawing architecture — per-mode native paths

Drawing functions (`line`, `rectangle`, `ellipse`, `text`, etc.) dispatch on the image's **native color type** and write pixels directly in that format. There is never a lossy conversion to RGBA as an intermediate step.

```rust
// NOT this:
let canvas = img.to_rgba8();  // ❌ destroys L, F, I, CMYK fidelity

// THIS:
match &mut canvas {
    Luma8(ref mut buf)  => draw_into_luma(buf, ...),
    Rgb8(ref mut buf)   => draw_into_rgb(buf, ...),
    Rgba8(ref mut buf)  => draw_into_rgba(buf, ...),
    // ...
}
```

Each mode writes pixels in its native byte layout — 1 byte for L, 3 for RGB, 4 for RGBA, 4-byte f32 LE for F, 4-byte i32 LE for I.

---

## Manifest-Driven Development

`manifest.yaml` is the **single source of truth** for the entire project. Every function, its signature, supported modes, parameter variants, and edge cases is defined here.

### What manifest drives

```
manifest.yaml
    │
    ├──→ scripts/generate_stubs.py        → Rust stubs in pillow-rs/src/ops/
    ├──→ scripts/generate_fixtures.py     → Test fixtures (inputs + expected outputs)
    ├──→ scripts/bench/bench_spec.py      → Benchmark specification (166 functions)
    ├──→ scripts/coverage/compute_coverage.py → Trust verification per function
    ├──→ tests/test_parity.py             → Pytest parametrization (1,555 tests)
    └──→ docs/COVERAGE.md                 → Auto-generated coverage report
```

### Adding a new function (step by step)

1. **Define** — Add the function entry to `manifest.yaml`:

   ```yaml
   - name: my_function
     signature: 'my_function(img: Image, param: int) -> Image'
     supported_modes: [L, RGB, RGBA]
     param_variants:
       - param: 1
       - param: 5
     edge_cases:
       - param: 0
     status: implemented
     pillow_since: '1.0'
     supported_targets: [cpu]
   ```

2. **Generate stubs** — `python scripts/generate_stubs.py` scaffolds Rust function signatures in `pillow-rs/src/ops/`

3. **Implement** — Fill in the Rust implementation in the generated stub file

4. **Add binding** — Add `#[pyfunction]` delegation in `pillow-rs-py/src/lib.rs`:
   ```rust
   #[pyfunction]
   fn my_function(img: &PyImage, param: i32) -> PyResult<PyImage> {
       Ok(PyImage { inner: pillow_rs::ops::my_module::my_function(&img.inner, param)? })
   }
   ```

5. **Add Python wrapper** — Add one-line method in `pillow-rs-py/python/pillow_rs/image.py`:
   ```python
   def my_function(self, param):
       return _core.my_function(self._image, param)
   ```

6. **Register module** — If new ops file: add `pub mod my_module;` to `pillow-rs/src/ops/mod.rs`

7. **Generate fixtures** — `python scripts/generate_fixtures.py` creates test fixtures from manifest

8. **Map coverage** — Add entry to `scripts/coverage/coverage_map.json`:
   ```json
   "test_parity[Image.my_function__Image_my_function_L_variant1]": ["Image.my_function"]
   ```

9. **Run tests and validate coverage**:
   ```bash
   python -m pytest tests/ --json-report --json-report-file=/tmp/report.json
   python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json
   ```

### Fixing a failing test (incremental edit)

1. Find the failing test in `xfailed_tracker.txt` or pytest output
2. Run it in isolation:
   ```bash
   python -m pytest tests/ -k "test_name_here" -v --tb=long --timeout=300
   ```
3. Compare RSPIL output vs Pillow output to understand the mismatch
4. Fix the Rust implementation in `pillow-rs/src/ops/<module>.rs`
5. Re-run the single test to confirm the fix
6. Run the full suite to check for regressions:
   ```bash
   python -m pytest tests/ -q --timeout=300
   ```
7. Update `xfailed_tracker.txt` — mark as `[x]` when fixed
8. Commit with message: `fix: Image.my_function passes PIL parity for mode X`

---

## How to Test

### Full suite

```bash
# Build + generate fixtures + run tests (suite0 only, fast path)
bash scripts/build_and_test.sh

# With extended suite
bash scripts/build_and_test.sh 1   # suite0 + suite1
bash scripts/build_and_test.sh 2   # suite0 + suite2
```

### Direct pytest

```bash
# All 1,555 tests (~60s)
python -m pytest tests/ -q --tb=line --timeout=300

# Single test
python -m pytest tests/ -k "ImageFilter_BLUR" -v --tb=long --timeout=300

# Specific module
python -m pytest tests/ -k "ImageChops" -q --timeout=300

# With JSON report (required for coverage)
python -m pytest tests/ --json-report --json-report-file=/tmp/report.json -q --timeout=300

# Run only suite0 (fast, core functions)
python -m pytest tests/ -k "not suite1 and not suite2 and not suite3" -q --timeout=300
```

### Rust core tests

```bash
cargo test -p pillow-rs
cargo test -p pillow-rs -- --nocapture   # show output
```

### Test architecture

- **`tests/test_parity.py`** — Single parametrized test file. Fixture data drives 1,555 test cases.
- **`tests/engine.py`** — Execution engine: creates PIL and RSPIL inputs, runs both, compares results.
- **`tests/conftest.py`** — Pytest config: manifest loading, backend selection, fixture helpers.
- **`tests/fixtures/`** — JSON fixtures (not committed). Regenerated by `scripts/generate_fixtures.py`.

Each test:
1. Creates identical inputs for both PIL (reference) and RSPIL
2. Runs the same operation on both
3. Asserts binary-identical output with `assert_images_equal()` or `assert_values_equal()`

---

## How to Check Coverage

```bash
# 1. Run tests with JSON report
python -m pytest tests/ --json-report --json-report-file=/tmp/report.json -q --timeout=300

# 2. Compute coverage
python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json

# 3. Generate coverage docs
python scripts/coverage/generate_multi_backend_coverage.py  # → docs/COVERAGE.md
```

Coverage mapping lives in `scripts/coverage/coverage_map.json`. Every test must have an entry mapping it to its manifest function(s). When you add a test, add it here.

CI gate (`scripts/ci_coverage.sh`) runs the full pipeline and exits 1 on any coverage gap.

---

## How to Benchmark

```bash
# Full suite (166 functions, 6 targets)
bash scripts/bench/bench_all.sh full

# Incremental (only changed code since last run — SHA-256 cache)
bash scripts/bench/bench_all.sh incremental

# Priority tier (12 most-used ops)
bash scripts/bench/bench_all.sh --group priority

# Specific functions
bash scripts/bench/bench_all.sh --only resize,crop
```

Output regenerates `BENCHMARKS.md`. The benchmark spec is auto-generated from `manifest.yaml` — no manual registration.

### Benchmark targets

| Target | Runner | Description |
|--------|--------|-------------|
| `native_cpu` | Python + pillow_rs | Native Rust vs Pillow CPU |
| `wasm_cpu` | Node.js + WASM | WASM in Node.js runtime |
| `browser_cpu` | Puppeteer + headless Chrome | WASM in browser |
| `native_gpu` | wgpu compute | Native GPU (NYW — not yet wired) |
| `wasm_gpu` | Node.js + WebGPU | WASM GPU (NYW) |
| `browser_gpu` | Puppeteer + WebGPU | Browser GPU (NYW) |

---

## Code Quality & Linting

### Rust

```bash
# Format
cargo fmt

# Lint (CI gate — must pass with zero warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Check a specific crate
cargo clippy -p pillow-rs --all-targets -- -D warnings
```

### Pre-commit checklist

- [ ] `cargo fmt` passes (no diff)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] No `unwrap()` or `expect()` outside `#[cfg(test)]`
- [ ] `thiserror` for error types, never bare `anyhow` in core
- [ ] `&str` over `String`, `&[T]` over `Vec<T>` in function parameters
- [ ] `#[derive(Debug)]` on all public types
- [ ] `///` doc comments with `# Examples` on all `pub` functions
- [ ] Import order: `std` → external crates → `crate` → `super` → `self`
- [ ] No redundant `.clone()` — prefer borrowing
- [ ] Drawing code dispatches per-mode, no `to_rgba8()` conversions
- [ ] Python bindings contain no loops, no arithmetic, no math imports

### Full lint script

```bash
bash scripts/lint.sh   # runs fmt → clippy → tests → coverage
```

### Naming conventions

- **Public API names match Pillow exactly** — `Image.open()`, `Image.resize()`, `Image.filter()`
- **Import name** is `RSPIL` (mirrors `PIL`)
- **Internal Rust types** use any naming convention — `RsImage`, `PyImage`, `RsDraw`

---

## Claude-Assisted Development

pillow-rs has project-specific Claude Code skills and agents to accelerate development.

### Skills

#### `fix-pil-parity`

Research → implement → validate cycle for fixing PIL parity test failures.

**When to use:** "fix more tests", "continue fixing xfailed", "make this test pass"

**How to invoke:** `/fix-pil-parity` or ask Claude to fix a specific failing test

**What it does:**
1. Reads `xfailed_tracker.txt` to find the next failing test
2. Researches Pillow's actual C/Python source code for the exact algorithm
3. Implements the algorithm in Rust in `pillow-rs/src/ops/`
4. Validates with the single failing test
5. Updates `xfailed_tracker.txt`

**Best for:** New contributors — it encodes the proven fix cycle end-to-end.

#### `compute-backend`

GPU/SIMD/WebGPU compute backend development.

**When to use:** "add GPU support for ops", "implement pool_simd", "migrate shaders", "make shaders mode-aware"

**What it does:**
- Guides shader creation in `pillow-rs/src/compute/gpu_shaders/`
- Registers ops in `compute/registry.rs` with cpu_fn + gpu_shader entries
- Ensures CPU fallback for every GPU op

#### `rust-development`

General Rust code quality. Use for clippy fixes, borrow checker issues, API design, and idiomatic Rust patterns.

**When to use:** "fix clippy warnings", "refactor this", "make this idiomatic"

### Agents

#### `pil-parity-fixer`

Autonomous agent that picks failing tests from `xfailed_tracker.txt` and works through the full fix cycle without supervision.

**When to use:** "fix xfailed tests", "continue fixing tests", "make tests pass"

**How to invoke:** Ask Claude to launch the pil-parity-fixer agent

### Onboarding with skills

New contributors should:

1. **Set up the project** (see [Project Setup](#project-setup))
2. **Pick a failing test** from `xfailed_tracker.txt`
3. **Invoke the fix-pil-parity skill:** `/fix-pil-parity` and specify the test name
4. **Let Claude handle the research → implement → validate cycle**
5. **Review the changes** — Claude shows exactly what changed and why
6. **Run the full suite** to check for regressions

The skills encode project-specific knowledge (manifest structure, binding rules, drawing architecture, test patterns) so new contributors don't need to learn everything upfront.

---

## Publishing to Registries

pillow-rs targets three registries. All share the same version from `Cargo.toml` workspace.

### PyPI (Python)

```bash
cd pillow-rs-py
maturin build --release        # builds .whl
maturin publish                # uploads to PyPI
```

Configuration in `pillow-rs-py/pyproject.toml`:
- `name = "pillow-rs"`
- `requires-python = ">=3.8"`
- `license = { text = "MIT-CMU" }`
- Classifiers include all supported Python versions

### npm (JavaScript / WASM)

```bash
cd pillow-rs-js
wasm-pack build --target web --release
cd pkg
npm publish
```

Configuration in `pillow-rs-js/package.json`:
- `name = "@pillow-rs/wasm"`
- `files = ["pkg/"]` (only the built WASM bundle)

### crates.io (Rust)

```bash
cargo publish -p pillow-rs
```

Configuration in `pillow-rs/Cargo.toml`:
- `name = "pillow-rs"`
- `license = "MIT-CMU"` (from workspace)

### Release checklist

- [ ] Bump version in workspace `Cargo.toml`
- [ ] Update changelog
- [ ] Run full test suite: `python -m pytest tests/ -q --timeout=300`
- [ ] Run clippy: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Regenerate benchmarks: `bash scripts/bench/bench_all.sh incremental`
- [ ] Regenerate coverage: `python scripts/coverage/generate_multi_backend_coverage.py`
- [ ] Build and test WASM: `wasm-pack build --target web && node scripts/bench/bench_wasm_cpu.mjs`
- [ ] Publish to all three registries

---

## License

By contributing, you agree that your contributions will be licensed under the same [MIT-CMU License](LICENSE) as the project.

Copyright © 2024-2026 by Appunni M and contributors.

---

## Questions?

Open a [GitHub Discussion](https://github.com/pillow-rs/pillow-rs/discussions) or [Issue](https://github.com/pillow-rs/pillow-rs/issues).

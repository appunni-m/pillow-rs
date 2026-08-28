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
pip install pillow==12.2.0 numpy pyyaml coverage
```

| Package | Used by | Purpose |
|---------|---------|---------|
| `pillow` | parity + coverage + benchmarks | PIL reference — every parity case compares RSPIL output against Pillow |
| `coverage` | coverage | Python source coverage collector |
| `pyyaml` | parity + coverage + benchmarks | Parses `manifest.yaml` |
| `numpy` | parity + benchmarks | Generate array inputs for `fromarray()` parity cases |

### Clone and build

```bash
git clone https://github.com/pillow-rs/pillow-rs
cd pillow-rs

# Python — builds pillow-rs + pillow-rs-py in one command
cd pillow-rs-py && maturin develop --release

# WASM — builds pillow-rs + pillow-rs-js in one command
cd pillow-rs-js && wasm-pack build --target web

# Build the pure-Rust core
make pillow-rs-build
```

`maturin develop` and `wasm-pack build` both compile the entire dependency tree. Core gets built automatically — no separate `cargo build` step.

### Verify everything works

```bash
# Quick sanity check
python -c "from RSPIL import Image; print(Image.new('RGB', (10, 10)))"

# Run the public parity corpus through every available facade/backend
make migration-parity-test-all-backends
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
    ├──→ scripts/build_migration_parity_inputs.py → public input corpus
    ├──→ scripts/bench/bench_spec.py             → benchmark specification
    ├──→ scripts/run_migration_parity.py         → live Pillow comparisons
    ├──→ scripts/run_migration_*coverage.py      → managed coverage evidence
    └──→ docs/COVERAGE.md                        → generated coverage report
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

7. **Generate public inputs** — `make migration-parity-inputs` regenerates the manifest-driven input corpus

8. **Map coverage** — Add the corresponding input-only coverage plan and case
   selector to `pillow-rs/tests/fixtures/inputs/coverage/`.

9. **Run parity and validate coverage**:
   ```bash
   make migration-parity-test-all-backends
   make migration-parity-coverage
   ```

### Fixing a failing parity case (incremental edit)

1. Find the failing `case_id` in the parity result or Coverage MCP output
2. Run it in isolation:
   ```bash
   make migration-parity-case MIGRATION_PARITY_CASE="PIL.Image.Image.operation.case"
   ```
3. Compare RSPIL output vs Pillow output to understand the mismatch
4. Fix the Rust implementation in `pillow-rs/src/ops/<module>.rs`
5. Re-run the single parity case to confirm the fix
6. Run the full parity corpus to check for regressions:
   ```bash
   make migration-parity-test-all-backends
   ```
7. Commit with message: `fix: Image.my_function passes PIL parity for mode X`

---

## How to Test

### Full parity campaign

```bash
# Regenerate and validate the manifest-driven public input corpus
make migration-parity-inputs
make migration-parity-inputs-check

# Run the shared public corpus through CPU, SIMD, GPU/WGSL, Python, and JS/WASM,
# then measure the same workflows against Pillow and order the reverse gaps
make test

# The target-only combined lane
make migration-parity-test-all-backends
```

### Incremental parity

```bash
# One public case, without replacing the full-suite artifact
make migration-parity-case MIGRATION_PARITY_CASE="PIL.Image.Image.copy.behavior.default"

# A comma-separated filtered parity run
make migration-parity-test MIGRATION_PARITY_CASE_IDS="case-a,case-b"

# One filter for every lane in `make test`, including reverse Pillow coverage
make test MIGRATION_TEST_CASE_IDS="case-a,case-b"

# The same filter for the target-only all-backend runner
make migration-parity-test-all-backends MIGRATION_ALL_BACKENDS_CASE_IDS="case-a,case-b"

# Coverage uses the indexed plans, not a test-runner JSON report.
make migration-parity-coverage

# Order Pillow source and public-operation gaps from the latest reverse run.
make migration-parity-pillow-missing-manifest
```

### Parity architecture

- **`pillow-rs/tests/fixtures/manifest.yaml`** — Fixed public surface and coverage plans.
- **`pillow-rs/tests/fixtures/inputs/parity/`** — Input-only public workflows; expected outputs are produced by the live Pillow oracle.
- **`scripts/run_migration_parity.py`** — Executes the same workflow against Pillow and the Python binding.
- **`scripts/run_all_backend_tests.py`** — Runs CPU, SIMD, bounded GPU, Python, Node WASM, and browser WASM lanes with one public corpus.
- **`scripts/run_migration_js_parity.py`** — Sends JS-compatible cases to either the Node or browser WASM adapter and compares them with Pillow.
- **`scripts/run_migration_pillow_coverage.py`** — Runs that public corpus against Pillow while collecting Python source coverage.
- **`scripts/report_migration_pillow_missing.py`** — Produces the ordered source and public-feature gap manifest; it is evidence, not a new denominator.

Each parity case creates the same input workflow for Pillow and the selected
target, then compares the canonical output or error. A case is selected by its
stable `case_id`, so the same filter can be reused for local, managed, and
Coverage MCP runs.

---

## How to Check Coverage

```bash
# Rust + Python coverage through the indexed public plans
make migration-parity-coverage

# Pillow source coverage using the exact same public corpus
make migration-parity-pillow-coverage

# Target Rust + Python merged coverage for a filtered case set
make migration-parity-coverage-rust MIGRATION_COVERAGE_CASE_IDS="case-a,case-b"
```

Coverage mapping lives in the manifest and indexed coverage inputs. Every
public input case must have a manifest operation and coverage-plan selector.
Coverage MCP can compare a filtered run with an explicit snapshot baseline;
the primary incremental number is expected to be the deduplicated union of the
baseline and the selected increment.
An observed Coverage MCP 0.15.0 defect currently violates that invariant for
some filtered LLVM runs; keep the exact reproduction in
[`docs/coverage-mcp-incremental-union-bug-20260827.md`](docs/coverage-mcp-incremental-union-bug-20260827.md)
until the dashboard fixes its primary projection.

The CI gate runs the maintained Make targets and rejects invalid or incomplete
parity/coverage result interfaces.

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
| `browser_cpu` | Puppeteer + headless Chrome | WASM in browser; same public parity corpus as Node |
| `native_gpu` | wgpu compute | Native GPU (NYW — not yet wired) |
| `wasm_gpu` | Node.js + WebGPU | WASM GPU (NYW) |
| `browser_gpu` | Puppeteer + WebGPU | Browser GPU (NYW) |

---

## Code Quality & Linting

pillow-rs uses a curated, multi-layered linting setup that catches bugs, enforces idiomatic Rust, and keeps the codebase consistent — without burying contributors in noise.

### How it works

Instead of the blunt `-D warnings` (which turns every deprecation notice into a hard error), we use **workspace-level lint configuration** in `Cargo.toml`. Each lint is assigned a deliberate level:

| Level | Meaning | CI fails? |
|-------|---------|-----------|
| `deny` | Hard rule — always an error | **Yes** |
| `warn` | Shows in `cargo clippy` output, but doesn't block builds | No |
| `allow` | Silenced intentionally (with a `TODO` tracking migration) | No |

This means adding a new dependency with a deprecation doesn't break CI. Adding an `unwrap()` in production code **does**.

### What gets enforced

The full configuration lives in `[workspace.lints]` at the workspace root. Here's what each category covers:

#### Rust compiler lints (`[workspace.lints.rust]`)

| Lint | Level | Why |
|------|-------|-----|
| `future-incompatible` | deny | Catch breakage before the next Rust edition |
| `nonstandard_style` | deny | Snake case, CamelCase — consistent naming everywhere |
| `unused` | deny | Dead imports, dead variables — zero dead code |
| `unsafe_code` | warn | Visible but not forbidden — needs `// SAFETY:` comment |
| `deprecated` | allow | PyO3 `to_object` → `IntoPyObject` migration tracked in TODO |

#### Clippy lints (`[workspace.lints.clippy]`)

**Performance (all `deny`):**
`redundant_clone`, `large_enum_variant`, `needless_collect`, `clone_on_copy`, `unnecessary_to_owned`

**Safety (all `deny`):**
`unwrap_used`, `expect_used`, `todo`

**Style & Idiom (all `deny`):**
`needless_borrow`, `map_unwrap_or`, `needless_range_loop`, `unnecessary_cast`

These were chosen intentionally — not "all clippy lints," but the subset that has caught real bugs in this codebase. No lint is added without a demonstrated reason.

### Running lint checks

```bash
# Format (auto-fixes)
cargo fmt

# Format check (CI gate)
cargo fmt --check

# Lint — workspace config handles levels; -A deprecated silences PyO3 migration noise
cargo clippy --all-targets --all-features -- -A deprecated

# Lint a single crate
cargo clippy -p pillow-rs --all-targets -- -A deprecated

# Full lint script (fmt → clippy → tests → coverage)
bash scripts/lint.sh
```

**Why `-A deprecated`?** PyO3 v0.23+ deprecated `ToPyObject::to_object` in favor of `IntoPyObject`. The migration touches 80+ call sites in `pillow-rs-py`. Until that migration is done, `-A deprecated` keeps the deprecation warnings from masking real issues. A `TODO(#ci)` in the workspace config tracks this.

### rustfmt configuration

`rustfmt.toml` at the workspace root sets:

| Setting | Value | Why |
|---------|-------|-----|
| `reorder_imports` | `true` | Consistent import ordering without nightly |
| `max_width` | `100` | Fits side-by-side diffs on most screens |
| `edition` | `2021` | Matches workspace edition |
| `tab_spaces` | `4` | Standard Rust indent |

Nightly-only options (`imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`) are noted in comments — enable once stable.

### Pre-commit checklist

- [ ] `cargo fmt --check` passes (no diff)
- [ ] `cargo clippy --all-targets --all-features -- -A deprecated` passes
- [ ] `make migration-parity-test-all-backends` passes
- [ ] No `unwrap()` or `expect()` in runtime code without a documented invariant
- [ ] `thiserror` for error types, never bare `anyhow` in core
- [ ] `&str` over `String`, `&[T]` over `Vec<T>` in function parameters
- [ ] `#[derive(Debug)]` on all public types
- [ ] `///` doc comments with `# Examples` on all `pub` functions
- [ ] Import order: `std` → external crates → `crate` → `super` → `self`
- [ ] No redundant `.clone()` — prefer borrowing
- [ ] Drawing code dispatches per-mode, no `to_rgba8()` conversions
- [ ] Python bindings contain no loops, no arithmetic, no math imports

### Adding or adjusting a lint

1. Add the lint to `[workspace.lints.rust]` or `[workspace.lints.clippy]` in `Cargo.toml`
2. Set the level: `deny` for hard rules, `warn` for advisories
3. If a lint group is at `deny` and you need a specific member at a different level, give the group `priority = -1` so the member can override
4. Run `cargo clippy --all-targets --all-features -- -A deprecated` to see the effect
5. Fix or `#[expect(clippy::lint_name, reason = "...")]` with a documented reason — never bare `#[allow]`
6. Document the rationale in this section

### Suppressing a lint (when you must)

Use `#[expect]`, NOT `#[allow]`. `expect` fires a warning if the lint stops triggering — preventing stale suppressions:

```rust
// ✅ Correct — documents why, self-cleaning
#[expect(clippy::bad_bit_mask, reason = "Verify extracted byte components are in valid u8 range")]
fn test_bit_extraction() { ... }

// ❌ Wrong — silent, rots forever
#[allow(clippy::bad_bit_mask)]
fn test_bit_extraction() { ... }
```

This follows the rust-development skill principle: **fix warnings, don't silence them**. Suppression is a last resort, not a first response.

### Editor integration

Most editors pick up `rustfmt.toml` and workspace lint configs automatically:

- **VS Code** with `rust-analyzer`: format-on-save uses `rustfmt.toml`; clippy warnings appear inline
- **JetBrains Rust** / **IntelliJ**: detects `Cargo.toml` workspace lints; Ctrl+Alt+L formats
- **vim/neovim** with `rust-analyzer` LSP: `:RustFmt` and inline diagnostics

No IDE-specific config files needed — everything lives in the workspace root.

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
- [ ] Run full parity corpus: `make migration-parity-test-all-backends`
- [ ] Run clippy: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Regenerate benchmarks: `bash scripts/bench/bench_all.sh incremental`
- [ ] Regenerate coverage: `python scripts/coverage/generate_multi_backend_coverage.py`
- [ ] Build and run JS/WASM parity: `make test-wasm`
- [ ] Publish to all three registries

---

## License

By contributing, you agree that your contributions will be licensed under the same [MIT-CMU License](LICENSE) as the project.

Copyright © 2024-2026 by Appunni M and contributors.

---

## Questions?

Open a [GitHub Discussion](https://github.com/pillow-rs/pillow-rs/discussions) or [Issue](https://github.com/pillow-rs/pillow-rs/issues).

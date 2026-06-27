# pillow-rs Development Instructions

## Architecture

Workspace with three crates:
- `pillow-rs/` — Pure Rust, all image logic, ZERO binding dependencies
- `pillow-rs-py/` — PyO3 bindings, thin wrapper (~200 lines max)
- `pillow-rs-js/` — wasm-bindgen, thin wrapper (~200 lines max)

**Iron rule:** Core never touches Python objects, JS objects, file paths, or network. Core takes Rust primitives, returns Rust primitives. All I/O and type conversion lives in binding crates.

## Python Binding Rules

`pillow-rs-py/python/pillow_rs/` MUST be thin wrappers:
- **NO** `for`/`while` loops, list comprehensions, `import math/os/subprocess/tempfile`
- **NO** arithmetic (`+`, `-`, `*`, `/`, `min`, `max`, `sorted`, `sum`)
- **NO** `if/elif/else` beyond isinstance checks, None defaults, or mode dispatch
- All logic in `pillow-rs/src/`; bindings delegate via `_core.xxx()` or `_rust_image.xxx()`

## Drawing Architecture

**Iron rule: Draw directly in the image's native pixel format. NEVER convert to RGBA for drawing.**

Every draw function dispatches on canvas type:
```
Luma8 (1 byte/px) | LumaA8 (2 bytes/px) | Rgb8 (3 bytes/px) | Rgba8 (4 bytes/px)
```

Mode-specific color: see `pillow-rs/src/draw/mod.rs` dispatch table for per-mode `fill=X` semantics.

## Logging

Use `log` crate macros. NEVER `eprintln!` or `println!` in library code.

| Level | When | Example |
|-------|------|---------|
| `log::error!` | Failures, corrupt data | `log::error!("JPEG: invalid Huffman table at offset {}", off);` |
| `log::warn!` | Recoverable issues, fallbacks | `log::warn!("Unknown EXIF tag, skipping");` |
| `log::info!` | High-level operations | `log::info!("Opening {}×{} {} image", w, h, mode);` |
| `log::debug!` | Algorithm steps, backend selection | `log::debug!("[GPU] {} op(s) {}×{}", ops.len(), w, h);` |
| `log::trace!` | Internal per-scan/pixel detail | `log::trace!("progressive: S[{}] ss={}", idx, ss);` |

**Rules:**
- Prefix messages with context: `"progressive:"`, `"[GPU]"`, `"[SIMD]"`, module name
- Core crates NEVER initialize a logger — bindings do that (`pyo3-log`, `console_log`)
- Test files can use `eprintln!` for progress output
- New core crates must add `log = "0.4"` to `Cargo.toml`

## Rust Code Style

Delegate to `rust-development` skill. Key repo specifics:
- `thiserror` for errors, never `unwrap()`/`expect()` outside tests
- `&str` over `String`, `&[T]` over `Vec<T>` in parameters
- `cargo clippy --all-targets --all-features -- -D warnings` before commit

## Manifest-Driven Development

All work starts from `manifest.yaml` — the single source of truth for the API surface.

**Adding a function:**
1. Add entry to `manifest.yaml` (signature, modes, variants)
2. `scripts/generate_stubs.py` → creates stub in core
3. Implement in `pillow-rs/src/ops/<module>.rs`
4. Add binding delegation in `pillow-rs-py/src/lib.rs`
5. Add Python wrapper in `pillow-rs-py/python/pillow_rs/`
6. Register in `__init__.py` + `ops/mod.rs` if new module
7. Write PIL parity test using `assert_images_equal()` / `assert_values_equal()`
8. Add JSON fixture in `tests/fixtures/` with `operation.module` + `operation.target`
9. Run tests + `scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json`

## Building & Testing

```bash
# Python
maturin develop --manifest-path pillow-rs-py/Cargo.toml

# WASM
wasm-pack build --target web  # from pillow-rs-js/

# Core tests
cargo test -p pillow-rs

# Full build + test (always use this — handles fixtures safely)
bash scripts/build_and_test.sh        # Suite0
bash scripts/build_and_test.sh 1      # Suite1
bash scripts/build_and_test.sh all    # All suites
bash scripts/lint.sh                  # fmt → clippy → tests → trust report
```

**NEVER `rm -rf` manually** — fixtures are read-only. Use the scripts.
**NEVER edit fixture output files** — edit the generator instead.

## Coverage

Trust-based binary: function is TRUSTED if ≥1 PIL parity test passes.
- Map: `scripts/coverage/coverage_map.json` (`"test_name": ["Module.function"]`)
- Report: `python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json`
- Tests that only verify signatures/stubs don't count — must be PIL parity

## Debugging Protocol for Porting Crates (especially `pillow-rs-freetype`)

When porting a C library (FreeType, etc.) to Rust, follow this protocol.
Mistakes here waste hours. Each step builds on the previous one — skip none.

### 1. Establish a single source of truth for references

Every test fixture (JSON, binary, SHA) MUST be generated from the EXACT
external reference implementation, version-matched. Document:

- **What** generated it (PIL? raw C library? our own code?)
- **Which version** (PIL 12.2.0 bundles FreeType 2.14.3 — use THAT version)
- **How to regenerate** (one script, reproducible from a clean checkout)

If references are regenerated from the code under test, tests are meaningless.
Self-referential fixtures pass 100% and prove nothing.

### 2. Version-lock ALL reference generators

Before writing a single line of comparison code, verify every reference source
uses the same upstream version:

```
Reference matrix generator → FreeType 2.14.3?
PIL (if used for refs)     → bundles FreeType 2.14.3?
System C library           → FreeType 2.14.3?  (often 2.13.x)
Vendored C source          → FreeType 2.14.3?
Your Rust port baseline    → FreeType 2.14.x?
```

A 1-patch-version mismatch (2.13.2 vs 2.14.3) can flip 850 tests from pass
 to fail because the autohinter changed. **Always check this first.**

### 3. Compare C-vs-Rust at the boundary, not the output

When pixels differ, do NOT iterate on `getmask()`/`getbbox()` hoping to
stumble on the bug. Instead:

1. Pick ONE failing glyph (start simple: `A`, `|`, `-`).
2. Dump its 26.6 outline coordinates from BOTH C and Rust **before** any
   autohinting. They must match. If not — scaler bug. Fix scaler first.
3. Dump edge positions (`fpos`, `opos`, `pos`) from BOTH after hinting.
4. Dump final hinted 26.6 point coordinates from BOTH.
5. Find the FIRST point that diverges. Everything downstream of that point
   is a consequence, not a cause.

**Binary search the divergence.** If point N is the first mismatch, the bug
is in whatever function touched point N: `align_strong_points`,
`align_weak_points`, `align_edge_points`, etc.

### 4. Build the C reference WITH instrumentation

Do not try to access FreeType internals via Python ctypes — the struct
offsets are fragile. Instead:

```bash
# Build FreeType from vendored source with debug tracing
cd pillow-rs-freetype/freetype && mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Debug \
  -DFT_DEBUG_LEVEL_TRACE=ON \
  -DCMAKE_INSTALL_PREFIX=$HOME/.local
```

Then use `FT_Set_Debug_Hook` or environment variable `FT2_DEBUG` to enable
per-module trace output. Compare C's `af_latin_hints_align_edge_points`
trace with our Rust equivalent.

### 5. Fix root cause, not symptoms

Common anti-patterns to avoid:

- **Clamping outputs to match expected values.** If `getbbox` returns (0,7,4,8)
  and PIL expects (0,7,4,10), the fix is NOT `gy_max.max(10)`. The fix is
  finding why `bbox_y_min` differs by 2px.

- **Enabling code paths that amplify bugs.** Phantom-point advance
  adjustment needs correct edge positions first. Enabling it with wrong edges
  made PIL score DROP from 1170 to 1140.

- **Adding special cases per glyph.** Each special case is a future bug.
  Find the algorithmic root cause and fix it once.

- **Renaming files/directories mid-debug.** `fonts_nohint` → `fonts_autohint`
  is fine ONCE, but churn obscures real diffs. Settle names early.

### 6. Categorize failures before fixing

Before touching code, classify every failure:

```
SHA-only (bbox correct):     subpixel coverage difference
Bbox wrong:                  edge position difference
Size wrong:                  advance or metrics difference
Empty/zero output:           pipeline broken entirely
```

Each category has a different root cause. Mixing them wastes time.

### 7. Commit working state before experimenting

Always commit (with permission) before a risky change. If the experiment
fails, `git checkout -- <file>` is one keystroke. Without the commit, you
lose the working baseline.

### 8. Document the divergence, not just the fix

When you find a bug, write in the commit message:
- What C produces (exact value)
- What Rust produces (exact value)
- Which C function/line the Rust code diverges from
- Why it diverges (off-by-one? wrong sign? missing case?)

This lets the next person verify the fix without re-deriving the diagnosis.

### 9. Never claim "algorithmically complete" without pixel parity

A diff that shows "only overflow macros changed" between C versions does NOT
mean the port is algorithmically correct. The port can have bugs in any
function. Pixel parity (byte-identical SHA-256) is the only proof.

### 10. Use C trace output as the oracle

When stuck, add `eprintln!` to the Rust function and compare with C's
`FT_TRACE` output. Build C with `-DFT_DEBUG_LEVEL_TRACE` and set
`FT2_DEBUG="any:7"` to get maximum verbosity. The C trace shows exactly
what each function computes — match it line by line.

### 11. Annotate source code with C-verification status

**Every function ported from FreeType MUST carry verification annotations**
so work is never repeated. Use these markers in doc comments:

| Marker | Meaning |
|--------|---------|
| `✅ VERIFIED: ...` | Confirmed byte-for-byte or algorithmically correct vs C reference (include C file + line range) |
| `⚠️ BUG: ...` | Known divergence from C — include what C does, what Rust does, and what to fix |
| `⚠️ UNVERIFIED: ...` | Not yet compared against C — may be correct or buggy, needs tracing |
| `⚠️ SIMPLIFIED: ...` | Intentional simplification — document what C does that we skip and why |
| `⚠️ DEAD CODE: ...` | Code path that never executes due to upstream bugs — document what needs fixing upstream |
| `⚠️ DEPENDENCY: ...` | Correctness depends on fixing upstream bugs — document the dependency chain |

**Every annotation must include C reference info**: function name, file, and
line range (e.g., `aflatin.c:3991-4075`).

**When to annotate:**
- After tracing: if the function produces identical output to C → `✅ VERIFIED`
- After finding a bug: describe it with `⚠️ BUG:` so the next person doesn't re-diagnose
- When skipping analysis: mark `⚠️ UNVERIFIED:` so it's clear work is still needed
- When intentionally diverging: use `⚠️ SIMPLIFIED:` with justification

**Examples of good annotations:**
```rust
// ✅ VERIFIED: Structure matches C's af_latin_hints_apply. Flags match
//    aflatin.c:2671-2698 for smooth anti-aliased rendering.

// ⚠️ BUG: C's smooth path (aflatin.c:4016-4075) uses inline
//    |dist - standard| < 40 check + fractional pixel quant.
//    We incorrectly call snap_width() which is strong-hinting only.
```

**Current verification status** is tracked in `pillow-rs-freetype/doc/TASKS.md`
under "Code Annotations Added". Always read it before starting autohinter work
so you don't re-investigate verified functions.

### 12. Systematic debugging loop (per-pass trace → first divergence → fix)

When iterating on autohinter bugs, this loop finds real bugs within minutes:

**Step A — Dump per-pass intermediate coordinates.**  Add temporary
`eprintln!` traces that dump ALL point coordinates after each hinting pass:

```rust
// After hint_edges → dump edge positions + point x values
// After align_edge_points → dump point x values + which got touched (T/.)
// After align_strong_points → dump point x values + touch flags
// After IUP → dump point x values
```

This isolates WHICH function produces the first divergent value.

**Step B — Binary search on point index.**  Once you know point N is the
first mismatch, grep C's source for the function that touches point N:
- If p5 diverges after `align_edge_points` → check segment→edge chain and
  compare edge positions between C and Rust.
- If p5 diverges after `align_strong_points` → the bug is in the
  interpolation formula or the edge that got assigned.
- If p5 diverges after `align_weak_points` (IUP) → the bug is in
  reference-point values fed into IUP, not IUP itself.

**Step C — Compare edge data structures at every stage.**  Before each
phase, dump `fpos`, `opos`, `pos`, `link`, `serif`, `flags` (especially
`AF_EDGE_DONE`) for every edge.  Compare with C's `FT2_DEBUG="aflatin:7"`
trace which emits `ANCHOR`, `STEM`, `LINK`, `SERIF_LINK2`, `ADJUST` lines.
A single missing DONE flag or wrong link index cascades to everything
in the next phase.

**Step D — Always check C's internal helper functions.**  When our code has
`// We skip…` or `// Simplified…`, grep the C source for the function
behind the simplification.  For example:
- "We don't compute edge v coordinates" → check C's `afhints.c` for how
  `edge->first->first->v` is used in the serif overlap check.
- "in_dir==out_dir==NONE: we skip" → C has `ft_corner_is_flat()`.
- "all blue_edge are NULL" → C assigns them in `compute_blue_edges`.

Every simplification is a potential bug.  Implementing the missing function
has been the source of the three largest fixes (+137, +71, +27 tests).

**Step E — Audit operator precedence in bitwise expressions.**  Rust's
`&` binds LOOSER than binary `-` and `+`.  C's `&` binds TIGHTER than `-`.
Every expression of the form `(expr) & !N - val` is WRONG in Rust.
Must be `((expr) & !N) - val`.  Same for `(expr) & !N + val`.

Checklist when facing unknown divergence:
```
[ ] Point coordinates match C before hinting? → scaler is correct
[ ] Edge fpos/opos match C before hint_edges? → segment/edge detection correct
[ ] Edge pos match C after hint_edges? → Phase 1-4 correct
[ ] Touch flags match C after align_edge? → segment-chain correct
[ ] Strong-point positions match C? → scale interpolation correct
[ ] IUP positions match C? → weak-point classification + IUP correct
[] If all of the above match but SHA differs → rasterizer or outline→bitmap path
```

## Rules

- Public API names match Pillow exactly. Import name: `RSPIL`.
- Reference: **Pillow** for API, **Puhu** (`puhu/`) for algorithms/quirks
- NEVER use git (`commit`, `checkout`, `revert`, `stash`) without explicit permission
- NEVER change fixture output/input JSON images or binaries
- `pillow-rs-py` must contain NO `if`/`else` — all logic in core
- Never leave commit message as "anthropic" or "fable"
- Run only failing tests; remove `show()` function
- Timeout tests at 3 minutes
- Research exact algorithms via internet when needed
- Write separate code paths per mode when needed
- Don't give tasks back to user — do it all yourself
- Add tasks to task list and don't stop until done

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
- For pipeline trace debugging (C→Rust parity), use `trace!(target: "autohint::pipeline", ...)`
  wrapped in `#[cfg(debug_assertions)]` and `log::log_enabled!` guard.
  Enable via: `RUST_LOG=autohint::pipeline=trace`
  See `pillow-rs-freetype/src/autohint/latin.rs:apply_hints` for the template.
- **Pipeline trace statements are permanent** -- never revert them.  They are
  `#[cfg(debug_assertions)]`-gated with `log_enabled!` guard, so they compile
  to zero instructions in release builds.  Commit them with the code.

## Rust Code Style

Delegate to `rust-development` skill. Key repo specifics:
- `thiserror` for errors, never `unwrap()`/`expect()` outside tests
- `&str` over `String`, `&[T]` over `Vec<T>` in parameters
- `cargo clippy --all-targets --all-features -- -D warnings` before commit

## Manifest-Driven Development

All work starts from `manifest.yaml` — the single source of truth for the API surface.

## Autohinter Parity Status

**Current: 10,601/11,084 passed (95.6%), 483 failures** (2026-07-01)

### Fix 1: top_to_bottom dimension gating (853→569, -284)
Bug: `hint_edges` applied `top_to_bottom_hinting` to BOTH dimensions. C gates to VERT
only (aflatin.c:4271-4273). Fix: `dim == Dimension::Vert &&` guard at line 1937.
Scripts fixed to 100%: beng, guru, goth, mong.

### Fix 2: blue zone outlier detection (569→483, -86)
Without HarfBuzz GSUB, some script-specific standard characters produce unshaped forms
with wrong Y (e.g., knda saknda y=790 instead of headline y=563). Blue zone ref
picked the flat median over the correct round median. Fix: when flat/round medians
differ >20% upem, trust rounds for top zones and flats for bottom zones.
Scripts fixed to 100%: knda, gujr, lao, mlym, sinh, sund, taml.

### Remaining: 483 failures (19 scripts)
- Heavy: adlm (72%), hani (60%), nkoo (36%), deva (24%), cher (22%), hebr (19%)
- Moderate: latb/latp (5-7%), geok (7%)
- Light: cans (3%), telu (2%), thai (2%), mymr (4%), etc.

### Debug tools
- C binary: `/tmp/gen_refs_v4` links `pillow-rs-freetype/freetype/build/libfreetyped.so`
- C trace: `FT2_DEBUG="aflatin:7" LD_LIBRARY_PATH=pillow-rs-freetype/freetype/build /tmp/gen_refs_v4`
- Rust trace: `RUST_LOG=autohint::pipeline=trace cargo run -p pillow-rs-freetype --example debug_glyph`
- Test: `cargo test -p pillow-rs-freetype --test direct_ft_compare`
- Plan: `pillow-rs-freetype/doc/MASTER_PARITY_PLAN.md`

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
- **NO git reverts** — never `git revert` or `git reset --hard` to undo changes. If a code change
  causes test regressions, fix forward by editing the code to correct the issue. Always move
  forward, never go back. Git history is there as backup if absolutely needed.
- NEVER change fixture output/input JSON images or binaries
- `pillow-rs-py` must contain NO `if`/`else` — all logic in core
- Never leave commit message as "anthropic" or "fable"
- Run only failing tests; remove `show()` function
- Timeout tests at 3 minutes
- Research exact algorithms via internet when needed
- Write separate code paths per mode when needed
- Don't give tasks back to user — do it all yourself
- Add tasks to task list and don't stop until done

## Parity Debugging Methodology (C → Rust Port)

This section documents the exact process used to achieve 99.97% pixel parity
with FreeType's autohinter. Follow these steps in order for any C→Rust port.

### Step 1: One Glyph, One Size, Two Binaries

**Never debug through test suites.** Test fixtures interleave traces from
thousands of glyphs. Build standalone C and Rust binaries that load exactly
one font, one glyph, one size:

```rust
// Rust: pillow-rs-freetype/examples/trace_one.rs
fn main() {
    let fp = "/absolute/path/to/font.ttf";
    let data = std::fs::read(fp).unwrap();
    let font = Font::truetype(&data, 12.0, BitmapBackend::FreeType).unwrap();
    font.getmask("5").unwrap(); // ONE call, all traces from THIS glyph
}
```

```c
// C: compile with vendored FreeType
int main() {
    FT_New_Memory_Face(lib, buf, sz, 0, &face);
    FT_Set_Char_Size(face, 12*64, 0, 72, 0);
    FT_Load_Glyph(face, FT_Get_Char_Index(face, 0x35),
                  FT_LOAD_RENDER | FT_LOAD_FORCE_AUTOHINT);
    return 0;
}
```

### Step 2: Dump Every Pipeline Stage

Instrument BOTH C and Rust with `fprintf(stderr, ...)` / `eprintln!()` at each stage:

| Stage | C location | Rust location | What to dump |
|-------|-----------|---------------|-------------|
| reload | afhints.c:1014 | loader.rs:68 | ox[0..5], fx[0..5] |
| compute_edges | aflatin.c:2310 | latin.rs:1099 | fpos, opos, pos for every edge |
| hint_edges | aflatin.c:4244 | latin.rs:1665 | edges after each phase (1-4) |
| align_edge_points | afhints.c:1369 | latin.rs:2112 | which points got TOUCH_X |
| align_strong_points | afhints.c:1585 | latin.rs:2169 | x, ox, fx for all grid-fitted points |
| IUP (align_weak) | afhints.c:1798 | latin.rs:2330 | reference pair (v1,u1,v2,u2) |
| Save to outline | afhints.c:1320 | latin.rs:840 | final x, ox for all points |
| Phantom adjust | afloader.c:518 | latin.rs:820 | pp1x value, shift applied |

### Step 3: Find the First Diverging Point

Compare C and Rust output stage by stage. The FIRST stage that differs is
the bug location. Everything downstream is a consequence.

**Example trace comparison (finding the WEAK_INTERPOLATION bug):**

```
Stage                    C                           Rust
reload                   ox[0..5]=185,133,64,31,31   same           ✓
compute_edges (HORZ)     5 edges: fpos=40,120,...     same           ✓
hint_edges (phase 4)     pos=0,64,256,348,375         same           ✓
align_edge_points        pt[20] not yet touched       same           ✓
align_strong_points      pt[20] x=33, TOUCHED         pt[20] WEAK     ✗ ← BUG HERE
IUP                      refs (pt[14], pt[20])        refs (pt[14], pt[21])
Final                    pt[15]=201                   pt[15]=200     +1 diff
```

### Step 4: Trace the Diverged Function Internals

Once you know WHICH function diverges, trace its internals. For `align_strong`,
pt[20] was skipped because WEAK_INTERPOLATION was set. Tracing backward:

```
reload WEAK classification:  C flags=0x00 (STRONG)    Rust flags=0x10 (WEAK)
  → both-None case:          C corner_is_flat=FALSE   Rust corner_is_flat=TRUE
    → different inputs:       C in=(-103,-60)          Rust in=(-11,4)
      → different u/v:        C v=-3 (pt[17])          Rust v=-1 (pt[19])
        → direction chain     near_limit=9 affects     same near_limit but
          different point merging at UPEM=1000          different first-point
```

### Step 5: Verify Against C Source Code

Read the EXACT C function. Look for sequential checks that our code might
have combined, or index-delta updates that our code might skip.

**Key finding (afhints.c:1221-1290):** C checks XOR quadrant first, THEN
`ft_corner_is_flat` separately. When corner_is_flat returns true, C updates
`prev_v->u = next_u - prev_v` and `next_u->v = -prev_v->u`. These delta
updates change the direction chain for DOWNSTREAM point classifications.
Our Rust code had `xor || corner_is_flat(...)` as a single boolean — the
short-circuit OR skipped the delta update when XOR already evaluated to true.

### Step 6: Fix the Minimal Code

Fix ONLY what differs. One function, minimal change. Run the full test suite.
If other glyphs still fail, the same mechanism affects different contour
topologies — the root cause is the same.

## Case Study: The 2026-06-30 WEAK_INTERPOLATION Fix

### Timeline

| Phase | What | Result |
|-------|------|--------|
| Start | 541 failures | Baseline |
| Fix 1 | walk_contour conic wrap (commit 887070a) | 411 failures |
| Fix 2 | getlength from hmtx (commit cf19f9e) | 313 failures |
| Fix 3 | getmetrics FT_PIX_CEIL (commit cbbdcba) | 309 failures |
| Fix 4 | pp1.x phantom translation (commit 04975f8) | 18 failures |
| Fix 5 | WEAK_INTERPOLATION classification (commit 1ecd364) | 9 failures |

### What the fix changed (loader.rs:217-233)

Before:
```rust
// C's XOR quadrant check (afhints.c:1221-1245): same sign for both axes
((in_x ^ out_x) >= 0 && (in_y ^ out_y) >= 0)
    || corner_is_flat(in_x, in_y, out_x, out_y)
```

After:
```rust
// C (afhints.c:1276-1290): XOR check, then corner_is_flat
let xor_same = (in_x ^ out_x) >= 0 && (in_y ^ out_y) >= 0;
if xor_same {
    true
} else if corner_is_flat(in_x, in_y, out_x, out_y) {
    // Update index deltas (C: afhints.c:1286-1287)
    hints.points[pv].u = nu as i32 - pv as i32;
    hints.points[nu].v = -(hints.points[pv].u);
    true
} else {
    false
}
```

And the spike branch:
```rust
// Before: flags & AF_FLAG_NEAR != 0
// After:  true  (C's afhints.c:1293 unconditionally marks spikes as WEAK)
```

### Why 12 lines fixed 9 tests

The delta update (`pv->u`, `nu->v`) changes which neighbor points are
consulted for subsequent point classifications. At UPEM=1000, `near_limit=9`
FU creates a dense direction-chain network where a single delta change
propagates through 5+ downstream points, flipping their WEAK/STRONG status.
This changes IUP reference pairs, producing +1 unit coordinate differences
that cascade through `render_conic` subdivision into pixel mismatch.

### Remaining 9 failures

Same mechanism (WEAK classification), different glyph topologies:
- NSDB 'B' (gid=37): 3 contours, different u/v chain
- NSDB 'g' (gid=74): descender contour, near_limit affects descending arc
- LiberationSerif '$': UPEM=2048, bold stem-width thresholds
- LiberationMono 'l': monospace, different standard_width
- LiberationSansNarrow ';': narrow italic, NO_HORIZONTAL + narrow metrics

### Documentation

Autohinter documentation: `pillow-rs-freetype/src/autohint/mod.rs` (pipeline + font categories) and doc comments on key functions (`reload`, `apply_hints`, `align_weak_points`, `build_direction_chain`)

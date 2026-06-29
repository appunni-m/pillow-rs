# CI Pipeline Analysis & Freetype Integration Report

_Generated 2026-06-29 from analysis of `.github/workflows/ci.yml` and test runs._

## 1. CI Pipeline Structure (`.github/workflows/ci.yml`)

The CI has **5 parallel jobs** (Rust tasks run in a matrix, Python across 3 versions):

| Job | What it does | Current Status |
|-----|-------------|----------------|
| `rust (fmt-clippy)` | `cargo fmt --check` + `cargo clippy --all-targets --all-features` | **BROKEN** — multiple crates fail clippy |
| `rust (test)` | `cargo test -p pillow-rs` | ✅ 64 core tests pass |
| `rust (supply-chain)` | `cargo deny check` + `cargo audit` | Not verified |
| `python` (3.8/3.10/3.12) | `maturin develop --release` → `scripts/check_bindings.py` → `pytest` → coverage | **BLOCKED** — depends on `rust` job passing first (`needs: rust`) |
| `wasm` | `wasm-pack build --target web --dev` | Not verified |

### Why CI fails

The `rust (fmt-clippy)` job is the gate: it runs `cargo clippy --all-targets --all-features` which compiles **every workspace crate including examples and tests with all lints denied**. This fails across three crates.

Since `python` `needs: rust`, the entire pipeline is blocked at the first stage.

---

## 2. Clippy Errors by Crate

### pillow-rs-freetype — 9 broken examples + 1 broken test

Examples reference APIs that have been refactored (private fields, different function signatures):

| Example | Error count | Root cause |
|---------|------------|------------|
| `trace_raster` | 6 | Accesses private field `font.data`; wrong arg count to `apply_hints` |
| `trace_segments_o` | 8 | Calls private `compute_segments` and `link_segments_inner` |
| `validate_hinter` | 2 | `AfLatinMetrics::new()` now takes 2 args (upem + num_glyphs), called with 1 |
| `dump_all_masks` | 5 | Similar API decay |
| `dump_mask_compare` | 6 | Similar API decay |
| `dump_outline` | 1 | API mismatch |
| `dump_metrics` | 1 | `AfLatinMetrics::new()` arg count |
| `trace_hint_edges` | 5 | Private function access |
| `debug_glyph` | 3 | API mismatch |
| `cmp_glyph` | 3 | API mismatch |
| **`fixed_parity` (test)** | 2 | Same `AfLatinMetrics::new()` mismatch |

**Fix options:**
- **Option A:** Update all examples and tests to match current API (invest ~2-4 hours)
- **Option B:** Exclude examples from clippy scope (`cargo clippy --lib --tests --bins -- -A deprecated` instead of `--all-targets`). Quick CI fix but leaves examples unmaintained.

### pillow-rs-image — 6 clippy deny errors

| File | Error | Line |
|------|-------|------|
| `encode/webp/vp8/dct.rs` | `approx_constant` — custom `S2 = 0.707...` instead of `std::f64::consts::FRAC_1_SQRT_2` | 110 |
| `encode/webp/mod.rs` | `unnecessary_cast` — `quality as u8` when `quality` is already `u8` | 42 |
| `encode/webp/vp8/encoder.rs` | `redundant_clone` — 3 unnecessary `.clone()` on planes | 1102-1104 |
| `encode/webp/vp8/tokenize.rs` | `absurd_extreme_comparisons` — `p <= 255` where `p: u8` is always true | 741 |

All are trivial to fix (< 10 minutes).

### pillow-rs (core) — 4 clippy deny errors

| File | Error | Line |
|------|-------|------|
| `compute/pool_cpu/ops/filter.rs` | `unnecessary_cast` — `w as u32` where `w` is already `u32` | 583 |
| `compute/pool_cpu/ops/filter.rs` | `unnecessary_cast` — `h as u32` where `h` is already `u32` | 588 |
| `font/mod.rs` | `unnecessary_cast` — `mask.advance_width as i32` where already `i32` | 112 |
| `font/mod.rs` | `unnecessary_cast` — `mask.advance_width as i32` where already `i32` | 247 |

All trivial to fix (< 5 minutes).

### Additional: 3,000+ warnings

`cargo clippy --all-targets --all-features` also produces ~3,000+ warnings (mostly `arithmetic_side_effects`, `missing_docs`, `cast_possible_truncation`, `cast_sign_loss`). These are `warn`-level in `Cargo.toml` and do **not** block CI. They should be addressed incrementally.

---

## 3. Freetype Integration Architecture

```
pillow-rs-py (Python bindings)
  └── pillow_rs (core Rust library)
       └── pillow-rs-font (thin façade, re-exports)
            └── pillow-rs-freetype (pure-Rust FreeType 2.14.1 port)
```

**Integration point:** `pillow-rs/src/font/mod.rs:69-75`

```rust
let inner = pillow_rs_font::Font::truetype(
    &data, size, pillow_rs_font::BitmapBackend::PIL
)
```

**`BitmapBackend::PIL`** is the special integration flag. It tells the freetype port to render in PIL-compatible coordinates:

| Backend | Coordinate system | Origin |
|---------|-------------------|--------|
| `BitmapBackend::PIL` | y-down from ascender | Advance-based width with baseline padding |
| `BitmapBackend::FreeType` | y-up from baseline | Raw FreeType bounding box |

Defined in `pillow-rs-freetype/src/font.rs:16-20` and `pillow-rs-freetype/src/font.rs:380-420`.

**Conclusion: pillow-rs IS integrated with pillow-rs-freetype.** The freetype code is compiled, linked, and actively rendering glyphs in test runs. The failures are pixel-level mismatches, not linkage gaps.

---

## 4. Fixture Test Results (774 total cases, 2026-06-29)

```
Suite 0 (core functions):  752 passed,   2 failed  → 99.7% pass rate
Suite 1 (font/deform):       1 passed,  22 failed  →  4.3% pass rate
──────────────────────────────────────────────────────────────
Total:                     753 passed,  24 failed  → 96.9% pass rate
```

### 22 Suite 1 failures detail

**20 font rendering failures (pillow-rs-freetype):**

| Category | Tests | Failure mode |
|----------|-------|--------------|
| `FreeTypeFont` truetype rendering | 4: liberation_serif_20, dejavu_sans_16, L, RGBA | Image mismatch |
| `TransposedFont` | 2: rotate_90, flip_left_right | Image mismatch |
| `getmask` | 4: default, L, RGB, RGBA modes | Image mismatch |
| `getmask2` | 4: default, L, RGB, RGBA modes | Tuple + image mismatch |
| `load_default` | 3: L, RGB, RGBA modes | Image mismatch |
| `load_default_imagefont` | 3: L, RGB, RGBA modes | Image mismatch |

All produce rendered glyphs — their sizes are correct — but pixels don't match PIL reference byte-for-byte. Common causes in a FreeType port:
- Rasterizer differences (sub-pixel positioning, anti-aliasing levels)
- Autohinter not matching FreeType's `af_latin_hints_*` output
- Glyph metrics differences (advance width, ascender/descender)

**2 non-font failures:**

| Test | Error |
|------|-------|
| `ImageOps.deform__L_suite1` | `ValueError: expected tuple of length 2, but got tuple of length 4` |
| `ImageOps.deform__RGB_suite1` | Same error |

Bug in `mesh_flatten` → input shape handling in `pillow-rs-py/python/pillow_rs/image.py:650`.

---

## 5. Manifest Coverage Warnings

50 operations in `manifest.yaml` are marked `status: implemented` but have no matching fixture, or their `@pytest.mark.covers` references an unknown operation name:

- `Image.new` (10 modes)
- `ImageFont.{getbbox,getlength,getmask,getmask2}` (4 modes each)
- `ImageFont.{font_variant,get_variation_axes,get_variation_names,getmetrics}`
- Various `ImageDraw.*` operations

These produce warnings during test collection but do NOT fail the test run (unless `--strict-covers` is set). They indicate the manifest is ahead of the fixture set.

---

## 6. Quickest CI Fix Plan

### Phase 1 — Unblock CI (make `rust` job pass)

**Minimal approach (~15 min):**

1. Change clippy invocation in `.github/workflows/ci.yml` from `--all-targets` to `--lib --tests --bins` so broken freetype examples are excluded.
2. Fix the 10 trivial clippy deny errors in `pillow-rs-image` (6) and `pillow-rs` (4).

This unblocks the `python` job, allowing test results to surface in CI. Freetype examples can be fixed in a separate PR.

### Phase 2 — Fix 24 test failures

1. **22 font failures:** Compare pillow-rs-freetype rasterizer/autohinter output against PIL's bundled FreeType 2.14.3. See `pillow-rs-freetype/doc/TASKS.md` and the [debugging protocol](../CLAUDE.md#debugging-protocol-for-porting-crates) in CLAUDE.md.
2. **2 deform failures:** Fix `mesh_flatten` tuple handling in `pillow-rs-py/src/lib.rs` or the Python wrapper.

### Phase 3 — Coverage hygiene

1. Either add fixtures for the 50 missing manifest operations, or mark them `status: planned` / `status: stubbed`.
2. Register `Image.new` and `ImageFont` operations in `manifest.yaml` if they are genuinely implemented.

---

## 7. Environment Notes

- **Rust:** `rustc 1.91.0` (stable)
- **Pillow:** 12.2.0 (bundles FreeType 2.14.3)
- **Python:** 3.12.11
- **pillow-rs-freetype:** Ports FreeType 2.14.1 — **1 patch version behind** PIL's bundled FreeType 2.14.3. The autohinter changed between these versions, which can cause systematic pixel mismatches at large scale. See `CLAUDE.md` debugging protocol rule #2.

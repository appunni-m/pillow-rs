# pillow-rs Comprehensive Codebase Audit

**Date:** 2026-06-20
**Scope:** `pillow-rs/` (core), `pillow-rs-py/` (Python bindings), CPU backend, image implementation
**Excluded:** JS/WASM bindings (in progress), SIMD (partial), GPU (partial)
**Focus:** Security, Performance, Maintainability — ranked by severity

---

## Table of Contents

1. [Security Issues](#1-security-issues)
2. [Performance Issues](#2-performance-issues)
3. [Architecture & Design Issues](#3-architecture--design-issues)
4. [Python Binding Issues](#4-python-binding-issues)
5. [Dependency & Supply Chain Issues](#5-dependency--supply-chain-issues)
6. [Testing & Quality Issues](#6-testing--quality-issues)
7. [Maintainability Issues](#7-maintainability-issues)
8. [Summary Action Plan](#8-summary-action-plan)

---

## 1. Security Issues

### 1.1 [CRITICAL] Integer Overflow in Dimension Multiplication

**Location:** 30+ sites across the codebase
**Files:** `image.rs:232-237`, `filter.rs:256,280,328,366,573,588`, `geometry.rs:640`, `chops.rs:64,127`, `color.rs:79,114,196`

**Description:** The pattern `(w * h) as usize` and `(w * h * channels) as usize` silently wraps on overflow because `w` and `h` are `u32`. For example, `w = 0x10001, h = 0x10001` → `w * h = 0x1` (truncated). The tiny buffer then gets indexed with the full logical dimensions → out-of-bounds memory access.

```rust
// Current (broken):
let buf = vec![0u8; (w * h) as usize * channels];  // wraps!

// Fixed:
let total = (w as u64)
    .checked_mul(h as u64)
    .and_then(|p| p.checked_mul(channels as u64))
    .ok_or_else(|| PilError::ValueError("image dimensions overflow".into()))?;
let buf = vec![0u8; total as usize];
```

**Recommendation:** Create a `checked_pixel_count(w, h, channels) -> Result<usize>` helper and use it everywhere dimensions are multiplied for allocation.

---

### 1.2 [HIGH] Allocation DoS via Unbounded Dimensions

**Location:** `image.rs:148-196` (`Image::new`), `image.rs:226-319` (`frombytes`), `effects.rs:1208-1241` (`op_effect_mandelbrot`), `geometry.rs:400,687` (`rotate`, `transform`)

**Description:** No upper bound on image dimensions before allocation. `Image::new(65536, 65536)` allocates 16 GB. `frombytes` with 8 bytes of "data" can request any dimension. `EffectMandelbrot` takes attacker-controlled `w,h` directly.

**Recommendation:** Add `MAX_PIXELS = 268_435_456` (matching PIL's `MAX_IMAGE_PIXELS`, ~1 GB RGBA) and check at every allocation entry point. Make it user-overridable like PIL's `Image.MAX_IMAGE_PIXELS`.

---

### 1.3 [HIGH] Panics in Production Code via `unwrap()`/`expect()` on `from_raw()`

**Location:** `geometry.rs:463-475, 558-574, 716-725`, `effects.rs:773-784, 1106, 1201`

**Description:** 20+ `.unwrap()`/`.expect()` calls on `DynamicImage::from_raw()` that panic if the buffer size mismatches computed dimensions. This is the exact result of an unchecked integer overflow (1.1). Instead of a clean error, the process crashes.

```rust
// Current (panics):
let img = RgbaImage::from_raw(w, h, out).unwrap();

// Fixed:
let img = RgbaImage::from_raw(w, h, out)
    .ok_or_else(|| PilError::ValueError("buffer size mismatch in rotate".into()))?;
```

**Recommendation:** Replace every `unwrap()`/`expect()` on `from_raw()` with `ok_or_else(|| PilError::ValueError(...))?`.

---

### 1.4 [MEDIUM] Draw `width` Parameter Enables CPU DoS

**Location:** `draw.rs:60-68` (`draw_line_on_canvas`), `draw.rs:102-113` (`draw_rect_on_canvas`)

**Description:** `width: u32` in draw ops drives unbounded loops. `width = 1_000_000` means 1 million Bresenham line iterations per parallel offset. Each Bresenham call loops over pixel length.

**Recommendation:** Clamp `width` to `min(canvas_width, canvas_height).min(100)`.

---

### 1.5 [MEDIUM] Mesh Transform Unbounded Iteration

**Location:** `effects.rs:1113-1203`

**Description:** `transform_mesh` reads mesh bounding boxes from attacker-controlled data. bx/bw values can be `i32::MAX`, creating loops with 10^18 iterations.

**Recommendation:** Clamp mesh element bounding boxes to output image dimensions.

---

### 1.6 [MEDIUM] Division-by-Zero / Float NaN in Effect Noise

**Location:** `effects.rs:630-636`

**Description:** `op_effect_noise` Box-Muller transform: `(-2.0 * radius.ln() / radius)`. When `radius` approaches `f64::MIN_POSITIVE`, this produces `Inf`/`NaN`. `ln(0.0)` panics in debug mode (Rust ≥ 1.67).

**Recommendation:** Add epsilon guard: `if radius < 1e-10 { continue; }`.

---

### 1.7 [MEDIUM] Palette Index Integer Overflow on 32-bit

**Location:** `image.rs:473, 980-984, 1062-1067`, `draw/mod.rs:625-634`

**Description:** `idx * 3` where `idx = pixel_value as usize`. If `pixel_value > 255` (possible from raw byte sources), this overflows `usize` on 32-bit platforms. Currently partially guarded by `.get()` bounds checks.

**Recommendation:** Consistently validate `idx < 256` before the multiply, or mask with `& 0xFF`.

---

### 1.8 [LOW] Path Traversal in File Operations

**Location:** `pillow-rs-py/src/lib.rs:60-72, 1094-1098, 2245-2248`

**Description:** File paths from Python user code are passed directly to `std::fs::read/write` without sanitization. Matches PIL's behavior but lacks defense-in-depth.

**Recommendation:** At minimum, document this in the security policy. Consider canonicalizing and validating paths are within expected directories.

---

### 1.9 [LOW] Eval/PointOp LUT Validation Exists but Could be Stricter

**Location:** `effects.rs:567-578, 656-667`

**Description:** LUT length is validated as `lut.len() == 256 * n_bands`, which is correct. The slicing `&lut[b * 256..(b + 1) * 256]` is then guaranteed in-bounds. No vulnerability, but `n_bands` could theoretically be huge if pipelines are constructed manually.

---

### 1.10 [POSITIVE] No Unsafe Code

**Finding:** Zero `unsafe` blocks in the entire `pillow-rs/src/` codebase. Rust memory safety guarantees apply fully. This is excellent for an image processing library handling untrusted input.

---

## 2. Performance Issues

### 2.1 [CRITICAL] No Parallelism — Zero Rayon Usage

**Location:** Entire codebase

**Description:** `rayon` is not a dependency. All pixel loops are serial. The following are trivially parallelizable at the row level:
- `pil_resize` horizontal pass (each source row independent)
- `pil_resize` vertical pass (each output row independent)  
- `execute_filter3x3`/`execute_filter5x5` (each output pixel independent)
- `pil_box_blur` horizontal/vertical passes
- `channel_op_binary` (each pixel independent)
- All `ImageChops` operations
- `rank_filter_impl` (each pixel independent)
- `op_enhance_*` (each pixel independent)

**Recommendation:** Add `rayon` and use `par_chunks()` at the row level. Expected 4-16x speedup on modern CPUs for large images.

---

### 2.2 [HIGH] Box Blur is O(w·h·radius) Instead of O(w·h)

**Location:** `filter.rs:254-300`

**Description:** For every pixel, the box blur inner loop recomputes the full window sum: `for dx in -r_int..=r_int { acc += work[idx]; }`. This is O(radius) per pixel. A sliding window approach (subtract exiting pixel, add entering pixel) is O(1) per pixel.

Additionally, each pass allocates new `hpass` and `vpass` Vecs. For a 3-pass Gaussian blur, that's 6 full-image heap allocations.

**Recommendation:** Implement sliding-window accumulator and pre-allocate two ping-pong buffers.

---

### 2.3 [HIGH] Rank Filter Allocates Vec Per Pixel

**Location:** `filter.rs:308-389`

**Description:** Every pixel allocates a fresh `Vec<u8>` (or `Vec<f32>`) with `area` capacity, fills it, calls `sort_unstable()`, then drops it. For 1000×1000 with 5×5 kernel = 1M small Vec allocations.

**Recommendation:** Use stack-allocated array `[u8; MAX_KERNEL_SIZE]` and `select_nth_unstable()` instead of `sort_unstable()` — the latter is O(n log n) when O(n) suffices for finding a single rank element.

---

### 2.4 [HIGH] `resize_f` Uses 2D Direct Convolution Instead of Separable

**Location:** `geometry.rs:183-233`

**Description:** Non-Nearest `resize_f` does direct 2D convolution: O(dst_w·dst_h·support_x·support_y). `resize_i` correctly uses two-pass separable decomposition (O(dst_w·src_h·support + dst_h·dst_w·support)). For Lanczos-3, this is 6× slower than necessary.

**Recommendation:** Make `resize_f` use the same separable approach as `pil_resize` and `resize_i`.

---

### 2.5 [HIGH] Rounded Rectangle Clones Image N Times

**Location:** `draw.rs:756-913`

**Description:** `op_draw_rounded_rect` clones the full image, then calls `op_draw_pieslice` 4× and `op_draw_rectangle` 3×, each of which re-clones and does a full RGBA roundtrip. For a single rounded rect, the image gets cloned and converted ~7×.

**Recommendation:** Create one RGBA canvas, perform all draw operations on it, convert back once.

---

### 2.6 [HIGH] Ellipse Outline Duplicates Bresenham Walk

**Location:** `draw.rs:219-300`

**Description:** `draw_ellipse_on_canvas` runs the identical Bresenham quarter-ellipse generator twice — once for fill, once for outline. The outline pass additionally scans all `img_w × img_h` pixels for edge detection.

**Recommendation:** Run the Bresenham generator once, record both fill and edge pixels simultaneously. Eliminate the full-image edge detection scan by tracking edges during the walk.

---

### 2.7 [MEDIUM] Nearest-Neighbor Resize Uses Virtual Dispatch in Hot Path

**Location:** `pil_resize.rs:648`

**Description:** Each nearest-neighbor output pixel calls `pixel_at(&work, sx, sy)`, which does `match` on `DynamicImage` variant and calls `get_pixel()` (virtual dispatch). For a 4000×4000→2000×2000 downscale, this is 4M virtual calls.

**Recommendation:** After determining channels, index raw bytes directly: `work_bytes[(sy * sw + sx) * channels..]`.

---

### 2.8 [MEDIUM] Channel Op Inner Loop is Cache-Unfriendly

**Location:** `chops.rs:66-75`

**Description:** The pixel loop `for y { for x { for c { ... } } }` with inner channel loop causes stride access: consecutive channels of the same pixel are not adjacent in memory (gap = image width × channels). This kills L1 cache.

**Recommendation:** Restructure to `for y { for x { let base = y * stride + x * ch; for c { out[base+c] = op(a[base+c], b[base+c]); } } }`.

---

### 2.9 [MEDIUM] Redundant Premultiply/Unpremultiply Passes in Resize

**Location:** `pil_resize.rs:588-704`

**Description:** For RGBA/LA images, `pil_resize` premultiplies alpha (full-image O(w·h) float pass), runs the two-pass resize, then unpremultiplies (another O(w·h) float pass). Precision is lost in the u8 roundtrip.

**Recommendation:** Fold premultiplication into the weighted sum accumulator — scale pixel values at accumulation time without an intermediate u8 premultiply buffer.

---

### 2.10 [MEDIUM] `pil_round` Branches in Hot Loop

**Location:** `pil_resize.rs:125-134`

**Description:** Called once per output channel. Uses three branches (`v <= 0.0`, `v >= 256.0`, else). Each branch can mispredict near extreme values.

**Recommendation:** Replace with `(v + 0.5) as i32; v.clamp(0, 255) as u8` — the `clamp` compiles to branchless `cmov` on x86.

---

### 2.11 [MEDIUM] Lanczos Kernel Recomputes `PI * x` Twice

**Location:** `pil_resize.rs:49-60`, duplicated in `geometry.rs:49-60`

**Description:** `kernel_lanczos` computes `PI * x` twice independently. In the inner loop of coefficient precomputation for every output pixel dimension.

**Recommendation:** Compute `pix = PI * x` once, then `let pix_a = pix / a;` and reuse.

---

### 2.12 [MEDIUM] `try_split` Scans Full Histogram 5× Per Split

**Location:** `quantize.rs:431-608`

**Description:** Every call to `try_split` does 5 full scans of ~65,536 entries: filter for box contents, left bounds, left count, right bounds, right count. With 255 splits, that's 255 × 5 × 65,536 = ~83 million iterations.

**Recommendation:** Pass pre-filtered entry list. Cache bounds/counts from scan #1 rather than re-scanning 4 more times.

---

### 2.13 [MEDIUM] Chops Invert Double-Allocates

**Location:** `chops.rs:364-392`

**Description:** `op_chops_invert` copies the full pixel buffer with `raw.to_vec()`, then wraps in `from_raw`. This allocates the buffer twice.

**Recommendation:** Clone the `DynamicImage` and mutate in-place via `as_mut_bytes()` or similar.

---

### 2.14 [LOW] LUT Operations Recompute Byte Offsets Per Channel

**Location:** `chops.rs:127-137`

**Description:** `channel_op_binary_lut` recomputes `a_idx`, `b_idx`, `o_idx` and the LUT index for each channel `c` inside the innermost loop, when only the channel offset changes.

**Recommendation:** Hoist base offsets outside the channel loop.

---

### 2.15 [LOW] Thumbnail Reduce Checks Bounds on Every Interior Pixel

**Location:** `geometry.rs:868-888`

**Description:** The thumbnail reducing-gap loop checks `min(cur_h-1, ...)` and `min(cur_w-1, ...)` on every pixel, but these guards only matter for edge blocks.

**Recommendation:** Split into fast path (interior blocks, no bounds checks) and slow path (edge blocks).

---

### 2.16 [LOW] Quantize Octree Uses 4-Level Nested Loop

**Location:** `quantize.rs:1225-1335`

**Description:** Lookup cube population uses `for fri { for fgi { for fbi { for fai { ... } } } }`. Manageable at current sizes but would benefit from precomputed fine-to-coarse mapping.

---

## 3. Architecture & Design Issues

### 3.1 [HIGH] Pipeline Mode Tracking Bug — Checks Root Source, Not Accumulated State

**Location:** `image.rs:422-456`

**Description:** `materialize()` computes `is_p_mode` from `source.explicit_mode()` — the root source's explicit mode, NOT the accumulated mode after all pipeline ops. Consider:

```
Paletted("P") → push Convert(RGB) → push Crop
```

`is_p_mode` reads `"P"` from the root source, even though `Convert(RGB)` changed the mode. This can cause the materializer to apply palette-index-safe logic to RGB data.

**Recommendation:** Track the effective mode incrementally as ops are pushed. Store the accumulated mode on the `Pipeline` variant rather than reading the root.

---

### 3.2 [HIGH] Double-Apply in `materialize_indices`

**Location:** `image.rs:497-509`

**Description:** `materialize_indices()` calls `source.materialize()` (which executes the pipeline), then calls `execute_batch(backend, ops, &img, Some("P"))` with the **same** `ops` vector. The ops get applied twice.

**Recommendation:** Separate "materialize the data source" from "apply pending operations." Don't apply ops that were already applied.

---

### 3.3 [HIGH] Registry Maintainability — 2696 Lines, Three Parallel Match Statements

**Location:** `compute/registry.rs`

**Description:** Adding a new operation requires updating three separate match statements in the same file:
1. `variant_key()` — maps `PipelineOp` to `&'static str`
2. `register_all()` — registers the op with its implementations
3. `op_id()` — maps to `OpId` for GPU dispatch

Missing an arm in any one causes runtime panics. The file has 100+ near-identical `expect()` calls in a 400-line GPU registration block.

**Recommendation:** Implement a declarative macro that generates all three match arms from a single definition. Example:

```rust
define_op!(Crop, "Crop", gpu = "crop.wgsl", simd = false);
// Generates: variant_key arm + register_all entry + op_id arm
```

---

### 3.4 [MEDIUM] Image Enum Collapses Two Orthogonal Concerns

**Location:** `image.rs:46-68`

**Description:** The 5-variant `Image` enum mixes "where is the data?" (Loaded, Path, Bytes, Paletted) with "what processing?" (Pipeline). This creates:
- Deep `Arc<Pipeline { source: Arc<Pipeline { ... }> }>` nesting
- `push_op` cloning ops vectors at every level
- `Path`/`Bytes`/`Pipeline` all carrying redundant `format: Option<ImageFormat>`

**Recommendation:** Two-layer design:
```rust
enum DataSource { Loaded(DynamicImage), Paletted(PalettedData), Path { ... }, Bytes { ... } }
struct Image { source: Option<DataSource>, pipeline: Vec<PipelineOp>, ... }
```

---

### 3.5 [MEDIUM] Backend Abstraction Leaks

**Location:** `compute/registry.rs:28-56`, `compute/mod.rs:130`

**Description:**
- `OpEntry` stores `gpu_shader` and `gpu_source` for every op, even CPU-only ones
- GPU WGSL shaders are embedded via `include_str!()` macros in the central registry, not auto-discovered
- `route()` only does all-or-nothing backend selection — no mixed CPU/GPU pipelines
- No size threshold: GPU is always preferred (priority 100), even for tiny images where CPU is faster

**Recommendation:** Split `OpEntry` into per-backend data maps. Add a size threshold for GPU routing (~256×256 minimum). Consider mixed-backend execution for pipelines.

---

### 3.6 [MEDIUM] GPU `supports()` Checks Registration, Not Compilation

**Location:** `registry.rs:296-300`, `pool_gpu/mod.rs:248-258`

**Description:** `gpu_supports()` returns `true` if a shader was registered, but shaders that fail WGSL validation are silently skipped during pipeline compilation. A GPU-backed op might claim support but crash at runtime because the shader was never compiled.

**Recommendation:** `supports()` should check the live `GpuInner::pipelines` HashMap, not the static registry.

---

### 3.7 [MEDIUM] `apply_transparency` Bypasses Lazy Pipeline

**Location:** `image.rs:970-1000`

**Description:** Unlike other mutating ops (`putpixel`, `putdata`, `putalpha`), `apply_transparency` immediately materializes to RGBA in-place instead of pushing a pipeline op. This breaks consistency — callers with pending pipeline ops before `apply_transparency` may lose them.

**Recommendation:** Add `PipelineOp::ApplyTransparency` variant and use the standard pipeline mechanism.

---

### 3.8 [LOW] Global Mutex on Every `route()` Call

**Location:** `compute/mod.rs:78, 130-134`

**Description:** `ACTIVE: OnceLock<Mutex<HashSet<Backend>>>` is acquired on every pipeline execution. With only 3 backends (Cpu, Simd, Gpu), this could be a single `AtomicU8` bitmask.

**Recommendation:** Replace with `AtomicU8` where each bit represents one backend's enabled state.

---

### 3.9 [LOW] Inconsistent Ownership in Cross-Image Ops

**Location:** `pipeline.rs:197 vs 242`, `registry.rs:1846`

**Description:** `Blend` uses `Arc<Image>` for `other`; `Merge` uses owned `Vec<Image>` for bands. The `Merge` registration then clones each band just to wrap in `Arc`. This is inconsistent and wasteful.

**Recommendation:** Standardize on `Arc<Image>` for all cross-image references in `PipelineOp` variants.

---

### 3.10 [MEDIUM] Format Handling Scattered Across 4+ Locations

**Location:** `image.rs:321,341,1540,1555`, `format.rs:10`, `image.rs:856`

**Description:** Adding a new image format requires modifying:
- Format string parsing (`format.rs`)
- Magic byte detection (`image.rs:detect_format_from_magic`)
- Format-to-mode mapping (`image.rs:detect_format_mode`)
- PNG special-casing in `open`/`open_bytes`/`decode_paletted_png_reader`

The `formats/` directory exists but is empty — format modules were planned but never implemented.

**Recommendation:** Implement a `FormatHandler` trait with a global `FormatRegistry`. New formats should self-register via trait implementations.

---

## 4. Python Binding Issues

### 4.1 [HIGH] Binding Layer Contains Business Logic (CLAUDE.md Violations)

**Location:** Multiple Python files

**Violations of rule:** *"All logic in `pillow-rs/src/`; bindings delegate via `_core.xxx()`"*

| File | Lines | Violation |
|------|-------|-----------|
| `image.py:495-517` | 23 | `_align8to32` — full 8-to-32-bit alignment algorithm in Python |
| `image.py:519-558` | 40 | `toqimage()` — complete Qt image conversion logic |
| `image.py:565-598` | 34 | `frombytes()` — class-vs-instance detection logic |
| `image.py:658-680` | 23 | `transform()` — mesh data flattening |
| `operations.py:81-125` | 45 | `fromarray()` — numpy detection, array interface resolution, list flattening |
| `imagedraw.py:153-180` | 28 | `shape()` — outline validation and dispatching |
| `imagefont.py:288-293` | 6 | `TransposedFont.getbbox` — width/height calculation |

**Recommendation:** Move all logic into Rust core. Each Python method should be a single delegation call to `_core.xxx()` or `_rust_image.xxx()`.

---

### 4.2 [HIGH] For Loops and Arithmetic in Python Binding Layer

**Location:** `image.py:213,368,397,512,542,543,641`, `imagefont.py:28`, `operations.py:89-124`

**Violation of rules:**
- *"NO `for`/`while` loops, list comprehensions"*
- *"NO arithmetic (`+`, `-`, `*`, `/`, `min`, `max`, `sorted`, `sum`)"*

Examples:
- `image.py:213` — generator expression `tuple(Image(band) for band in self._rust_image.split())`
- `image.py:512` — explicit `for i in range(rows):` loop
- `image.py:504-511` — `*`, `//`, `+`, `%` arithmetic for BMP row alignment
- `imagefont.py:28` — `len(str(text)) * 6`

**Recommendation:** These must move to core. Examples:
- `split()` should return `Vec<Image>` directly from core (already does, but wrapper re-wraps)
- BMP alignment logic should be a core function `align_row_to_32(width: u32, mode: &str) -> (usize, usize)`
- Font text length computation should be `_core.text_width(text)` in Rust

---

### 4.3 [HIGH] File Size Exceeds "~200 lines max"

**Location:**
| File | Lines | Over by |
|------|-------|---------|
| `pillow-rs-py/src/lib.rs` | 2,526 | 12.6× |
| `pillow-rs-py/python/pillow_rs/image.py` | 743 | 3.7× |
| `pillow-rs-py/python/pillow_rs/imagefont.py` | 338 | 1.7× |
| `pillow-rs-py/python/pillow_rs/imagefilter.py` | 234 | 1.2× |

**Recommendation:** For `lib.rs`: split by module (e.g., `image_methods.rs`, `draw_methods.rs`, `font_methods.rs`, `palette_methods.rs`). Each should stay ~200 lines. For Python files: strip logic to core as noted above; aim for ~200 lines of pure delegation.

---

### 4.4 [MEDIUM] Leaky PIL Dependency

**Location:** `imageops.py:69`, `imagefont.py:78-84`

**Description:**
- `imageops.py:69`: `from PIL.ImageColor import getrgb` — a PIL replacement importing PIL
- `imagefont.py:78-84`: `from PIL import ImageFont as PILFreeType` — creates actual PIL font instances for pixel-identical rendering

This creates a circular dependency where the PIL replacement silently falls back to PIL.

**Recommendation:** Move color parsing to Rust core (it already exists in `color.rs:parse_color_str`). For fonts, implement full FreeType parsing or document the limitation.

---

### 4.5 [LOW] Duplicate Function Definition

**Location:** `imagechops.py:81-86`

```python
def offset(image, xoffset, yoffset=None):  # line 81 — dead, immediately overwritten
    raise NotImplementedError(...)
def offset(image, xoffset, yoffset=None):  # line 86 — real implementation
```

The first definition is dead code. Remove it.

---

### 4.6 [LOW] Inconsistent Type Annotation Style

**Location:** Across Python files

Some files use `from typing import Optional` (old style), others use `X | None` (PEP 604). Pick one.

---

### 4.7 [MEDIUM] lib.rs Logic That Belongs in Core

**Location:** `pillow-rs-py/src/lib.rs`

Specific functions containing logic that should be in `pillow-rs/src/`:
- `tobytes_formatted_swap()` (lines 229-245) — byte-swapping loop
- `regular_polygon()` (lines 1246-1307) — full polygon vertex computation with trigonometry
- `multiline_textbbox()` (lines 1546-1593) — full multiline text layout algorithm
- `getextrema_formatted()` etc. — mode-based formatting/dispatch

**Recommendation:** Move implementations to `pillow-rs/src/ops/` and call them via single-line delegations.

---

## 5. Dependency & Supply Chain Issues

### 5.1 [RESOLVED] Historical Duplicate `png` Versions

**Description:** This applied while the historical `pillow-rs-image` crate
lived in this repository. Codec ownership has moved to the sibling
`image-slash-star` package and the `pillow-rs-image` directory has been
removed.

**Recommendation:** Keep codec version alignment in `image-slash-star`; do not
restore a downstream `pillow-rs-image` codec crate in this repo.

---

### 5.2 [HIGH] `wgpu` is an Unconditional Dependency

**Description:** `wgpu = "24"` is in `pillow-rs/Cargo.toml` with no feature gate. This means:
- ~70 additional crate dependencies always compiled
- WASM builds pull in Vulkan, Metal, DirectX, EGL native bindings
- Android NDK, Apple ObjC runtime dependencies included

**Recommendation:** Put wgpu behind `features = ["gpu"]`. Default features can include `gpu`, but WASM and CPU-only builds should be able to opt out.

---

### 5.3 [RESOLVED] Historical `serde_yaml` in Removed Dev-Dependencies

**Location:** removed `pillow-rs-image/Cargo.toml` (dev-dependencies)

**Description:** This applied only to the historical `pillow-rs-image` crate,
which has been removed from this repository.

**Recommendation:** No downstream action remains in this repo.

---

### 5.4 [MODERATE] `thiserror` v1 + v2 Both Compiled

**Description:** `pillow-rs` uses `thiserror = "1"`; `wgpu` dependencies use `thiserror = "2"`. Both versions are compiled.

**Recommendation:** Bump `pillow-rs` to `thiserror = "2"` (API-compatible, same derive macros).

---

### 5.5 [MODERATE] Missing Crate Metadata

**Location:** All `Cargo.toml` files

None of the remaining crates set: `repository`, `homepage`, `documentation`,
`authors`, `keywords`, `categories`, or `readme`.

**Recommendation:** Add workspace-level metadata fields, especially `repository` and `documentation` for crates.io discoverability potential.

---

### 5.6 [LOW] `pillow-rs-js` Profile Ignored

**Location:** `pillow-rs-js/Cargo.toml`

**Description:** Cargo warns: `profiles for the non root package will be ignored`. The `[profile.release]` in `pillow-rs-js/Cargo.toml` is silently ignored.

**Recommendation:** Merge into `[workspace.profile.release]` in root `Cargo.toml`, or use a package-specific override.

---

### 5.7 [INFO] No CVE Scanning in CI

**Description:** No `cargo-audit` or `cargo-deny` in CI scripts. The `lint.sh` runs fmt + clippy + tests + "trust report" but doesn't scan for known vulnerabilities.

**Recommendation:** Add `cargo audit` to `scripts/lint.sh` and `scripts/ci_coverage.sh`.

---

## 6. Testing & Quality Issues

### 6.1 [HIGH] 23 Implemented Operations Have Zero Fixtures

**Description:** The manifest declares these as `status: implemented` but no test fixtures exist:

- **ImageFont:** 16 methods — `FreeTypeFont.{getbbox, getlength, getmask, getmask2, getmetrics, getname, font_variant, ...}`, `ImageFont.{getbbox, getlength, getmask}`, `TransposedFont.{getbbox, getlength, getmask}`
- **Image properties:** `size`, `width`, `height`, `mode`, `format`, `info`
- **Image operations:** `has_transparency_data`, `is_animated`, `n_frames`, `palette`

**Impact:** 12.6% of the "100% CPU implementation" has no test coverage at all.

**Recommendation:** Generate fixtures for all 23 missing operations. Prioritize font rendering operations as they are the most complex.

---

### 6.2 [HIGH] 41 Operations Have Empty `edge_cases: []`

**Description:** Operations like `Image.transpose`, `Image.quantize`, `Image.transform`, `Image.point`, `Image.reduce` have no edge case tests defined in the manifest.

**Recommendation:** Add edge case entries for at minimum: zero-dimension images, single-pixel images, extreme parameter values, and mode-specific corner cases.

---

### 6.3 [MEDIUM] No XFAIL Mechanism

**Description:** There is no mechanism for "expected to fail." Operations are either `implemented` (must pass all tests) or `ignored` (no tests). If an operation works for some modes but not others, you can't mark specific mode cases as xfail — you'd have to remove the fixture case entirely.

**Recommendation:** Add an `xfail` annotation mechanism to the test engine, or a `xfailed_tracker.txt` that the test runner reads to skip known-failing cases.

---

### 6.4 [MEDIUM] Temp File Leak in 34 Test Cases

**Location:** `engine.py:338-354`

**Description:** `_file_open` creates temporary files for `Image.open`/`Image.save` tests and explicitly does not delete them ("RSPIL may lazily load image data"). Each test run leaves orphaned temp files.

**Recommendation:** Register temp files for cleanup in a pytest fixture `teardown` or use `tempfile.TemporaryDirectory` with a known lifetime.

---

### 6.5 [MEDIUM] Benchmark Code Embedded in Coverage Report

**Location:** `scripts/coverage/compute_coverage.py:498-541`

**Description:** The coverage report generation includes a `run_benchmarks()` function that creates 2000×2000 images and runs operations in a loop. This mixes benchmarking with coverage, making coverage reports slow.

**Recommendation:** Extract benchmark code to `scripts/bench/`. Coverage report should only compute coverage.

---

### 6.6 [MEDIUM] Linux-Only Fixture Generation

**Location:** `scripts/generate_fixtures.py:105-112`

**Description:** `ctypes.CDLL('libc.so.6')` is used for deterministic random seeding. This is Linux-only. On macOS, the library is `libc.dylib`. On Windows, it doesn't exist.

**Recommendation:** Use a platform-independent RNG seeded with a fixed value, or use Python's `random` module with a fixed seed.

---

### 6.7 [MEDIUM] Suite Filtering via Fragile Substring Match

**Location:** `scripts/build_and_test.sh:15-22`

**Description:** Test suites are filtered via `-k "suite${SUITE}"` substring matching. If any test case ID accidentally contains "suite1" as a substring (e.g., "suite10"), it gets incorrectly included.

**Recommendation:** Use pytest markers (`@pytest.mark.suite1`) instead of substring matching on test IDs.

---

### 6.8 [MEDIUM] `Image.new` Fixture for Ignored Operation

**Description:** `tests/fixtures/input/jsons/Image.new.json` exists, but `Image.new` (class method) is `status: ignored` in the manifest. In `--strict-covers` mode, this causes collection failure.

**Recommendation:** Either remove the fixture or change the manifest status to `implemented`.

---

### 6.9 [LOW] No Individual Test Debug Mode

**Description:** When a test fails, you must manually parse fixture JSONs and rerun `generate_fixtures.py` to compare. No `--update-fixtures` or per-fixture debug output.

**Recommendation:** Add `--fixture-debug` flag that dumps the actual vs expected values for the failing case.

---

### 6.10 [INFO] Worktree Coverage Scripts Drifted from Main

**Description:** Worktrees under `.claude/worktrees/` contain coverage scripts (`generate_coverage_page.py`, `generate_fixture_tests.py`, etc.) not present on `main`. These features were developed in isolation but never merged.

**Recommendation:** Audit worktree scripts and merge the valuable ones to `main`.

---

## 7. Maintainability Issues

### 7.1 [HIGH] `raw_bytes_to_image` Triplicated

**Location:** `image.rs:1646`, `geometry.rs:100`, `filter.rs:30`

**Description:** The same 28-line function exists in three files. A bug fix would need to be applied three times; they will inevitably drift.

**Recommendation:** Extract to `pillow-rs/src/utils.rs` and import from all three locations.

---

### 7.2 [HIGH] 95% of Core Code Has No Unit Tests

**Description:** Only 2 of 30+ Rust source files contain `#[test]` annotations:
- `ops/imageops.rs` — EXIF orientation parsing only
- `compute/pool_simd/ops/scalar.rs` — SIMD scalar ops only

**Completely untested:** `image.rs` (1711 lines), `draw/mod.rs` (1447 lines), `ops/quantize.rs` (1706 lines), `ops/pil_resize.rs` (809 lines), `compute/registry.rs` (2696 lines), all `compute/pool_cpu/ops/` files, `color.rs`, `pipeline.rs`, `font/mod.rs`.

**Recommendation:** Add Rust-level unit tests for at minimum:
- `raw_bytes_to_image` / image buffer management
- Core pixel arithmetic functions
- Pipeline op construction and materialization
- Key quantize functions (median cut, lookup cube)

---

### 7.3 [MEDIUM] Magic Mode Numbers Throughout SIMD Code

**Location:** `compute/pool_simd/ops/scalar.rs`

**Description:** The mode encoding `0=L, 1=LA, 2=RGB, 3=RGBA` is used as bare integers in ~80+ locations: `mode >= 2`, `mode == 1 || mode == 3`, `mode * 256`. No named constants.

**Recommendation:** Define a `#[repr(u32)] enum PixelFormat { L = 0, LA = 1, RGB = 2, RGBA = 3 }` with `From<ColorMode>` and use it throughout.

---

### 7.4 [MEDIUM] 30+ Public API Functions Without Doc Comments

**Location:** `compute/mod.rs`, `compute/registry.rs`, `color.rs`, `pipeline.rs`

**Description:** Functions like `route()`, `execute_batch()`, `enable_backend()`, `extract_params()`, `parse_color_str()`, and types like `ResampleFilter`, `TransposeMethod`, `TransformMethod`, `ColorMode`, `DitherMethod` have no doc comments.

**Recommendation:** Add `///` doc comments to every public API item. This is essential for an open-source project.

---

### 7.5 [MEDIUM] `String` Error Type in Palette Functions

**Location:** `color.rs:188-221, 674-700`

**Description:** `palette_getcolor_append`, `palette_getcolor_validate`, and `palette_save_to_file` return `Result<_, String>` instead of `Result<_, PilError>`. Inconsistent with the rest of the codebase.

**Recommendation:** Use `PilError` variants for all errors. Add `PaletteError(String)` variant if needed.

---

### 7.6 [MEDIUM] `PilError::AssertionError` is Dead

**Location:** `error.rs:12`

**Description:** The `AssertionError` variant is only used at `ops/imageops.rs:95` with `String::new()`. It carries zero context.

**Recommendation:** Either add meaningful assertion errors (with file/line context) or remove the variant.

---

### 7.7 [LOW] Unused Dead Code

**Location:**
- `image.rs:1538-1552` — `#[allow(dead_code)] detect_format_from_magic`
- `geometry.rs:480-576` — `#[allow(dead_code)] transform_affine_generic` (96 lines)
- `bitmap_font/data.rs:855` — `#[allow(dead_code)] BITMAP_ADVANCE_WIDTHS`
- `image.rs:1698` — orphaned `#[allow(dead_code)]` annotation

**Recommendation:** Either use these functions or remove them.

---

### 7.8 [LOW] `color.rs` Unused Assignment

**Location:** `color.rs:226`

```rust
let _palette_len = palette.len();  // computed, never used
```

Remove this line.

---

### 7.9 [LOW] File Naming Collisions

**Description:** `ops/imageops.rs` (API layer) and `compute/pool_cpu/ops/imageops.rs` (CPU implementation) have identical names. Same for `ops/chops.rs` and `compute/pool_cpu/ops/chops.rs`.

**Recommendation:** Rename implementation files: `compute/pool_cpu/ops/impl_imageops.rs`, `compute/pool_cpu/ops/impl_chops.rs`.

---

### 7.10 [INFO] Zero TODO/FIXME/HACK/XXX Markers

**Finding:** A grep across the entire codebase returned zero results. Either the codebase is uniquely disciplined, or tech debt is tracked elsewhere. If the latter, document where in CLAUDE.md.

---

## 8. Summary Action Plan

### Immediate (Security — Blockers for Production)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 1 | Integer overflow in `(w * h) as usize` — add `checked_pixel_count()` helper | 1 day | Prevents OOB memory access |
| 2 | Replace all `unwrap()`/`expect()` on `from_raw()` with proper errors | 2 hours | Prevents process crashes |
| 3 | Add `MAX_PIXELS` limit at all allocation entry points | 1 day | Prevents allocation DoS |
| 4 | Clamp draw `width` and mesh bounding boxes | 2 hours | Prevents CPU DoS |
| 5 | Add epsilon guard in Box-Muller noise | 30 min | Prevents NaN/Inf propagation |

### Short-Term (Performance — Biggest Wins)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 6 | Add rayon for parallel row processing | 2 days | 4-16× speedup on large images |
| 7 | Convert box blur to sliding-window accumulator | 1 day | O(radius)→O(1) speedup |
| 8 | Convert `resize_f` to separable convolution | 1 day | 6× Lanczos speedup |
| 9 | Eliminate rank filter per-pixel Vec allocations | 4 hours | Eliminates 1M allocations |
| 10 | Fix rounded rect N× image clones | 3 hours | 7× draw speedup |
| 11 | Unify ellipse Bresenham walk | 2 hours | 2× ellipse draw speedup |

### Medium-Term (Architecture — Structural Health)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 12 | Fix pipeline mode tracking (accumulated vs root) | 1 day | Correctness bug fix |
| 13 | Fix `materialize_indices` double-apply | 1 day | Correctness bug fix |
| 14 | Implement declarative op registration macro | 2 days | Eliminates ~2000 lines of boilerplate |
| 15 | Add `FormatHandler` trait and registry | 3 days | Clean format extensibility |
| 16 | Restructure Image enum (two-layer design) | 3 days | Cleaner ownership, no deep Arc nesting |

### Short-Term (Python Bindings — CLAUDE.md Compliance)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 17 | Move `_align8to32`, `toqimage`, `fromarray`, `transform` logic to core | 2 days | Binding layer truly thin |
| 18 | Remove all for loops and arithmetic from Python files | 1 day | CLAUDE.md compliance |
| 19 | Split lib.rs into per-module files | 1 day | Files ~200 lines each |
| 20 | Remove duplicate `offset()` and leaky PIL imports | 1 hour | Clean dependencies |

### Short-Term (Dependencies)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 21 | Align `png` version across workspace | 2 hours | Remove duplicate compilation |
| 22 | Put `wgpu` behind `features = ["gpu"]` | 1 hour | Remove 70+ deps from WASM build |
| 23 | Replace deprecated `serde_yaml` | 30 min | Modern dependency |
| 24 | Bump `thiserror` to v2 | 30 min | Remove duplicate compilation |
| 25 | Add `cargo-audit` to CI | 1 hour | Vulnerability scanning |

### Short-Term (Testing)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 26 | Generate fixtures for 23 missing operations | 2 days | 100% coverage |
| 27 | Add edge case tests for 41 operations | 1 day | Robustness |
| 28 | Add Rust unit tests for `image.rs`, `quantize.rs`, `pil_resize.rs` | 3 days | Core correctness |
| 29 | Fix temp file leak and `Image.new` fixture | 2 hours | CI hygiene |
| 30 | Add xfail mechanism | 4 hours | Partial implementation tracking |

### Long-Term (Polish)

| # | Issue | Effort | Impact |
|---|-------|--------|--------|
| 31 | Add doc comments to all public APIs | 2 days | OSS usability |
| 32 | Extract triplicated `raw_bytes_to_image` | 1 hour | DRY |
| 33 | Add named constants for SIMD mode encoding | 2 hours | Readability |
| 34 | Replace global `Mutex` with `AtomicU8` for backends | 1 hour | Lock-free routing |
| 35 | Fix `PilError::AssertionError` (use or remove) | 30 min | Clean error types |
| 36 | Remove dead code items | 1 hour | Clean codebase |
| 37 | Add crate metadata (repository, authors, etc.) | 30 min | crates.io readiness |
| 38 | Merge worktree coverage scripts to main | 1 hour | Feature consolidation |
| 39 | Extract benchmark code from coverage report | 1 hour | Separation of concerns |
| 40 | Replace `-k` substring filtering with pytest markers | 2 hours | Test reliability |

---

### Total Estimated Effort

| Priority | Tasks | Estimated Effort |
|----------|-------|-----------------|
| Immediate (Security) | #1–5 | 3 days |
| Short-Term (Perf + Bindings + Deps + Tests) | #6–11, #17–30 | 18 days |
| Medium-Term (Architecture) | #12–16 | 10 days |
| Long-Term (Polish) | #31–40 | 8 days |
| **Total** | **40 tasks** | **~39 days** |

---

*Generated by comprehensive codebase audit. Focus areas: security, performance, maintainability for open-source readiness. SIMD and GPU backends excluded as noted.*

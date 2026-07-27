# ImageFont final gap action report

Date: 2026-07-27

Purpose: decision-grade list of what is still missing before claiming full
Pillow 12.2.0 `PIL.ImageFont` parity in Rust.

This file answers two questions:

1. What uncovered lines/regions mean in terms of actual logic.
2. What Pillow `ImageFont` behavior is missing, wrong, or under-proven in Rust
   across the files that implement Font behavior.

## Evidence boundary

- Repo: `/Users/lazytrot/work/pillow-rs`
- Branch reviewed: `main`
- Last trusted measured commit:
  `21086af6f5fff5921b554e3b6fe76d6613b5874d`
- Last trusted Coverage MCP command:
  `font-tests-coverage-with-freetype-pillow-12-2`
- Last trusted Coverage MCP run:
  `126a382e-f67f-4f04-9422-6033145acceb`
- Last trusted Coverage MCP snapshot:
  `e67116f1-f510-46ba-80a0-23768d214d3a`
- Coverage suite: `font-with-freetype`
- Oracle: repo-local `.oracle-venv/bin/python`
- Oracle Pillow version: `12.2.0`
- Pillow source inspected:
  `.oracle-venv/lib/python3.12/site-packages/PIL/ImageFont.py`
- Rust sources inspected:
  - `pillow-rs/src/lib.rs`
  - `pillow-rs/src/font/mod.rs`
  - `pillow-rs/src/font/imagingft.rs`
  - `pillow-rs/src/font/pilfont.rs`
  - `pillow-rs-py/python/pillow_rs/imagefont.py`
  - lower `pillow-rs-freetype/src/**` files shown by Coverage MCP
- Active fixture root:
  `pillow-rs/tests/fixtures/font/inputs/public-api`
- Manifest:
  `pillow-rs/tests/fixtures/font/font_manifest.yaml`
- Harness:
  `pillow-rs/tests/font_public_api.rs`

Trusted current claim:

> The last trusted active `PIL.ImageFont` fixture corpus has exact runtime
> parity against Pillow 12.2.0 for the rows it exercises.

Not trusted:

> Full `PIL.ImageFont` parity is complete.

The second claim is not defensible because direct `imagingft.rs` region gaps
remain and the lower pure-Rust FreeType implementation still has large uncovered
areas that can affect public `ImageFont` output.

## Current coverage status

Coverage MCP snapshot: `e67116f1-f510-46ba-80a0-23768d214d3a`.

Direct Font files:

| File | Lines | Branches | Functions | Regions | Decision |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | Public Rust Font adapter methods are reached by active rows. |
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | Default Aileron path is covered. |
| `pillow-rs/src/font/imagingft.rs` | 1686/1712 98.48% | 252/262 96.18% | 165/176 93.75% | 2618/2718 96.32% | Real FreeType-backed gaps remain. |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | Bitmap `ImageFont.ImageFont` is under-proven by function/region coverage. |

The full workspace suite totals are much lower because the Font coverage
artifact includes non-Font workspace files. For ImageFont decisions, use the
file-specific Font and lower FreeType coverage below.

## 1. Uncovered-line logic-based analysis

### `pillow-rs/src/font/imagingft.rs`

Coverage MCP reports 17 relevant direct gaps in the trusted snapshot:

- 7 uncovered lines.
- 10 partial-branch lines.

| Line(s) | Coverage reason | Rust logic | Pillow behavior | Decision / action |
|---:|---|---|---|---|
| `91`, `92` | partial/uncovered | FreeType error-table declaration and unknown error fallback. | Pillow `_imagingft.c::geterror` maps FreeType errors to `OSError`; table misses use `unknown freetype error`. | Do not add private table tests as parity proof. Add only a real public ImageFont row if Pillow itself exposes this fallback from an input font. |
| `253` | uncovered | Static table row for `FT_Err_Too_Many_Instruction_Defs`. | Pillow raises `OSError("too many instruction definitions")`. | Behavior is already proven by public row `font.getlength.hinter_too_many_instruction_defs`; LLVM still marks static table data as uncovered. Keep as artifact, not duplicate fixture noise. |
| `271` | uncovered | Static table row for `FT_Err_Invalid_Horiz_Metrics`. | Pillow can expose `OSError("invalid horizontal metrics")` from malformed metrics data. | Real remaining error-classification gap. Add a malformed font row only if Pillow 12.2.0 naturally returns this exact error. |
| `796` | partial branch | Constant/declaration region around kerning defaults. | No independent Pillow behavior. | Treat as LLVM segment-normalization artifact unless source-context later proves a reachable public branch. |
| `826`, `829` | partial branch | `floor26` / `ceil26` fixed-point conversion helpers. | Pillow C uses fixed-point floor/ceil/rounding for bbox, anchor, mask size, and offsets. | Add independent public rows with negative bearings, fractional `start`, descenders/ascenders, anchor extremes, and glyphs crossing pixel boundaries. Do not add duplicate `AV` rows just to move counters. |
| `928` | partial branch | `bbox_from_run_with_flags(..., load_flags)` route marker. | Pillow fallback layout changes load flags for BASIC mono paths and applies kerning after the first glyph. | Existing BASIC/mono/kerning rows prove visible behavior, but marker remains. Recheck after stroke and rounding rows; do not chase with duplicate rows. |
| `1116`, `1119`, `1121` | partial/uncovered | Stroked bitmap extent clamp against bbox-derived target. | Pillow allocates through `_imagingft.c::bounding_box_and_anchors` and clips writes during render. It does not obviously use this Rust-style post-stroke extent mutation. | Suspect Rust-only compatibility shim. After lower stroker parity is fixed, remove it or prove it with a Pillow/C first-divergence trace. |
| `1215`, `1216` | partial/uncovered | `stroke_filled=true` branch to `FT_Outline_Glyph_StrokeBorder`. | Pillow supports `FreeTypeFont.getmask2(..., stroke_filled=True)` through FreeType border stroking. | Real implementation gap. Rust has the option wired, but lower stroke-border success is incomplete. Add successful public rows only after the lower implementation is real. |
| `1281`, `1282` | partial/uncovered | Defensive short-read guard in BGRA embedded-bitmap pixel extraction. | Pillow consumes embedded bitmap data through FreeType; malformed/truncated color bitmap behavior can surface as errors or clipped pixels. | Add only real SBIT/color-bitmap malformed or edge fonts if Pillow exposes this path. Do not unit-test the guard as parity. |
| `1299`, `1300` | partial/uncovered | Alpha-zero branch in premultiplied BGRA-to-coverage conversion. | Pillow's final mask coverage for BGRA embedded bitmaps depends on FreeType color glyph bitmap bytes. | Add a public SBIT/CBDT/CBLC or color-bitmap row with fully transparent pixels if available. Current SBIT rows prove non-zero BGRA conversion but not alpha-zero. |

### `pillow-rs/src/font/pilfont.rs`

Bitmap `ImageFont.ImageFont` active rows pass, but function/region coverage is
not complete. The important under-proven logic areas are:

- direct already-opened image constructor path `from_pilfont_data`;
- PILfont descriptor parsing variants;
- PNG/GIF/PBM glyph image loading in `open_pilfont_glyph_image`;
- PBM `P1` and `P4` parsing details;
- malformed descriptor table sizes;
- truncated glyph raster behavior;
- invalid glyph image modes;
- missing glyph and descriptor source-rectangle clipping variants;
- max string length behavior.

Action:

- Add only public Pillow bitmap `ImageFont.load`, `ImageFont.getbbox`,
  `ImageFont.getlength`, `ImageFont.getmask`, and `load_default_imagefont` rows
  that execute distinct behavior.
- Do not count private loader unit tests as ImageFont parity.

### Lower pure-Rust FreeType coverage that still affects ImageFont

`imagingft.rs` is only the adapter. FreeType-backed `PIL.ImageFont` parity
depends on lower font loading, charmap, glyph selection, hinting, variation,
embedded bitmap, stroker, and rasterizer code.

High-risk lower files from the trusted snapshot:

| File | Lines | Branches | Functions | Regions | ImageFont risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 1107/8186 13.52% | 77/2075 3.71% | 94/586 16.04% | 1442/11495 12.54% | Very high: handles, face/glyph wrappers, charmap, variation, stroker, bitmap routes. |
| `pillow-rs-freetype/src/api.rs` | 263/1186 22.18% | 37/294 12.59% | 28/105 26.67% | 327/1737 18.83% | High: lower public font API feeding ImageFont. |
| `pillow-rs-freetype/src/font.rs` | 1298/4747 27.34% | 166/702 23.65% | 127/392 32.40% | 1794/6728 26.66% | High: load, names, metrics, glyph machinery. |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | High: rendered mask byte parity. |
| `pillow-rs-freetype/src/scaler.rs` | 806/1342 60.06% | 114/186 61.29% | 40/66 60.61% | 918/1436 63.93% | Medium/high: scaling and hinted metrics. |
| `pillow-rs-freetype/src/grays.rs` | 571/827 69.04% | 122/190 64.21% | 25/35 71.43% | 854/1106 77.22% | Medium/high: antialias rasterizer output. |
| `pillow-rs-freetype/src/tt/sbit.rs` | 254/814 31.20% | 21/72 29.17% | 19/108 17.59% | 375/1269 29.55% | High: embedded bitmap/color glyph formats and malformed paths. |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | High: Unicode/private-use/symbol encoding behavior. |
| `pillow-rs-freetype/src/tt/glyf.rs` | 174/545 31.93% | 34/96 35.42% | 8/20 40.00% | 219/694 31.56% | High: TrueType outlines. |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | High: CFF/OpenType outlines. |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 725/1493 48.56% | 148/480 30.83% | 32/48 66.67% | 1298/3107 41.78% | High: hinted TrueType and bytecode error classification. |
| `pillow-rs-freetype/src/autohint/latin.rs` | 1988/2962 67.12% | 673/1263 53.29% | 45/67 67.16% | 2806/4283 65.51% | Medium/high: Latin hinted glyphs. |
| `pillow-rs-freetype/src/autohint/cjk.rs` | 396/879 45.05% | 130/398 32.66% | 11/18 61.11% | 531/1180 45.00% | High for CJK fonts. |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 26/42 61.90% | 6/12 50.00% | 1/2 50.00% | 44/67 65.67% | Partially proven; malformed/device metric paths remain. |
| `pillow-rs-freetype/src/tt/mvar.rs` | 58/67 86.57% | 3/6 50.00% | 4/7 57.14% | 92/113 81.42% | Partially proven; malformed/value-tag paths remain. |
| `pillow-rs-freetype/src/tt/vhea.rs` | 8/11 72.73% | 1/2 50.00% | 1/1 100.00% | 8/9 88.89% | Partially proven; short/error path remains. |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 28/50 56.00% | 3/8 37.50% | 1/2 50.00% | 44/65 67.69% | Partially proven; malformed/overflow paths remain. |

Action:

- Do not treat high `imagingft.rs` line coverage as enough.
- A lower file is trusted only when a public Pillow `ImageFont` row reaches it
  and Rust exactly matches Pillow output or structured error payload.

## 2. Pillow ImageFont vs Rust implementation comparison

### Pillow 12.2.0 public surface

Pillow source exposes:

- module constants/enums: `MAX_STRING_LENGTH`, `Layout.BASIC`, `Layout.RAQM`;
- module functions: `load`, `load_path`, `truetype`, `load_default`,
  `load_default_imagefont`;
- bitmap class `ImageFont.ImageFont`: `getbbox`, `getlength`, `getmask`, loaded
  `info` state;
- FreeType class `ImageFont.FreeTypeFont`: constructor, `getname`,
  `getmetrics`, `getlength`, `getbbox`, `getmask`, `getmask2`, `font_variant`,
  `get_variation_names`, `set_variation_by_name`, `get_variation_axes`,
  `set_variation_by_axes`, `__getstate__`, `__setstate__`;
- wrapper class `ImageFont.TransposedFont`: `getmask`, `getbbox`, `getlength`.

Rust intentionally keeps filesystem/path/stream handling out of core. Core
accepts bytes and options; bindings/tests own host I/O and object conversion.
That architecture is correct if bindings remain thin.

### Surface comparison matrix

| Pillow behavior | Rust files | Current status | Missing / wrong / decision |
|---|---|---|---|
| `MAX_STRING_LENGTH` | `imagingft.rs`, `pilfont.rs` | Implemented as 1,000,000 character/byte guards. | Need explicit public rows for boundary and over-limit in both bitmap and FreeType paths if not already distinct. |
| `Layout.BASIC` | `mod.rs`, `imagingft.rs` | Implemented. | Active BASIC rows pass. Keep behavior in Rust core. |
| `Layout.RAQM` success | `mod.rs`, `imagingft.rs`, `error.rs` | Not implemented. Current no-raqm error parity is covered. | Full Pillow `ImageFont` parity cannot be claimed unless successful libraqm shaping is implemented or explicitly out of scope. |
| `load(filename)` bitmap font | `pilfont.rs`, test/binding loader | Implemented for active rows. | More bitmap descriptor/raster/error rows needed for function/region trust. |
| `load_path(filename)` | binding/test loader | Represented by fixture operation. | Search-path behavior is not core-owned. Keep thin binding/test wrapper only. |
| `load_default_imagefont()` | `pilfont.rs` embedded bitmap font | Active row passes. | Add malformed/default edge rows only if product scope requires them. |
| `load_default(size=None)` | `default_aileron.rs`, `mod.rs`, `imagingft.rs` | Covered through embedded Aileron FreeType path. | Keep pinned to Pillow 12.2.0 default font behavior. |
| `truetype(font, size, index, encoding, layout_engine)` | `lib.rs`, `mod.rs`, `imagingft.rs`, lower `fontdone` | Rust core has byte constructors and options. | Path/stream object behavior belongs in bindings. Encoding/charmap edge coverage remains weak. |
| Bitmap `ImageFont.getbbox` | `pilfont.rs` | Active rows pass. | Add edge rows for missing glyphs, descriptor clipping, multiline/no-op args if Pillow behavior matters. |
| Bitmap `ImageFont.getlength` | `pilfont.rs` | Active rows pass. | Add max-length and non-ASCII/bytes-like rows where Pillow behavior is distinct. |
| Bitmap `ImageFont.getmask` | `pilfont.rs` | Active rows pass. | Add PBM/PNG/GIF raster, invalid mode, truncated raster rows. |
| Bitmap `ImageFont.info` | `pilfont.rs` | Covered by rows. | No immediate gap unless API-shape parity requires exact Python metadata shape. |
| FreeType constructor | `ImageFont::from_bytes_with_options`, lower face loader | Active rows pass. | Path/stream behavior excluded from core. More malformed font classifications remain. |
| FreeType `__getstate__` / `__setstate__` | none | Missing. | Required only if Python class/state parity is in scope. Exclude explicitly if target is metrics/rendering behavior only. |
| `getname` | `imagingft.rs`, lower name table | Active rows pass. | More platform/missing-name rows only if coverage finds distinct branches. |
| `getmetrics` | `imagingft.rs`, lower metrics tables | Active rows pass for standard/fixed/fallback/vertical/mvar cases. | Malformed metrics and less-common table fallback paths remain. |
| `getlength` | `imagingft.rs::glyph_run`, lower charmap/hint/kerning | BASIC rows pass. | RAQM success missing; charmap/encoding, rounding, and bytecode edge paths remain. |
| `getbbox` | `imagingft.rs::bbox_from_glyph_run` | BASIC rows pass. | Negative bearings, anchors, fractional start, stroke, and bbox/stroker edge cases remain. |
| `getmask` | `imagingft.rs`, lower render/SBIT/stroker | Normal rows pass. | General stroke and additional embedded bitmap/color glyph paths remain under-proven. |
| `getmask2` | `imagingft.rs`, lower render/SBIT/stroker | Normal/start rows pass. | `stroke_filled=true` is wired but not proven; lower stroke-border is incomplete. |
| `font_variant` | `imagingft.rs`, lower face/variation reload | Active rows pass. | Add rows proving variation changes visible metrics/rendering, not only setter return shape. |
| `get_variation_names` | `imagingft.rs`, lower fvar/name | Active rows pass. | Missing-name and malformed variation table paths remain. |
| `set_variation_by_name` | `imagingft.rs`, lower variation instance | Active rows pass. | Add visible metric/render deltas for selected named instances. |
| `get_variation_axes` | `imagingft.rs`, lower fvar/mvar | Active rows pass. | Malformed/value-tag paths remain lower risk. |
| `set_variation_by_axes` | `imagingft.rs`, lower variation coords | Active rows pass. | Add visible metric/render deltas for axis changes. |
| `TransposedFont.__init__` | helper operations only | No Rust class shape. | Missing if exact class-shape parity is required. Behavior rows are covered through helpers. |
| `TransposedFont.getmask` | `get_transposed_mask` helper | Active rows pass. | Keep as helper unless public Rust `TransposedFont` is required. |
| `TransposedFont.getbbox` | `transposed_bbox` helper | Active rows pass. | Keep as helper unless public Rust `TransposedFont` is required. |
| `TransposedFont.getlength` | `validate_transposed_length` helper | Active rows pass. | Keep as helper unless public Rust `TransposedFont` is required. |

### Rust helper surfaces that must not become independent truth sources

These are not direct Pillow public endpoints:

- `getbbox_binary`
- `getmask2_with_start`
- `get_transposed_mask`
- `transposed_bbox`
- `validate_transposed_length`
- `text_bbox`
- `draw_text`
- `render_text`
- `render_text_binary`

Decision:

- Keep them only as harness/binding adapters that map to Pillow public behavior.
- Their expected output must always come from the runtime Pillow oracle.
- Do not store expected pixels, hashes, or error payloads in input JSON.

## Wrong or suspect Rust implementation areas

### 1. General stroke and stroke-border

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/handles.rs`

Status:

- Rust public options expose `stroke_width` and `stroke_filled`.
- `stroke_filled=false` routes to `FT_Outline_Glyph_Stroke`.
- `stroke_filled=true` routes to `FT_Outline_Glyph_StrokeBorder`.
- Lower `fontdone` stroker/border success is not generally implemented or
  trusted for real glyph outlines.

Decision:

- This is the highest-priority implementation gap.
- Implement real general stroker parse/segment/border export behavior.
- Do not add glyph-specific shortcuts.
- After implementation, add public Pillow rows for:
  - normal stroke;
  - `stroke_filled=true`;
  - mono stroke;
  - descenders/ascenders;
  - clipped sides;
  - independent glyphs beyond `A`.

### 2. Stroked extent clamp

File:

- `pillow-rs/src/font/imagingft.rs`

Status:

- Rust currently clips/mutates stroked bitmap extents after comparing actual
  rendered size with bbox-derived expected size.
- Pillow appears to allocate from `_imagingft.c::bounding_box_and_anchors` and
  clip writes, not mutate extents this way.

Decision:

- Treat this as suspect compatibility code.
- Do not build more behavior on top of it.
- After lower stroker parity is real, either remove this clamp or prove it with
  a first-divergence trace against Pillow/C.

### 3. Libraqm / advanced layout

Files:

- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs/src/error.rs`

Status:

- Current oracle environment has no libraqm.
- Rust maps direction/features/language to an unsupported-libraqm error and the
  public payload matches the no-libraqm Pillow oracle.

Decision:

- This is correct for the current no-raqm suite.
- It is not complete `PIL.ImageFont` parity.
- Successful libraqm shaping remains out of scope unless explicitly added.

### 4. Bitmap PILfont completeness

Files:

- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs-py/python/pillow_rs/imagefont.py`

Status:

- Active bitmap rows pass.
- Function/region coverage remains incomplete.

Decision:

- Add targeted public rows for distinct bitmap font formats and malformed input.
- Keep Python binding thin: path/file bytes in Python, parsing/rendering in Rust.

### 5. Embedded bitmap / color glyph completeness

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/tt/sbit.rs`
- `pillow-rs-freetype/src/render.rs`

Status:

- Recent SBIT fixes moved active private-use embedded bitmap rows to real
  Pillow parity.
- `GRAY2`, `GRAY4`, and `BGRA` coverage conversion is now implemented.
- Large SBIT regions remain untested.

Boundary:

- `pillow-rs-freetype/src/tt/sbit.rs` owns EBLC/EBDT and CBLC/CBDT parsing,
  strike selection, glyph bitmap decoding, compound bitmap composition, and
  malformed SBIT error classification.
- `pillow-rs/src/font/imagingft.rs` owns only the Pillow `_imagingft` adapter
  behavior after a FreeType-like glyph slot exists: layout bbox from bitmap
  glyph bounds, offsets, public mode conversion, and final mask coverage bytes.
- Any future fix that needs SBIT table-format knowledge in `imagingft.rs` should
  be rejected as a layering bug and moved into `pillow-rs-freetype`.

Decision:

- Add only independent SBIT/color bitmap rows that move lower `sbit.rs` or
  `render.rs` regions.
- Priority cases:
  - alpha-zero BGRA;
  - malformed/truncated color bitmap data;
  - additional embedded bitmap formats/strikes;
  - second glyphs that prove source rectangle and metrics variation.

### 6. Charmap / encoding

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/font.rs`
- `pillow-rs-freetype/src/tt/cmap.rs`

Status:

- BASIC Unicode-compatible rows pass.
- Constructor `encoding` is preserved as a public option.
- Lower charmap coverage remains weak.

Decision:

- Add public rows for symbol/private-use/non-default charmap behavior where
  Pillow output differs from normal Unicode charmap behavior.
- Keep encoding behavior in Rust core, not Python or JS.

### 7. Variation APIs prove return shape more than visible rendering

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/tt/fvar.rs`
- `pillow-rs-freetype/src/tt/gvar.rs`
- `pillow-rs-freetype/src/tt/mvar.rs`

Status:

- Variation names/axes/setter rows pass.
- Some metrics-path variation coverage exists.
- More visible rendering/metrics deltas are needed.

Decision:

- Add rows where setting a named instance or axis changes `getmetrics`,
  `getbbox`, `getlength`, or mask bytes, then compare exact Pillow output.

### 8. Public class-shape mismatch

Files:

- `pillow-rs/src/lib.rs`
- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs-py/python/pillow_rs/imagefont.py`

Pillow has:

- `ImageFont.ImageFont` for bitmap fonts;
- `ImageFont.FreeTypeFont` for FreeType fonts;
- `ImageFont.TransposedFont` wrapper.

Rust currently has:

- public `ImageFont` for FreeType-backed fonts;
- public `PilFont` exported from root for bitmap PILfont behavior;
- transposed behavior exposed as helper operations.

Decision:

- Behavior parity can continue with the current split.
- If exact API shape is required, introduce explicit Rust/root public types for
  bitmap, FreeType, and transposed fonts and keep them delegated through
  `pillow-rs/src/lib.rs`.

## Action order

1. Finish real lower `fontdone` stroker and stroke-border behavior.
2. Add successful public Pillow rows for normal stroke and `stroke_filled=true`.
3. Re-evaluate and remove/prove the Rust stroked extent clamp.
4. Add fixed-point bbox/mask rows for negative bearings, fractional starts,
   descenders/ascenders, and anchor extremes.
5. Add bitmap `ImageFont.ImageFont` rows for loader/raster/malformed edge paths.
6. Add only coverage-moving embedded bitmap/color glyph rows.
7. Add charmap/encoding rows where Pillow behavior is distinct.
8. Add variation rows that prove visible metric/render deltas.
9. Decide whether class-shape parity requires explicit public Rust
   `BitmapImageFont`, `FreeTypeFont`, and `TransposedFont` types.

## Final decision statement

Current trusted status:

- Active fixture rows are exact live-oracle parity rows.
- Inputs are pure inputs; outputs and error payloads are generated by Pillow at
  runtime.
- The harness does not compare Rust with itself.

Current blockers to full parity:

- general stroke/stroke-border implementation;
- suspect stroked extent clamp;
- successful libraqm shaping, if in scope;
- incomplete bitmap PILfont function/region coverage;
- incomplete embedded bitmap/color glyph coverage;
- incomplete lower charmap/encoding/table/error coverage;
- variation rows that do not yet prove enough visible output deltas;
- possible public class-shape mismatch with Pillow.

Therefore, the next engineering work should start with lower stroker and
stroke-border parity, because it is the largest known public `FreeTypeFont`
behavior gap and directly blocks covering the remaining `stroke_filled=true`
path in `pillow-rs/src/font/imagingft.rs`.

# ImageFont missing coverage and parity decision document

Date: 2026-07-27

Purpose: this is the actionable decision file for `PIL.ImageFont` parity. It
records:

1. uncovered-line and uncovered-region analysis by logic area; and
2. a Pillow 12.2.0 `PIL.ImageFont` vs Rust implementation comparison across the
   Rust files that participate in Font behavior.

This document is intentionally stricter than a passing-test summary. A Rust path
is trusted only when it is reached by an active public `PIL.ImageFont` fixture
row and that row compares Rust output/error payloads against the live Pillow
12.2.0 oracle at runtime.

## Evidence boundary

- Repo: `/Users/lazytrot/work/pillow-rs`
- Branch: `main`
- Current HEAD when this document was written:
  `5e6071cf5eb9af959e7e340f6ff7fa598e311e1d`
- Last trusted Coverage MCP command:
  `font-tests-coverage-with-freetype-pillow-12-2`
- Last trusted Coverage MCP run:
  `828c2d47-1a0f-4742-b5d9-8a9f49641ad3`
- Last trusted Coverage MCP snapshot:
  `4e04ba48-488e-4798-87f6-7fc34d4ad4ab`
- Last trusted measured commit:
  `2e45e4e4dec60bdfca5df2a7a17640f67a0037c7`
- Oracle: repo-local `.oracle-venv/bin/python`
- Oracle Pillow version: `12.2.0`
- Oracle RAQM status: unavailable (`features.check_feature("raqm") == false`)
- Active fixture root:
  `pillow-rs/tests/fixtures/font/inputs/public-api`
- Manifest:
  `pillow-rs/tests/fixtures/font/font_manifest.yaml`
- Harness:
  `pillow-rs/tests/font_public_api.rs`

Trusted status from the last Coverage MCP run:

- 350 active input-only Font rows passed.
- Expected output/errors were generated at runtime from live Pillow 12.2.0.
- Input JSON did not contain stored expected pixels, hashes, or error payloads.

Current working-tree status at this document:

- Working tree has 352 active input rows after the embedded-bitmap/SBIT sweep.
- The new SBIT rows are intentionally not yet trusted: they expose a real Rust
  mismatch against live Pillow instead of passing.
- Do not claim the working tree has complete ImageFont parity until the SBIT
  mismatch is fixed and Coverage MCP has produced a new passing snapshot.

## Pillow 12.2.0 public `ImageFont` surface

Local Pillow 12.2.0 exposes these relevant public surfaces:

| Pillow surface | Public functions / methods |
|---|---|
| Module functions | `load`, `load_path`, `truetype`, `load_default`, `load_default_imagefont` |
| Module constants/enums | `MAX_STRING_LENGTH`, `Layout.BASIC`, `Layout.RAQM` |
| `ImageFont.ImageFont` bitmap class | `getbbox`, `getlength`, `getmask` plus loaded `info` state |
| `ImageFont.FreeTypeFont` | `getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `getmask2`, `font_variant`, `get_variation_names`, `set_variation_by_name`, `get_variation_axes`, `set_variation_by_axes` |
| `ImageFont.TransposedFont` | `getmask`, `getbbox`, `getlength` |

Rust intentionally implements a byte-oriented core API. File/path/stream object
handling belongs in thin bindings or tests; parsing, layout, glyph selection,
metrics, rasterization, and error semantics must stay in Rust core.

## Direct Rust Font coverage status

Last trusted snapshot: `4e04ba48-488e-4798-87f6-7fc34d4ad4ab`.

| File | Lines | Branches | Functions | Regions | Decision |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | Adapter methods are fully reached by active public rows. |
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | Default Aileron path is covered. |
| `pillow-rs/src/font/imagingft.rs` | 1642/1666 98.56% | 246/254 96.85% | 163/174 93.68% | 2547/2645 96.29% | FreeType-backed Font still has real gaps. |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | Bitmap ImageFont behavior passes active rows but is not complete by function/region coverage. |

The overall workspace coverage from that suite is not the ImageFont decision
metric because the suite only targets Font behavior while the artifact includes
much of the workspace.

## 1. Uncovered-line logic-based analysis

### `pillow-rs/src/font/imagingft.rs`

Coverage MCP reported 13 direct gaps in the trusted snapshot: 5 uncovered lines
and 8 partial-branch lines.

| Line(s) | Rust logic | Pillow behavior | Decision |
|---:|---|---|---|
| `91`, `92` | FreeType error-table declaration and unknown-error fallback area. | `_imagingft.c::geterror` maps FreeType errors to `OSError` and has an unknown fallback. | Not an independent product behavior unless a public font triggers table miss. Do not add private table tests as parity proof. |
| `253` | Static table row for `FT_Err_Too_Many_Instruction_Defs`. | Pillow raises `OSError("too many instruction definitions")`. | Behavior is proven by public row `font.getlength.hinter_too_many_instruction_defs`; LLVM still marks static data as uncovered. Keep row; do not duplicate. |
| `271` | Static table row for `FT_Err_Invalid_Horiz_Metrics`. | Pillow can expose this as `OSError("invalid horizontal metrics")` if a real malformed font reaches it. | Real remaining potential error gap. Add only a real public-input font that Pillow itself rejects with this error. |
| `796` | Constant/declaration area around kerning defaults. | No separate Pillow behavior. | Coverage artifact unless source-context later proves a distinct branch. |
| `826`, `829` | `floor26` / `ceil26` fixed-point conversion. | Pillow C uses fixed-point pixel rounding in bbox/anchor/render math. | Add independent public rows for negative bearings, fractional `start`, descenders/ascenders, anchor extremes, and glyphs crossing pixel boundaries. |
| `928` | BASIC glyph-run/kerning branch marker. | Pillow fallback layout adds kerning only after a previous glyph exists. | Existing BASIC/mono/kerning rows prove visible behavior, but marker remains. Do not chase with duplicate rows unless source-context identifies a distinct public input. |
| `1094`, `1097`, `1099` | Rust stroked extent clamp adjusts target width/height after seeing rendered bitmap. | Pillow allocates from `_imagingft.c::bounding_box_and_anchors` and clips writes during render. | Suspect Rust-only compatibility shim. Keep visible. After lower stroker parity is fixed, remove it or prove it with a C/Pillow first-divergence trace. |
| `1193`, `1194` | `stroke_filled=true` selects `FT_Outline_Glyph_StrokeBorder`. | Pillow supports `getmask2(..., stroke_filled=True)` through FreeType `FT_Glyph_StrokeBorder`. | Real implementation/coverage gap. Lower `pillow-rs-freetype` stroke-border support is not generally trusted yet. |

### `pillow-rs/src/font/pilfont.rs`

Bitmap `ImageFont.ImageFont` behavior is covered for the active rows, but
function/region coverage is not complete.

Important untrusted logic areas:

- direct already-opened image constructor `from_pilfont_data`;
- `open_pilfont_glyph_image` PNG/GIF/PBM handling;
- PBM `P1` and `P4` parsing edge cases;
- truncated raster behavior where Pillow can defer an error until `getmask`;
- descriptor-table and source-rectangle clipping variants;
- invalid glyph image modes and zero/overflow dimensions.

Decision: add only public Pillow rows for distinct bitmap `ImageFont.load`,
`load_default_imagefont`, `ImageFont.getbbox`, `ImageFont.getlength`, and
`ImageFont.getmask` behavior. Do not count private loader unit tests as parity.

### Lower `pillow-rs-freetype` coverage that affects ImageFont trust

`imagingft.rs` is only the adapter. FreeType-backed `PIL.ImageFont` correctness
depends on the lower pure-Rust FreeType implementation.

High-risk lower coverage from the trusted snapshot:

| File | Lines | Branches | Functions | Regions | ImageFont risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 1107/8186 13.52% | 77/2075 3.71% | 94/586 16.04% | 1442/11495 12.54% | Very high: handles, face/glyph wrappers, stroker, variation and color/bitmap routes. |
| `pillow-rs-freetype/src/api.rs` | 208/1186 17.54% | 35/294 11.90% | 25/105 23.81% | 275/1737 15.83% | High: lower public font API under ImageFont. |
| `pillow-rs-freetype/src/font.rs` | 1286/4747 27.09% | 162/702 23.08% | 126/392 32.14% | 1777/6728 26.41% | High: load, names, metrics, charmap, glyph selection. |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | High: rendered mask byte parity. |
| `pillow-rs-freetype/src/tt/sbit.rs` | 100/814 12.29% | 13/72 18.06% | 13/108 12.04% | 186/1269 14.66% | Very high: embedded bitmap/color glyph behavior is weak. |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | High: Unicode, private-use, symbol, and encoding behavior. |
| `pillow-rs-freetype/src/tt/glyf.rs` | 174/545 31.93% | 34/96 35.42% | 8/20 40.00% | 219/694 31.56% | High: TrueType outline behavior. |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | High: CFF/OpenType outlines. |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 725/1493 48.56% | 148/480 30.83% | 32/48 66.67% | 1298/3107 41.78% | High: hinted TrueType bytecode and error classification. |
| `pillow-rs-freetype/src/grays.rs` | 571/827 69.04% | 122/190 64.21% | 25/35 71.43% | 854/1106 77.22% | Medium/high: antialias raster output. |
| `pillow-rs-freetype/src/scaler.rs` | 806/1342 60.06% | 114/186 61.29% | 40/66 60.61% | 918/1436 63.93% | Medium/high: scaling and metrics. |
| `pillow-rs-freetype/src/autohint/latin.rs` | 1988/2962 67.12% | 673/1263 53.29% | 45/67 67.16% | 2806/4283 65.51% | Medium/high: Latin hinted glyphs. |
| `pillow-rs-freetype/src/autohint/cjk.rs` | 396/879 45.05% | 130/398 32.66% | 11/18 61.11% | 531/1180 45.00% | High for CJK fonts. |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 26/42 61.90% | 6/12 50.00% | 1/2 50.00% | 44/67 65.67% | Partially proven; malformed/device-metrics paths remain untrusted. |
| `pillow-rs-freetype/src/tt/mvar.rs` | 58/67 86.57% | 3/6 50.00% | 4/7 57.14% | 92/113 81.42% | Partially proven; malformed/value-tag paths remain untrusted. |
| `pillow-rs-freetype/src/tt/vhea.rs` | 8/11 72.73% | 1/2 50.00% | 1/1 100.00% | 8/9 88.89% | Partially proven; short/error path remains untrusted. |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 28/50 56.00% | 3/8 37.50% | 1/2 50.00% | 44/65 67.69% | Partially proven; malformed/overflow paths remain untrusted. |

Decision: 100% direct `imagingft.rs` coverage would still not be enough if the
lower FreeType paths remain uncovered by public Pillow rows.

## 2. Pillow vs Rust implementation comparison

| Pillow behavior | Rust files | Current status | Missing / wrong / decision |
|---|---|---|---|
| `ImageFont.load` bitmap PILfont loading | `pillow-rs/src/font/pilfont.rs`; test/binding loader | Implemented for active rows. | Core should not own filesystem search. More bitmap loader/render edge rows are needed for full region trust. |
| `ImageFont.load_path` | test/binding loader | Represented by fixture operation. | Path search is binding/test behavior, not core parsing logic. Keep bindings thin. |
| `ImageFont.load_default_imagefont` | `pilfont.rs` embedded `courb08` | Covered by active row. | Add only if corrupted/default edge behavior is explicitly in scope. |
| `ImageFont.load_default(size)` | `default_aileron.rs`, `mod.rs`, `imagingft.rs` | Covered by active rows. | Keep pinned to Pillow 12.2.0 default Aileron behavior. |
| `ImageFont.truetype` | `mod.rs`, `imagingft.rs`, lower `fontdone` | Byte constructor/options implemented. | Core does not model Python path/stream object shape. That is intentional. |
| Bitmap `ImageFont.getbbox/getlength/getmask/info` | `pilfont.rs` | Active rows pass. | Function/region coverage still incomplete; add distinct public bitmap rows. |
| `FreeTypeFont.getname` | `imagingft.rs`, lower name tables | Active rows pass. | More missing-name/platform fallback rows only if coverage shows distinct public behavior. |
| `FreeTypeFont.getmetrics` | `imagingft.rs`, lower metrics tables | Active rows pass for standard/fixed/fallback/vertical/mvar cases. | Malformed metrics, value-tag, and vertical edge paths remain undertrusted. |
| `FreeTypeFont.getlength` | `imagingft.rs::glyph_run`, lower charmap/kerning/hinting | Active BASIC rows pass. | RAQM success missing; more charmap, bytecode, and fixed-point rounding cases needed. |
| `FreeTypeFont.getbbox` | `imagingft.rs::bbox_from_glyph_run` | Active BASIC rows pass. | Negative bearings, fractional starts, anchor extremes, and stroked extent paths remain undertrusted. |
| `FreeTypeFont.getmask` | `imagingft.rs`, `fontdone` render paths | Active normal rows pass. | General stroke and embedded bitmap/color glyph paths are not trusted. |
| `FreeTypeFont.getmask2` | `imagingft.rs`, lower stroke/render paths | Active normal/start rows pass. | Successful `stroke_filled=true` is wired but not proven; lower stroke-border implementation is incomplete. |
| Variation APIs | `imagingft.rs`, lower fvar/gvar/mvar/name paths | Active rows pass. | Need metric/render rows proving variation changes, plus malformed variation table paths if in scope. |
| `TransposedFont` class | helper operations in Rust/tests | Behavior rows pass. | Rust does not expose a 1:1 `TransposedFont` class shape. Decide whether class-shape parity is required. |
| `Layout.RAQM`, `direction`, `features`, `language` | `ImageFontTextOptions`, `PilError::UnsupportedLibraqm` | No-libraqm error parity is covered. | Successful libraqm shaping is not implemented. Complete `PIL.ImageFont` parity cannot be claimed while this is excluded. |
| `FreeTypeFont.__getstate__` / `__setstate__` | none | Not implemented. | Missing if Python class-shape/state parity is in scope. Exclude explicitly if the product target is metrics/rendering only. |

## 3. Current concrete mismatch found by the SBIT sweep

Existing SBIT rows used text `"A"` with generated SBIT fonts. Inspection showed
the bitmap strike glyphs are actually mapped to private-use codepoints such as
`U+E000` and `U+E001`. The `"A"` rows did not exercise the embedded bitmap
image data and therefore gave a false sense of SBIT coverage.

The working tree now changes/adds public rows in:

- `pillow-rs/tests/fixtures/font/inputs/public-api/font.getmask.json`
- `pillow-rs/tests/fixtures/font/inputs/public-api/font.getmask2.json`

These use private-use inputs such as `"\ue000"` and `"\ue001"` to force real
embedded bitmap behavior.

Current failing example:

- Case: `font.getmask.sbit_mono_private_base`
- Pillow 12.2.0: status `ok`, mode `L`, size `[10, 2]`, non-empty pixels.
- Rust: status `ok`, mode `L`, size `[10, 0]`, empty pixels.

First-divergence analysis:

- `pillow-rs-freetype/src/api.rs::sbit_glyph_slot` creates a bitmap glyph slot
  with a real rendered bitmap but sets `outline_cbox` / `outline_bbox` to zero.
- `pillow-rs/src/font/imagingft.rs::glyph_run` stores only `slot.outline_cbox`
  in `RunGlyph`.
- `pillow-rs/src/font/imagingft.rs::bbox_from_glyph_run` sizes the mask from the
  stored cbox before rendering.
- For bitmap-only glyphs, the cbox is zero, so Rust allocates a zero-height
  public mask even though the later render pass has bitmap pixels available.

Decision:

- This is a real Rust implementation bug exposed by a live Pillow public input.
- Fix the adapter to use an effective layout cbox for bitmap glyph slots:
  bitmap bounds should come from `bitmap_left`, `bitmap_top`, `bitmap.width`,
  and `bitmap.rows`.
- After the fix, run `make -C pillow-rs font-tests`, then a Coverage MCP run.
- Only then update the trusted row count and SBIT coverage numbers.

## 4. Wrong or suspect Rust implementation that needs a decision

### A. General stroker and `stroke_filled=true`

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/handles.rs`

Pillow supports stroked text and `stroke_filled=True`. Rust has the public
options wired, but lower pure-Rust FreeType stroker/export support is still not
generally proven for real glyph outlines.

Decision:

- Highest-priority implementation gap after the current SBIT bbox bug.
- Remove fixture-specific stroker shortcuts over time; do not add more.
- Implement general `FT_Stroker_ParseOutline`, segment routes, border export,
  and render-to-bitmap behavior.
- Add live Pillow public rows for normal stroke, `stroke_filled=true`, mono
  stroke, descenders/ascenders, and clipped sides.

### B. Stroked extent clamp

File:

- `pillow-rs/src/font/imagingft.rs`

Rust currently mutates bbox extents after seeing actual stroked bitmap extents.
Current evidence does not show Pillow doing the same; Pillow allocates from
`bounding_box_and_anchors` and clips writes.

Decision:

- Treat as a compatibility shim, not trusted parity.
- After real lower stroker parity exists, remove this clamp or prove it by C
  first-divergence trace.

### C. Successful RAQM shaping

Files:

- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs/src/error.rs`

Rust returns `PilError::UnsupportedLibraqm` for direction/features/language.
That is correct for the current no-raqm Pillow oracle because the public payload
matches Pillow's no-raqm error behavior.

Decision:

- This is an explicit out-of-scope area, not full parity.
- Complete `PIL.ImageFont` parity requires either real RAQM shaping or a product
  decision permanently excluding it.

### D. Bitmap vs FreeType class shape

Files:

- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs/src/lib.rs`

Pillow has separate public concepts:

- `ImageFont.ImageFont` for bitmap fonts;
- `ImageFont.FreeTypeFont` for FreeType fonts;
- `ImageFont.TransposedFont` wrapper.

Rust currently exposes FreeType as `ImageFont`, bitmap internally as `PilFont`,
and transposed behavior as helper operations.

Decision:

- Behavior parity can continue with the current internal split.
- If public API shape must mirror Pillow, introduce explicit public shape for
  bitmap, FreeType, and transposed fonts rather than overloading one type.

### E. Error mapping

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/convert.rs`
- lower parser/hinter files

Recent public rows fixed/proved:

- missing `hmtx` -> `OSError("horizontal metrics (hmtx) table missing")`;
- IDEF overflow -> `OSError("too many instruction definitions")`.

Still unproven:

- `FT_Err_Invalid_Horiz_Metrics`;
- malformed `hdmx`, `mvar`, `vhea`, `vmtx`;
- rare bytecode/parser errors reachable only with specific malformed fonts;
- unknown FreeType error fallback through a public input.

Decision:

- Add only real fonts where Pillow naturally produces the target error.
- Do not hard-code error expectations in input JSON.
- Do not count private error-table unit tests as public parity.

### F. Embedded bitmap/color glyph path

Files:

- `pillow-rs-freetype/src/tt/sbit.rs`
- `pillow-rs-freetype/src/render.rs`
- `pillow-rs/src/font/imagingft.rs`

The SBIT sweep proves current fixture design was insufficient: rows using
`"A"` did not necessarily hit embedded bitmap glyph images. Private-use mapped
glyphs now expose a real mismatch.

Decision:

- Fix effective bitmap glyph bbox in public Font layout.
- Then keep minimal SBIT rows covering mono, gray, gray2, gray4, BGRA/color, and
  multi-glyph bitmap cases.
- Use Coverage MCP to confirm `sbit.rs` decoder regions actually move.

### G. Charmap and encoding

Files:

- `pillow-rs-freetype/src/tt/cmap.rs`
- `pillow-rs-freetype/src/font.rs`
- `pillow-rs/src/font/imagingft.rs`

Rust preserves constructor `encoding` as an option but mostly follows
Unicode-compatible behavior in active rows. Pillow can expose more charmap
behavior through FreeType.

Decision:

- Add symbol/private-use/non-default charmap rows only when a fixture proves
  Pillow output differs from the normal Unicode path.
- Keep encoding logic in Rust core, not Python/JS bindings.

## 5. Action order

1. Fix SBIT bitmap glyph layout bbox in `imagingft.rs`.
2. Re-run `make -C pillow-rs font-tests`.
3. Run Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`.
4. Update this document with the new run ID, snapshot ID, row count, and
   `imagingft.rs` / `sbit.rs` metrics.
5. Then address general stroker and `stroke_filled=true`.
6. Re-evaluate and remove/prove stroked extent clamp.
7. Add only minimal, independent public Pillow rows for:
   - stroke and stroke-border;
   - fixed-point rounding;
   - bitmap PILfont edge behavior;
   - charmap/encoding;
   - reachable FreeType error classifications;
   - variation rows that alter metrics/rendering.
8. Decide separately whether API class-shape parity includes Rust-level
   `BitmapImageFont`, `FreeTypeFont`, `TransposedFont`, and state roundtrips.

## Final decision statement

Trusted now:

- The last Coverage MCP snapshot proves exact runtime parity for the previous
  350-row input-only `PIL.ImageFont` corpus.
- The harness compares against live Pillow 12.2.0 and does not use stored output
  payloads in input JSON.

Not trusted now:

- Complete `PIL.ImageFont` parity.
- The current 352-row working tree, because the new SBIT rows expose a real
  failing mismatch.
- General stroke/stroke-border behavior.
- Successful RAQM shaping.
- Full bitmap PILfont function/region coverage.
- Embedded bitmap/color glyph behavior until the SBIT mismatch is fixed and
  Coverage MCP confirms real decoder coverage.
- Full charmap/encoding behavior.
- Exhaustive rare FreeType error behavior through public inputs.

The immediate next engineering action should be the SBIT bitmap glyph bbox fix,
because it is a concrete public Pillow parity failure found by uncovered-path
analysis. The next major design action should be the general stroker and
stroke-border implementation, because that is the largest remaining known
public `FreeTypeFont` behavior gap.

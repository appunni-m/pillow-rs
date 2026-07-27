# ImageFont final coverage and implementation review

Date: 2026-07-27

Purpose: this document is the decision artifact for `PIL.ImageFont` parity. It
combines:

1. uncovered-line / uncovered-region analysis based on the latest Coverage MCP
   snapshot; and
2. a Pillow 12.2.0 `ImageFont` vs Rust implementation comparison across the
   files that participate in Font behavior.

This is not a passing-test summary. Passing rows only prove the fixture corpus
they execute. Any code path not executed by a live Pillow oracle row remains
untrusted unless this document explicitly classifies it as an instrumentation
artifact or an intentional out-of-scope behavior.

## Evidence base

- Rust branch: `main`
- Rust HEAD when this document was written: `c5ca3f917395e3e8aba5231bb05c8aa54170d40c`
- Last coverage-measured implementation commit: `2e45e4e4dec60bdfca5df2a7a17640f67a0037c7`
- Coverage MCP command: `font-tests-coverage-with-freetype-pillow-12-2`
- Coverage MCP run: `828c2d47-1a0f-4742-b5d9-8a9f49641ad3`
- Coverage MCP snapshot: `4e04ba48-488e-4798-87f6-7fc34d4ad4ab`
- Coverage suite: `font-with-freetype`
- Oracle: repo-local `.oracle-venv/bin/python`
- Pillow oracle version: `12.2.0`
- Pillow oracle source inspected:
  - `.oracle-venv/lib/python3.12/site-packages/PIL/ImageFont.py`
  - native `PIL._imagingft`
- Oracle RAQM status: unavailable / false in the repo-local Pillow 12.2.0 venv

Current active Font fixture corpus:

- `350` input-only rows.
- `350/350` rows pass exact runtime parity against live Pillow 12.2.0.
- Input JSON files under `pillow-rs/tests/fixtures/font/inputs/public-api` are
  input-only. They do not contain expected output bytes, hashes, expected error
  strings, or cached oracle status.
- Output and error expectations are generated at test runtime from the Pillow
  oracle.

## Coverage status summary

Coverage snapshot `4e04ba48-488e-4798-87f6-7fc34d4ad4ab` reports these direct
Font-file metrics:

| File | Lines | Branches | Functions | Regions | Decision |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | Direct public FreeType wrapper is covered by the active corpus. |
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | Default Aileron path is covered. |
| `pillow-rs/src/font/imagingft.rs` | 1642/1666 98.56% | 246/254 96.85% | 163/174 93.68% | 2547/2645 96.29% | Real partial gaps remain, mainly stroke/stroke-border and rare error paths. |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | Bitmap ImageFont behavior passes active rows, but function/region coverage is not complete. |

The overall suite totals are low because the coverage artifact includes much of
the workspace, while the suite intentionally runs only Font behavior:

- Lines: 15709/50733, 30.96%
- Branches: 2671/10758, 24.83%
- Functions: 1203/3608, 33.34%
- Regions: 22636/78553, 28.82%

Do not use these overall totals to judge ImageFont parity. Use the Font and
lower `pillow-rs-freetype` file-level data below.

## 1. Uncovered-line logic-based analysis

### Direct `pillow-rs/src/font/imagingft.rs` gaps

Coverage MCP reports 13 relevant gaps in `imagingft.rs`: 5 uncovered lines and
8 partial-branch lines.

| Line(s) | Coverage reason | Rust logic | Pillow 12.2.0 behavior | Decision |
|---:|---|---|---|---|
| 91 | partial branch | Comment/metadata region on the FreeType error-message table. | `_imagingft.c::geterror` maps FreeType errors to Pillow `OSError` text and uses an unknown fallback for misses. | Instrumentation artifact around table metadata, not a separate public behavior. No implementation action by itself. |
| 92 | uncovered | `#[rustfmt::skip]` before `FT_ERROR_MESSAGES`. | Not a runtime Pillow behavior. | Instrumentation artifact. No product action. |
| 253 | uncovered | Table row for `FT_Err_Too_Many_Instruction_Defs` → `"too many instruction definitions"`. | Pillow maps the same FreeType error to `OSError("too many instruction definitions")`. | Behavior is now proven through public row `font.getlength.hinter_too_many_instruction_defs`; LLVM still marks static table data as uncovered. Do not chase with duplicate rows. |
| 271 | uncovered | Table row for `FT_Err_Invalid_Horiz_Metrics` → `"invalid horizontal metrics"`. | Pillow maps this FreeType error to `OSError("invalid horizontal metrics")` if a real font triggers it. | Real potential gap. Needs a public Pillow-reachable font fixture that triggers this exact error. Existing fixture reference names for this lane are not present in the checkout; either regenerate/import the missing synthetic font or document unreachable status. |
| 796 | partial branch | Constant `KERN_DEFAULT`. | Not a Pillow behavior. | LLVM segment artifact. No product action. |
| 826 | partial branch | `floor26`: 26.6 floor conversion. | Pillow C uses pixel/26.6 conversion in bbox/anchor math. | Add only distinct public rows that naturally hit negative/fractional bbox math. Do not add duplicate `AV`/`jQ` rows if coverage does not move. |
| 829 | partial branch | `ceil26`: 26.6 ceil conversion. | Pillow C uses ceil-style conversions in rendered extents/start offset paths. | Same as line 826: target negative bearings, fractional `start`, anchors, ascenders/descenders, and clipping rows. |
| 928 | partial branch | `bbox_from_run_with_flags` branch/instrumentation around load flags and run construction. | Pillow BASIC layout builds glyph info once and bbox/render consume it. | Existing BASIC, mono, kerning, and descender rows prove visible behavior, but branch marker remains. Treat as low-priority until source-context identifies a distinct input. |
| 1094 | partial branch | If stroked rendered width exceeds bbox-derived width, clamp `x_max`. | Pillow allocates from `bounding_box_and_anchors` and clips writes during render; no clear evidence that it mutates bbox like Rust does. | Suspect Rust-only compatibility shim. Keep visible until lower stroker/bbox parity is fixed, then remove or prove with C trace. |
| 1097 | partial branch | If stroked rendered height exceeds bbox-derived height, clamp `y_max`. | Same as above. | High-risk unproven branch. Needs real stroked descender/ascender rows after lower stroker support is general. |
| 1099 | uncovered | Body of stroked height clamp. | Same as above. | Same as 1097. This is the clearest direct uncovered behavior in `imagingft.rs`. |
| 1193 | partial branch | `if stroke_filled` selector. | Pillow routes `stroke_filled=True` through `FT_Glyph_StrokeBorder`; normal stroke uses `FT_Glyph_Stroke`. | Rust is wired, but successful public rows do not exercise this branch. Real implementation blocker lives lower in `pillow-rs-freetype`. |
| 1194 | uncovered | Call to `FT_Outline_Glyph_StrokeBorder`. | Pillow succeeds for valid stroke-border glyph outlines when FreeType supports the glyph. | Real gap. Implement general lower stroke-border geometry/export, then add public `getmask2(..., stroke_filled=True)` rows. |

### Direct `pillow-rs/src/font/pilfont.rs` gaps

`pilfont.rs` has 715/737 lines and 1014/1094 regions covered. Branch coverage is
100%, but function coverage is only 58/78. This means bitmap `ImageFont` behavior
is passing for current rows, but not every helper/function variant is trusted.

Important logic areas:

- `from_pilfont_data(data, image)` is a direct already-opened-image constructor.
  The active harness mostly uses loader-style `from_pilfont_glyph_data` and
  `open_pilfont_glyph_image` paths. This is not a public Pillow method by itself,
  but it backs `ImageFont.load`.
- `open_pilfont_glyph_image` decodes PNG/GIF through the normal image decoder,
  and locally handles PBM `P1`/`P4` because the workspace does not expose Netpbm
  as a general codec.
- PBM error behavior is delicate: truncated `P4` with CRLF separator can defer a
  SystemError until `getmask`, while other truncation paths return image-open
  errors. These rows should stay only if they are proven from Pillow, not from
  Rust self-expectations.

Decision: bitmap ImageFont is acceptable for the active corpus, but not complete.
Add rows only for distinct public `ImageFont.load/load_default_imagefont`
behaviors: mode `1`, mode `L`, invalid glyph image mode, PBM `P1`, PBM `P4`,
truncated PBM, missing glyph, zero-width text, and descriptor-table errors.

### Lower `pillow-rs-freetype` files that affect ImageFont trust

`imagingft.rs` delegates most real font behavior into `pillow-rs-freetype`.
These files are therefore in scope for `PIL.ImageFont` trust even though the
public test entry point is `pillow-rs/src/font`.

High-risk lower coverage from snapshot `4e04ba48-488e-4798-87f6-7fc34d4ad4ab`:

| File | Lines | Branches | Functions | Regions | ImageFont risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 1107/8186 13.52% | 77/2075 3.71% | 94/586 16.04% | 1442/11495 12.54% | Very high: FreeType handles, face/glyph wrappers, stroker, color APIs. |
| `pillow-rs-freetype/src/api.rs` | 208/1186 17.54% | 35/294 11.90% | 25/105 23.81% | 275/1737 15.83% | High: lower public font API paths. |
| `pillow-rs-freetype/src/font.rs` | 1286/4747 27.09% | 162/702 23.08% | 126/392 32.14% | 1777/6728 26.41% | High: load, names, metrics, glyph selection, charmap, variation, render entry. |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | High: output mask byte parity. |
| `pillow-rs-freetype/src/tt/sbit.rs` | 100/814 12.29% | 13/72 18.06% | 13/108 12.04% | 186/1269 14.66% | High: embedded bitmap/color glyph behavior is weakly covered. |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | High: Unicode/symbol/charmap behavior. |
| `pillow-rs-freetype/src/tt/glyf.rs` | 174/545 31.93% | 34/96 35.42% | 8/20 40.00% | 219/694 31.56% | High: TrueType outline shape. |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | High: CFF/OpenType outline behavior. |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 725/1493 48.56% | 148/480 30.83% | 32/48 66.67% | 1298/3107 41.78% | High: hinted TrueType bytecode behavior. |
| `pillow-rs-freetype/src/grays.rs` | 571/827 69.04% | 122/190 64.21% | 25/35 71.43% | 854/1106 77.22% | Medium/high: antialiased raster output. |
| `pillow-rs-freetype/src/scaler.rs` | 806/1342 60.06% | 114/186 61.29% | 40/66 60.61% | 918/1436 63.93% | Medium/high: scaling and metrics. |
| `pillow-rs-freetype/src/autohint/latin.rs` | 1988/2962 67.12% | 673/1263 53.29% | 45/67 67.16% | 2806/4283 65.51% | Medium/high: hinted Latin glyphs. |
| `pillow-rs-freetype/src/autohint/cjk.rs` | 396/879 45.05% | 130/398 32.66% | 11/18 61.11% | 531/1180 45.00% | High for CJK fonts. |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 26/42 61.90% | 6/12 50.00% | 1/2 50.00% | 44/67 65.67% | Publicly exercised once; malformed/alternate hdmx remains unproven. |
| `pillow-rs-freetype/src/tt/mvar.rs` | 58/67 86.57% | 3/6 50.00% | 4/7 57.14% | 92/113 81.42% | Publicly exercised once; malformed/value-tag paths remain unproven. |
| `pillow-rs-freetype/src/tt/vhea.rs` | 8/11 72.73% | 1/2 50.00% | 1/1 100.00% | 8/9 88.89% | Publicly exercised once; error/short path remains unproven. |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 28/50 56.00% | 3/8 37.50% | 1/2 50.00% | 44/65 67.69% | Publicly exercised once; malformed/overflow paths remain unproven. |

Decision: direct `imagingft.rs` coverage near 100% is not sufficient. Full
ImageFont parity requires public Pillow rows that naturally execute these lower
paths and compare live Pillow output/error payloads.

## 2. Pillow 12.2.0 `ImageFont` vs Rust implementation comparison

### Pillow public surface

Local Pillow 12.2.0 exposes:

- module functions:
  - `load`
  - `load_path`
  - `truetype`
  - `load_default`
  - `load_default_imagefont`
- bitmap class `ImageFont.ImageFont`:
  - `getbbox`
  - `getlength`
  - `getmask`
  - `info` exists as loaded metadata/state, though it is not listed as a normal
    public method by `dir(ImageFont.ImageFont)`
- FreeType class `ImageFont.FreeTypeFont`:
  - `getname`
  - `getmetrics`
  - `getlength`
  - `getbbox`
  - `getmask`
  - `getmask2`
  - `font_variant`
  - `get_variation_names`
  - `set_variation_by_name`
  - `get_variation_axes`
  - `set_variation_by_axes`
- wrapper class `ImageFont.TransposedFont`:
  - `getmask`
  - `getbbox`
  - `getlength`
- enum-like values:
  - `Layout.BASIC`
  - `Layout.RAQM`
  - `MAX_STRING_LENGTH`

### Rust public/root surface status

Rust exposes the FreeType-compatible class as `ImageFont` from
`pillow-rs/src/font/mod.rs`, re-exported explicitly from `pillow-rs/src/lib.rs`.

Root `lib.rs` exposes FreeType `ImageFont` functions such as:

- `imagefont_from_bytes`
- `imagefont_from_bytes_with_options`
- `imagefont_load_default`
- `imagefont_getname`
- `imagefont_getmetrics`
- `imagefont_getlength`
- `imagefont_getbbox`
- `imagefont_getmask`
- `imagefont_getmask2`
- `imagefont_variant`
- `imagefont_variant_with_options`
- `imagefont_get_variation_axes`
- `imagefont_get_variation_names`
- `imagefont_set_variation_by_name`
- `imagefont_set_variation_by_axes`

It also exposes adapter/test-oriented helpers:

- `imagefont_text_bbox`
- `imagefont_getbbox_binary`
- `imagefont_getmask2_with_start`
- `imagefont_get_transposed_mask`
- `imagefont_render_text_binary`

Decision: these helper surfaces are acceptable only as binding/test adapters.
They are not Pillow public endpoints and must not become independent sources of
truth. Their behavior must stay derived from real Pillow public operations.

### Surface-by-surface decision table

| Pillow surface | Rust implementation | Current coverage/parity status | Missing or wrong behavior |
|---|---|---|---|
| `ImageFont.load` | Bitmap PILfont path in `pilfont.rs`; file/path I/O remains outside core. | Active bitmap rows pass. | Region/function coverage is incomplete. Add only distinct Pillow-derived bitmap cases. |
| `ImageFont.load_path` | Fixture/binding-level path search; core accepts bytes. | One active row exists. | Full `sys.path` search semantics are not a core responsibility. Binding can own this if needed. |
| `ImageFont.load_default_imagefont` | `PilFont::load_default`. | Covered. | Additional corrupted embedded-data rows are unnecessary unless testing loader errors. |
| `ImageFont.load_default` | `ImageFont::load_default` with embedded Aileron. | Covered. | Keep tied to Pillow 12.2.0 Aileron. If Pillow default changes later, fixtures must pin or update intentionally. |
| `ImageFont.truetype` | `ImageFont::from_bytes_with_options`; bindings load paths/streams to bytes. | Constructor and load-failure rows pass. | Core does not implement OS font directory search or stream lifetime behavior. That is intentional if py/js stay thin. |
| bitmap `ImageFont.getbbox` | `PilFont` + harness adapter. | Active rows pass. | Need more bitmap malformed/edge rows if seeking 100% `pilfont.rs` regions. |
| bitmap `ImageFont.getlength` | `PilFont::getsize` / adapter. | Active rows pass. | Same bitmap edge coverage issue. |
| bitmap `ImageFont.getmask` | `PilFont::getmask`. | Active rows pass. | PBM/mode/truncation behavior should be expanded only through live Pillow oracle rows. |
| `FreeTypeFont.getname` | `ImageFont::getname`. | Covered. | Missing-name/fallback variants are only trusted where rows exist. |
| `FreeTypeFont.getmetrics` | `ImageFont::getmetrics`. | Covered for standard, fixed-width, hhea-zero/no-OS2, vertical/mvar-related rows. | Lower malformed metrics and rare metric error rows remain unproven. |
| `FreeTypeFont.getlength` | BASIC `GlyphRun` through `imagingft.rs`. | BASIC rows pass, including kerning and selected mono rows. | RAQM feature disabling/success is out of scope. More charmap/CJK/symbol and rare error rows needed for full trust. |
| `FreeTypeFont.getbbox` | Shared BASIC glyph run and bbox math. | Active rows pass. | Negative/fractional/anchor/stroke extent branches remain undercovered. |
| `FreeTypeFont.getmask` | BASIC render path through lower `fontdone`. | Active rows pass. | General stroked glyph rows and embedded bitmap/color glyph behavior remain weak. |
| `FreeTypeFont.getmask2` | BASIC render + offset/start path. | Active rows pass for normal/start cases. | `stroke_filled=True` successful path is not proven and lower `FT_Glyph_StrokeBorder` is incomplete. |
| `FreeTypeFont.font_variant` | `font_variant_with_options`. | Active rows pass. | Need more metric/render rows after axis/name changes if variation behavior is target-complete. |
| variation APIs | `get_variation_*` and `set_variation_*`. | Active rows pass. | Lower `mvar`, `gvar`, `varstore`, named-instance, bad-axis paths are not fully covered. |
| `TransposedFont` | No Rust class; helper operations model behavior. | Active rows pass. | Class-shape parity is missing. Behavior parity exists for active rows only. |
| `Layout.BASIC` | Implemented. | Covered. | None for active rows. |
| `Layout.RAQM` / `direction` / `features` / `language` | `PilError::UnsupportedLibraqm` internally; public parity maps to no-raqm Pillow `KeyError`. | Error rows pass in no-raqm environment. | Successful libraqm shaping is not implemented. This prevents full `PIL.ImageFont` parity. |

## 3. Wrong or suspect Rust implementation across files

### A. General stroker and stroke-border support is incomplete

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/handles.rs`

Facts:

- Pillow `FreeTypeFont.getmask/getmask2` supports `stroke_width`.
- Pillow `getmask2(..., stroke_filled=True)` routes through FreeType
  `FT_Glyph_StrokeBorder`.
- Rust now carries `stroke_filled` through `ImageFontTextOptions` and calls
  `FT_Outline_Glyph_StrokeBorder`.
- Lower `pillow-rs-freetype` still does not have general, trusted
  stroke-border geometry/export for real glyph outlines.
- The historical normal-stroke path includes fixture-specific support for a
  selected glyph route. That is not acceptable as a completeness strategy.

Decision:

- Highest priority implementation gap.
- Finish lower `FT_Stroker_ParseOutline`, segment routes, border counts/export,
  and stroked outline-to-bitmap behavior generally.
- Add live Pillow public rows for normal stroke and `stroke_filled=True` only
  after lower geometry is real.
- Do not add new glyph-specific shortcuts.

### B. Stroked extent clamp is a compatibility shim, not proven Pillow logic

File:

- `pillow-rs/src/font/imagingft.rs`

Rust currently clamps stroked `x_max` / `y_max` after seeing actual rendered
bitmap extents. The code comment says this exists because the pure-Rust stroker
can produce a larger bitmap than the bbox-derived target for an active DejaVu
stroke row.

Pillow C allocates the target from `bounding_box_and_anchors` and clips writes
during rendering. Current evidence does not prove Pillow mutates the bbox the
same way Rust does.

Decision:

- Treat this as suspect Rust-only logic.
- Keep it visible as a known risk, not as trusted parity.
- After lower stroker parity is fixed, remove this clamp or prove it with a
  first-divergence C trace.

### C. RAQM is intentionally unsupported, which blocks complete parity

Files:

- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs/src/error.rs`

Rust uses `PilError::UnsupportedLibraqm` when direction/features/language require
RAQM. The public harness maps this to Pillow's no-raqm `KeyError` payload in the
current oracle environment.

Decision:

- Correct for current no-raqm oracle.
- Not full `PIL.ImageFont` parity.
- If complete Pillow `ImageFont` parity becomes required, RAQM success must be
  implemented or explicitly and permanently excluded.

### D. Bitmap `ImageFont` class-shape differs

Files:

- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/lib.rs`

Pillow has a bitmap class named `ImageFont.ImageFont` and a FreeType class named
`ImageFont.FreeTypeFont`. Rust currently names the FreeType-compatible public
handle `ImageFont` and keeps bitmap PILfont as `PilFont`.

Decision:

- Behavior can still be tested, but class-shape parity is not exact.
- If the target is API shape parity with `PIL.ImageFont`, consider:
  - `ImageFont` enum/facade with Bitmap and FreeType variants; or
  - public `BitmapImageFont` + `FreeTypeImageFont` naming while bindings expose
    Pillow-compatible Python classes.
- Do not mix bitmap and FreeType behavior into one implementation without an
  explicit facade boundary.

### E. Core Rust intentionally does not own path/search/stream behavior

Files:

- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/lib.rs`
- binding crates

Pillow module functions accept filenames, file-like objects, and search system
font directories. Core Rust accepts bytes and structured options.

Decision:

- This is correct architecture if bindings remain thin.
- Python/JS bindings may perform byte loading/conversion, but must not own font
  parsing, glyph logic, metrics, layout, or parity rules.

### F. Embedded bitmap/color glyph paths are weak

Files:

- `pillow-rs-freetype/src/tt/sbit.rs`
- `pillow-rs-freetype/src/render.rs`
- `pillow-rs/src/font/imagingft.rs`

Coverage for `sbit.rs` is 100/814 lines and 186/1269 regions. Existing assets
include sbit/embedded bitmap fixtures, but the current public `ImageFont` rows do
not exercise enough independent embedded bitmap/color behavior to trust this
area.

Decision:

- Add Pillow public rows using existing sbit fixtures for `getmask`, `getmask2`,
  and `getbbox`.
- Compare live Pillow output bytes and offsets.
- If Pillow does not use embedded bitmaps for a fixture at the selected size,
  do not count the row as sbit coverage.

### G. Charmap / encoding behavior is not complete

Files:

- `pillow-rs-freetype/src/tt/cmap.rs`
- `pillow-rs-freetype/src/font.rs`
- `pillow-rs/src/font/imagingft.rs`

Pillow `truetype(..., encoding=...)` accepts multiple FreeType encodings, but
the current Rust path primarily selects Unicode-compatible behavior for active
rows and preserves `encoding` as an option.

Decision:

- Add public rows for symbol fonts and non-default charmap behavior only when a
  real Pillow font fixture proves different output.
- Do not implement Python-side encoding logic. The core must own resulting font
  behavior after bytes/options are passed in.

### H. Rare FreeType error mappings are table-aligned but not exhaustively reached

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/convert.rs`
- lower font parser files

Recent work proved:

- missing `hmtx` maps to `OSError("horizontal metrics (hmtx) table missing")`;
- positive IDEF opcode overflow maps to
  `OSError("too many instruction definitions")`.

Still unproven:

- `FT_Err_Invalid_Horiz_Metrics`;
- malformed hdmx/vmtx/vhea/mvar subpaths;
- some rare bytecode/parser errors that may be hard to reach through public
  Pillow `ImageFont`.

Decision:

- Add only real public-input fonts that trigger Pillow errors.
- Do not hard-code errors in fixture JSON.
- Do not assert table completeness as coverage completeness.

## 4. Active fixture corpus inventory

Active input-only files:

| Input file | Cases |
|---|---:|
| `font.ImageFont.getbbox.json` | 4 |
| `font.ImageFont.getlength.json` | 4 |
| `font.ImageFont.getmask.json` | 19 |
| `font.ImageFont.info.json` | 3 |
| `font.TransposedFont.getbbox.json` | 3 |
| `font.TransposedFont.getlength.json` | 3 |
| `font.TransposedFont.getmask.json` | 6 |
| `font.constructor.json` | 9 |
| `font.get_transposed_mask.json` | 10 |
| `font.getbbox.json` | 32 |
| `font.getbbox_binary.json` | 9 |
| `font.getlength.json` | 22 |
| `font.getmask.json` | 36 |
| `font.getmask2.json` | 43 |
| `font.getmask2_with_start.json` | 23 |
| `font.getmetrics.json` | 8 |
| `font.getname.json` | 5 |
| `font.has_variations.json` | 4 |
| `font.layout_failure.json` | 1 |
| `font.load.json` | 25 |
| `font.load_default_imagefont.json` | 1 |
| `font.load_failure.json` | 8 |
| `font.load_path.json` | 1 |
| `font.render_text.json` | 7 |
| `font.render_text_binary.json` | 9 |
| `font.text_bbox.json` | 6 |
| `font.transposed_bbox.json` | 7 |
| `font.unsupported_operation.json` | 1 |
| `font.validate_transposed_length.json` | 5 |
| `font.variations.json` | 36 |
| Total | 350 |

## 5. Recommended action order

1. Fix general stroker/stroke-border in `pillow-rs-freetype`.
   - This is the biggest real mismatch between Pillow public behavior and Rust.
   - It blocks `stroke_filled=True`, stroked descender/ascender rows, and removes
     the need for suspect extent clamps.
2. Add public `PIL.ImageFont` rows for stroke:
   - normal successful `stroke_width`;
   - `stroke_filled=True`;
   - `mode="1"` stroke;
   - descenders/ascenders like `jQ`;
   - clipped top/bottom/left/right stroked glyphs.
3. Add embedded bitmap/sbit public rows only when coverage proves `sbit.rs`
   execution.
4. Add bitmap PILfont edge rows for distinct public loader/render behavior.
5. Add charmap/encoding rows for real symbol/non-default charmap fonts.
6. Add rare FreeType error rows only from real fonts that Pillow itself rejects
   with the target error.
7. Re-run:
   - `make -C pillow-rs font-tests`
   - Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`
8. Update this document with the new run ID, snapshot ID, changed file metrics,
   and exact rows added.

## Final decision statement

Trusted today:

- The active 350-row `PIL.ImageFont` fixture corpus has exact runtime parity
  against live Pillow 12.2.0.
- The public Rust Font wrapper file `pillow-rs/src/font/mod.rs` is fully covered
  by this suite.

Not trusted today:

- Full `PIL.ImageFont` parity.
- Successful RAQM shaping.
- General stroke/stroke-border behavior.
- Suspect stroked extent-clamp logic.
- Complete bitmap PILfont loader/render region coverage.
- Embedded bitmap/color glyph behavior.
- Full charmap/encoding behavior.
- Exhaustive rare FreeType error mapping through public inputs.

The next implementation decision should be whether to prioritize general
stroker/stroke-border parity. That is the clearest high-value path toward both
better coverage and more truthful `PIL.ImageFont` compatibility.

# ImageFont decision gap review

Date: 2026-07-27

Target: Pillow `PIL.ImageFont` 12.2.0 parity against Rust `pillow-rs`.

Current evidence base:

- Pillow oracle: repo-local `.oracle-venv`, Pillow 12.2.0, `PIL.ImageFont.py`, native `PIL._imagingft` for `FreeTypeFont` rows.
- Rust public surface: `pillow-rs/src/lib.rs`, `pillow-rs/src/font/mod.rs`, `pillow-rs/src/font/pilfont.rs`, `pillow-rs/src/font/imagingft.rs`.
- Lower Rust FreeType implementation used by ImageFont: `pillow-rs-freetype/src/**`.
- Active tests: `pillow-rs/tests/font_public_api.rs` plus input-only rows under `pillow-rs/tests/fixtures/font/inputs/public-api`.
- Coverage MCP suite: `font-with-freetype`.
- Coverage MCP run: `649787d0-4488-4d12-9cef-d610bf8cc124`.
- Coverage MCP snapshot: `33772692-59a3-46aa-9471-0c48db9437c0`.

The defensible current claim is:

> The active ImageFont fixture rows match live Pillow 12.2.0 exactly.

The non-defensible current claim is:

> Rust has complete `PIL.ImageFont` parity.

That second claim is not true yet. Coverage and implementation review both show remaining gaps.

## 1. Uncovered-line logic-based analysis

### Direct Font files

Coverage for direct `pillow-rs/src/font` files in snapshot `33772692-59a3-46aa-9471-0c48db9437c0`:

| File | Lines | Branches | Functions | Regions | Decision status |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | Covered. |
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | Covered at the Rust adapter level. |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | Not fully trusted. Bitmap `ImageFont.ImageFont` behavior has parity rows, but function/region coverage says additional bitmap paths remain unproven. |
| `pillow-rs/src/font/imagingft.rs` | 1642/1666 98.56% | 246/254 96.85% | 163/174 93.68% | 2547/2645 96.29% | Not fully trusted. FreeType-backed ImageFont has reachable behavior gaps. |

### `imagingft.rs` uncovered/partial logic

| Line(s) | Logic | Why it matters | Decision |
|---:|---|---|---|
| `91`, `92`, `253`, `271` | FreeType error table entries and table-miss behavior. | Pillow `_imagingft.c::geterror` maps FreeType errors into public `OSError` messages. Rust has the table, but not every rare status is reached by a public `PIL.ImageFont` input. | Do not add private table unit tests as parity proof. Add only public ImageFont rows that naturally trigger these errors. |
| `796` | LLVM/source mapping around FFI constants/helper declarations. | This does not represent product behavior. | Treat as coverage artifact unless Coverage MCP source context later proves otherwise. |
| `826`, `829` | `floor26` / `ceil26` 26.6 fixed-point conversion. | Pillow C uses pixel rounding/floor/ceil behavior for bbox and mask placement. Partial coverage can hide off-by-one bbox/mask errors. | Add public rows with negative bearings, fractional starts, descenders, ascenders, and glyphs/fonts that cross floor/ceil boundaries. |
| `928` | BASIC bbox/glyph-run path with load flags. | BASIC layout is shared by `getlength`, `getbbox`, `getmask`, and `getmask2`. Partial coverage means not every load-flag path is independently proven. | Add minimal normal-vs-mono rows across bbox/mask endpoints. Avoid duplicates; each row should prove an independent branch. |
| `1094`, `1097`, `1099` | Stroked bitmap extent clamp. | This is Rust-only compatibility logic. Pillow allocates from `_imagingft.c::bounding_box_and_anchors` and clips during render; it does not obviously mutate extents this way. | Suspect implementation. Keep visible until lower stroker/bbox parity is fixed, then remove or prove by C/Pillow trace. |
| `1193`, `1194` | `stroke_filled=true` path to `FT_Outline_Glyph_StrokeBorder`. | Pillow supports `stroke_filled` through `getmask2(..., stroke_filled=True)`. Rust routes the option, but successful real-glyph stroke-border output is not proven. | Blocked by incomplete lower stroker implementation. Add success rows only after general stroker support works. |

### Lower `pillow-rs-freetype` coverage under ImageFont

`imagingft.rs` is not the whole implementation. It delegates glyph loading, metrics, hinting, rasterization, bitmap tables, and stroking into `pillow-rs-freetype`. Current ImageFont coverage leaves major lower paths untrusted:

| File | Line coverage | Region coverage | ImageFont risk |
|---|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 13.05% | 12.10% | High. Contains handle, glyph, charmap, bitmap, and stroker routes reached by ImageFont. |
| `pillow-rs-freetype/src/api.rs` | 17.54% | 15.83% | High. Core public fontdone API under ImageFont. |
| `pillow-rs-freetype/src/font.rs` | 26.67% | 25.79% | High. Face loading, glyph machinery, metrics, layout. |
| `pillow-rs-freetype/src/render.rs` | 39.24% | 39.13% | High. Pixel output parity risk. |
| `pillow-rs-freetype/src/scaler.rs` | 60.06% | 63.93% | Medium/high. Scaling and hinted metrics. |
| `pillow-rs-freetype/src/grays.rs` | 69.04% | 77.22% | Medium. Antialias rasterizer. |
| `pillow-rs-freetype/src/tt/sbit.rs` | 12.29% | 14.66% | High for embedded bitmap/color font paths. |
| `pillow-rs-freetype/src/tt/cmap.rs` | 33.50% | 36.27% | High for character mapping and byte/unicode input behavior. |
| `pillow-rs-freetype/src/tt/glyf.rs` | 31.93% | 31.56% | High for TrueType outline behavior. |
| `pillow-rs-freetype/src/tt/cff.rs` | 48.30% | 46.64% | High for CFF/OpenType fonts. |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 48.49% | 41.77% | High for hinted TrueType behavior. |
| `pillow-rs-freetype/src/autohint/latin.rs` | 67.12% | 65.51% | Medium/high for fallback hinting. |
| `pillow-rs-freetype/src/autohint/cjk.rs` | 45.05% | 45.00% | High for CJK fonts. |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 0.00% | 0.00% | Unproven horizontal device metrics. |
| `pillow-rs-freetype/src/tt/mvar.rs` | 0.00% | 0.00% | Unproven variation metric deltas. |
| `pillow-rs-freetype/src/tt/vhea.rs` | 0.00% | 0.00% | Unproven vertical metrics. |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 0.00% | 0.00% | Unproven vertical metrics. |

This means the ImageFont parity suite is strong for the active rows, but not yet strong enough to trust the whole implementation.

## 2. Pillow ImageFont vs Rust implementation review

### Pillow 12.2.0 public surface

Pillow `PIL.ImageFont` exposes these public behaviors:

| Pillow area | Public surface | Rust status |
|---|---|---|
| Module constructors | `load`, `load_path`, `load_default_imagefont`, `load_default`, `truetype` | Partially modeled. Rust core uses bytes/options, while path search and stream loading remain binding/test-harness concerns. That is architecturally correct only if bindings stay thin. |
| Bitmap font class | `ImageFont.ImageFont.getmask`, `getbbox`, `getlength`, `info` | Implemented as Rust `PilFont`, not as the same class shape. Active rows pass, but `pilfont.rs` is below 100% region/function coverage. |
| FreeType font class | `FreeTypeFont.__init__`, `getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `getmask2`, `font_variant`, `get_variation_names`, `set_variation_by_name`, `get_variation_axes`, `set_variation_by_axes` | Mostly implemented through Rust `ImageFont`. BASIC layout rows pass. Successful libraqm shaping is excluded. `stroke_filled=true` is wired but not proven for successful output. |
| Transposed wrapper | `TransposedFont.getmask`, `getbbox`, `getlength` | Rust exposes helper operations, not a class. Behavior rows exist. Class-shape parity is not implemented. |
| Layout enum | `Layout.BASIC`, `Layout.RAQM` | BASIC implemented. RAQM success unsupported; no-libraqm error parity is tested. |

### Rust helper operations that are not Pillow public methods

These Rust/test operations are adapters, not independent Pillow APIs:

- `getbbox_binary`
- `getmask2_with_start`
- `get_transposed_mask`
- `transposed_bbox`
- `validate_transposed_length`
- `text_bbox`
- `render_text`
- `render_text_binary`

Decision: keep them only if they are explicitly treated as helper/consumer paths proving public Pillow behavior. They must not become a separate compatibility target or hide a mismatch with `PIL.ImageFont`.

## 3. Missing or wrong Rust implementation across files

### A. General stroker support is incomplete

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/handles.rs`

Current Rust behavior:

- `imagingft.rs` routes stroked rendering through `FT_Outline_Glyph_Stroke` or `FT_Outline_Glyph_StrokeBorder`.
- The lower FreeType-compatible implementation currently has a DejaVu glyph-36 fixture-specific `FT_Outline_Glyph_Stroke` success path.
- General real-glyph outline parsing/export through `FT_Stroker_ParseOutline` is incomplete for conic/cubic/closed glyphs.
- Exploratory public rows such as stroked `"jQ"` showed Pillow succeeds while Rust fails with `FT_Err_Unimplemented_Feature`.

Decision:

- This is a real implementation gap, not only a coverage gap.
- Do not add more glyph-specific shortcuts.
- Fix lower `pillow-rs-freetype` stroker support first.
- After that, add public ImageFont rows for stroked ascenders, descenders, mono mode, empty text, negative stroke, and `stroke_filled=true`.

### B. Stroked extent clamp is suspect Rust-only logic

File:

- `pillow-rs/src/font/imagingft.rs`

Current Rust behavior:

- Rust clamps stroked `x_max`/`y_max` when actual stroked bitmap extents exceed bbox-derived target dimensions.

Pillow behavior:

- Pillow `_imagingft.c` computes allocation from `bounding_box_and_anchors` and clips rendered writes to that target.
- The reviewed Pillow path does not clearly show Rust-style post-stroke extent mutation.

Decision:

- Treat this as a temporary compatibility shim caused by lower stroker/bbox mismatch.
- Once general stroker support works, either remove this logic or prove it with C/Pillow trace evidence and exact fixture rows.

### C. Successful libraqm shaping is not implemented

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs/src/error.rs`
- `pillow-rs/tests/support/font_runner.rs`

Current Rust behavior:

- `direction`, `features`, and `language` return a dedicated internal `PilError::UnsupportedLibraqm`.
- Public parity rows map this to Pillow's no-libraqm `KeyError` payload.

Pillow behavior:

- Pillow supports successful RAQM shaping when built with libraqm.
- In the current no-libraqm oracle, Pillow errors for those arguments.

Decision:

- Current no-libraqm error parity is correct.
- Full `PIL.ImageFont` parity cannot be claimed while successful RAQM shaping is out of scope.
- If product scope remains “everything except libraqm,” keep this as a documented exclusion and hard-code the dedicated unsupported error.

### D. Bitmap ImageFont class shape differs from Pillow

Files:

- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/lib.rs`

Current Rust behavior:

- Bitmap fonts are represented by `PilFont`.
- FreeType fonts are represented by `ImageFont`.

Pillow behavior:

- Bitmap fonts use `PIL.ImageFont.ImageFont`.
- FreeType fonts use `PIL.ImageFont.FreeTypeFont`.

Decision:

- If the goal is behavioral parity only, this split can stay.
- If the goal is public API shape parity, Rust should expose a clearer `ImageFont`/`FreeTypeFont` distinction or a wrapper matching Pillow class semantics.
- Either way, `pilfont.rs` must reach trusted coverage through public Pillow rows.

### E. Path/search/stream behavior is not core-owned

Files:

- `pillow-rs/src/lib.rs`
- `pillow-rs-py/src/lib.rs`
- `pillow-rs-js/src/lib.rs`
- `pillow-rs/tests/support/font_runner.rs`

Current Rust architecture:

- Core accepts bytes and typed options.
- Bindings/tests own file loading and path handling.

Pillow behavior:

- `truetype` accepts paths and file-like objects.
- `load_path` searches `sys.path`.
- `truetype` has platform-specific fallback search behavior after `OSError`.

Decision:

- Keeping filesystem/search outside core is correct.
- Binding crates must stay thin: load bytes, pass options, return Rust result payloads.
- Bindings must not implement font parsing, layout, rendering, or parity comparison logic.

### F. Embedded bitmap/color, variation metrics, and vertical/device metrics are untrusted

Files:

- `pillow-rs-freetype/src/tt/sbit.rs`
- `pillow-rs-freetype/src/tt/hdmx.rs`
- `pillow-rs-freetype/src/tt/mvar.rs`
- `pillow-rs-freetype/src/tt/vhea.rs`
- `pillow-rs-freetype/src/tt/vmtx.rs`

Current Rust status:

- These files have low or zero ImageFont-suite coverage.
- They can affect public `PIL.ImageFont` output through metrics, glyph selection, embedded bitmap strikes, and variation coordinates.

Decision:

- Add public ImageFont rows that naturally exercise each table, or explicitly mark the table irrelevant to the supported public scope.
- Do not count FreeType-only unit tests as ImageFont parity proof.

### G. Error table completeness is not enough

File:

- `pillow-rs/src/font/imagingft.rs`

Current Rust status:

- Rust has a broad FreeType error message table matching Pillow `_imagingft.c::geterror`.
- Rare table rows are not reached by public ImageFont fixtures.

Decision:

- Keep the table.
- Add only public fixture rows that produce the corresponding Pillow error through `PIL.ImageFont`.
- Do not add private unit tests and claim public parity from them.

## 4. Final action list for decision

1. Complete general `FT_Stroker_ParseOutline` and outline export in `pillow-rs-freetype`; remove the DejaVu glyph-36-only stroke shortcut once real parity rows pass.
2. Re-evaluate and preferably remove the stroked extent clamp in `imagingft.rs`; keep it only if C/Pillow tracing proves identical behavior.
3. Keep `PilError::UnsupportedLibraqm` as the hard-coded no-libraqm scope behavior, and do not claim libraqm success parity.
4. Add minimal input-only public ImageFont rows for:
   - `stroke_filled=true` successful output;
   - stroked descenders/ascenders;
   - mono stroked rendering;
   - negative/empty stroke edge cases;
   - bbox floor/ceil rounding edges;
   - normal-vs-mono load flag differences;
   - embedded bitmap strike behavior;
   - variation metrics and metric-delta behavior;
   - reachable FreeType table errors.
5. Drive all new trust through live Pillow 12.2.0 oracle rows, not stored output JSON and not Rust self-comparison.
6. Use Coverage MCP after each meaningful slice and update this document only with new run/snapshot IDs.
7. Do not remove or reduce parity standards until this list is explicitly resolved or consciously excluded.

## 5. Immediate decision point

The highest-value next implementation task is lower stroker parity in `pillow-rs-freetype`.

Reason:

- It is a known public Pillow-success/Rust-failure gap.
- It blocks successful `stroke_filled=true`.
- It blocks trusted stroked descender/height-clamp coverage.
- It likely determines whether the Rust-only stroked extent clamp should be deleted.

If the project goal is “PIL.ImageFont except libraqm,” this should be fixed before spending more time on duplicate fixture expansion.

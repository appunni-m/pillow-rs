# Glyph Load And Render C API/ABI Slice

Scope: glyph loading, glyph slots, render modes, load flags, glyph objects, and
the render-mode parity gap. C FreeType is the oracle. Servo `rust-freetype` is
only a binding/reference checklist. The current `fontdone` safe Rust API is a
semantic layer, not ABI proof.

## In-Scope C Surface

| Symbol or type | C header area | ABI requirement | Current fontdone mapping | Plan |
|---|---|---|---|---|
| `FT_Load_Glyph(FT_Face, FT_UInt, FT_Int32)` | `freetype.h` | Export exact symbol, return `FT_Error`, mutate `face->glyph`, preserve `FT_Int32 load_flags` bits exactly. | `Face::load_glyph(glyph_index, LoadFlags) -> Result<GlyphSlot, FontError>` returns an immutable snapshot. | Keep safe API, add ABI layer that stores a mutable slot in `FT_FaceRec::glyph` and maps errors to exact `FT_Error`. |
| `FT_Load_Char(FT_Face, FT_ULong, FT_Int32)` | `freetype.h` | Resolve char code through active charmap, then call the same load pipeline as `FT_Load_Glyph`. | `Face::load_char(char_code, LoadFlags)` delegates through `get_char_index`. | ABI layer must preserve `FT_ULong` input width, charmap behavior, missing-glyph index `0`, and same side effects on the glyph slot. |
| `FT_Render_Glyph(FT_GlyphSlot, FT_Render_Mode)` | `freetype.h` | Render an existing slot according to numeric `FT_Render_Mode`; set `slot->format` to bitmap and fill `FT_Bitmap` fields. | Rendering is coupled to load via `Face::load_glyph(..., LoadFlags::RENDER)` or `Font::render_char_mode_for_index`. | Split load and render in the core/safe API so ABI `FT_Load_Glyph(..., no RENDER)` followed by `FT_Render_Glyph(slot, mode)` works without reloading. |
| `FT_Get_Glyph(FT_GlyphSlot, FT_Glyph*)` | `ftglyph.h` | Allocate/copy the current slot image into a heap glyph object; caller owns result. | No heap glyph object API. Current values are direct Rust structs. | Add pure-Rust owned glyph enum/record for outline and bitmap glyphs, then wrap it in ABI-compatible `FT_Glyph` handles. |
| `FT_Done_Glyph(FT_Glyph)` | `ftglyph.h` | Free heap glyph object created by `FT_Get_Glyph`, `FT_New_Glyph`, copies, or conversions. | Rust drop only. | ABI layer owns allocation table/boxed glyph records and releases on `FT_Done_Glyph`. |
| `FT_Glyph_Copy(FT_Glyph, FT_Glyph*)` | `ftglyph.h` | Deep-copy glyph object and return independently owned handle. | No mapping. | Implement clone for owned outline/bitmap glyphs; preserve format and advance. |
| `FT_Glyph_Transform(FT_Glyph, FT_Matrix*, FT_Vector*)` | `ftglyph.h` | Transform outline glyph coordinates and advance; translate bitmap glyph origin as C does. | No mapping. | Add transform operations for owned glyphs. Compare transformed cbox/bbox and advance to C. |
| `FT_Glyph_To_Bitmap(FT_Glyph*, FT_Render_Mode, FT_Vector*, FT_Bool)` | `ftglyph.h` | Convert outline glyph object to bitmap glyph in requested render mode; optionally destroy original. | No mapping. | Reuse the split render path for owned glyphs; preserve destroy semantics and translation origin. |
| `FT_Glyph_Get_CBox(FT_Glyph, FT_UInt, FT_BBox*)` | `ftglyph.h` | Support exact bbox modes and rounding rules. | Scaler has cbox/bbox-style values; no glyph-object API. | Add bbox mode constants and fixture comparisons for all supported owned glyph formats. |
| `FT_GlyphSlotRec` | `freetype.h` | `#[repr(C)]` field order, public field names, pointer ownership, bitmap/outline union semantics, `advance`, deltas, and format. | `GlyphSlot { glyph_index, metrics, advance, format, bitmap, bitmap_left, bitmap_top }`. | Keep safe snapshot; add C ABI record with exact `FT_GlyphSlotRec` layout and stable storage owned by face. |
| `FT_Glyph_Metrics` | `freetype.h` | Eight `FT_Pos` fields in C order, all 26.6 pixel units unless no-scale semantics apply. | `GlyphSlotMetrics` has the same semantic fields, not proven ABI layout. | Add `#[repr(C)] FT_Glyph_Metrics` and exact field-order tests. |
| `FT_Bitmap` | `ftimage.h` | Exact rows, width, pitch, buffer pointer, num_grays, pixel_mode, palette fields. | `RenderedBitmap { width, rows, pitch, pixel_mode, num_grays, left, top, buffer }`. | Add ABI bitmap record separate from Rust-owned buffer and verify pointer/lifetime rules. |
| `FT_Outline` | `ftimage.h` | Exact contours, points, tags, flags, and 26.6 coordinates. | Internal `Outline` type. | Expose through ABI slot/glyph records only after field layout and point/tag parity are tested. |

## Numeric Constant Exactness

The ABI layer must use the C numeric values exactly. The current safe
`LoadFlags` values in `src/api.rs` are compact semantic bits and must not be
exported as `FT_LOAD_*`.

| Constant | C value | Current fontdone safe mapping | Requirement |
|---|---:|---|---|
| `FT_LOAD_DEFAULT` | `0x00000000` | `LoadFlags::DEFAULT = 0` | Exact. |
| `FT_LOAD_NO_SCALE` | `0x00000001` | Missing. | Add ABI constant and behavior plan. |
| `FT_LOAD_NO_HINTING` | `0x00000002` | `LoadFlags::NO_HINTING = 1 << 2` (`0x4`) | ABI must use C value, not safe API value. |
| `FT_LOAD_RENDER` | `0x00000004` | `LoadFlags::RENDER = 1 << 0` (`0x1`) | ABI must use C value, not safe API value. |
| `FT_LOAD_NO_BITMAP` | `0x00000008` | Missing. | Add constant; current outline path effectively has no embedded bitmap support but must expose exact bit. |
| `FT_LOAD_VERTICAL_LAYOUT` | `0x00000010` | Missing. | Planned; affects advance vector and slot metrics. |
| `FT_LOAD_FORCE_AUTOHINT` | `0x00000020` | `LoadFlags::FORCE_AUTOHINT = 1 << 1` (`0x2`) | ABI must use C value, not safe API value. |
| `FT_LOAD_CROP_BITMAP` | `0x00000040` | Missing. | Planned/legacy semantics. |
| `FT_LOAD_PEDANTIC` | `0x00000080` | Missing. | Planned error-strictness behavior. |
| `FT_LOAD_ADVANCE_ONLY` | `0x00000100` | Missing. | Internal C flag; classify explicitly. |
| `FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH` | `0x00000200` | Missing. | Planned/classified. |
| `FT_LOAD_NO_RECURSE` | `0x00000400` | Missing. | Planned; composite slot format/metrics behavior. |
| `FT_LOAD_IGNORE_TRANSFORM` | `0x00000800` | Missing. | Planned once transforms exist. |
| `FT_LOAD_MONOCHROME` | `0x00001000` | Missing; `TARGET_MONO` is used instead. | Add exact constant and C interaction with `FT_LOAD_RENDER`. |
| `FT_LOAD_LINEAR_DESIGN` | `0x00002000` | Missing. | Planned; linear metrics output. |
| `FT_LOAD_SBITS_ONLY` | `0x00004000` | Missing. | Planned/unsupported until embedded bitmap support. |
| `FT_LOAD_NO_AUTOHINT` | `0x00008000` | Missing. | Planned; must differ from default/force autohint. |
| `FT_LOAD_COLOR` | `0x00100000` | Missing. | Planned/unsupported until color glyphs. |
| `FT_LOAD_COMPUTE_METRICS` | `0x00200000` | Missing. | Planned for bitmap metrics path. |
| `FT_LOAD_BITMAP_METRICS_ONLY` | `0x00400000` | Missing. | Planned for embedded bitmap path. |
| `FT_LOAD_SVG_ONLY` | `0x00800000` | Missing. | Internal C flag; classify explicitly. |
| `FT_LOAD_NO_SVG` | `0x01000000` | Missing. | Planned/unsupported until SVG glyphs. |

`FT_LOAD_TARGET_(x)` is `((x & 15) << 16)`. Exact target constants:

| Constant | C value | Current fontdone safe mapping | Requirement |
|---|---:|---|---|
| `FT_LOAD_TARGET_NORMAL` | `0x00000000` | Default render mode. | Exact. |
| `FT_LOAD_TARGET_LIGHT` | `0x00010000` | Missing. | Add `RenderMode::Light` or a separate load target; C render mode LIGHT renders like NORMAL but selects light hinting. |
| `FT_LOAD_TARGET_MONO` | `0x00020000` | `LoadFlags::TARGET_MONO = 1 << 3` (`0x8`) | ABI must use C value. |
| `FT_LOAD_TARGET_LCD` | `0x00030000` | `LoadFlags::TARGET_LCD = 1 << 4` (`0x10`) | ABI must use C value. |
| `FT_LOAD_TARGET_LCD_V` | `0x00040000` | `LoadFlags::TARGET_LCD_V = 1 << 5` (`0x20`) | ABI must use C value. |

`FT_LOAD_TARGET_MODE(flags)` must decode `((flags >> 16) & 15)` to the exact
`FT_Render_Mode` value. Invalid/multiple target combinations must be classified
against C FreeType rather than silently normalized.

## Render Modes And Glyph Formats

| C enum | C value | Current fontdone mapping | Gap |
|---|---:|---|---|
| `FT_RENDER_MODE_NORMAL` | `0` | `RenderMode::Normal`, `PixelMode::Gray`. | Present. |
| `FT_RENDER_MODE_LIGHT` | `1` | Missing. | Required. C documents it as equivalent to normal rendering, but it is also a load-target selector for light hinting. |
| `FT_RENDER_MODE_MONO` | `2` | `RenderMode::Mono`, `PixelMode::Mono`. | Present semantically; ABI numeric missing. |
| `FT_RENDER_MODE_LCD` | `3` | `RenderMode::Lcd`, `PixelMode::Lcd`. | Present semantically; ABI numeric missing. |
| `FT_RENDER_MODE_LCD_V` | `4` | `RenderMode::LcdV`, `PixelMode::LcdV`. | Present semantically; ABI numeric missing. |
| `FT_RENDER_MODE_SDF` | `5` | Missing. | Required as missing/unsupported with exact error until implemented. |
| `FT_RENDER_MODE_MAX` | `6` | Missing. | Required for enum exactness. |

| C glyph format | C value | Current fontdone mapping | Gap |
|---|---:|---|---|
| `FT_GLYPH_FORMAT_NONE` | `0x00000000` | `GlyphFormat::None`. | ABI numeric missing. |
| `FT_GLYPH_FORMAT_COMPOSITE` | `0x636f6d70` (`'comp'`) | Missing. | Required for `FT_LOAD_NO_RECURSE` and composite slot behavior. |
| `FT_GLYPH_FORMAT_BITMAP` | `0x62697473` (`'bits'`) | `GlyphFormat::Bitmap`. | ABI numeric missing. |
| `FT_GLYPH_FORMAT_OUTLINE` | `0x6f75746c` (`'outl'`) | `GlyphFormat::Outline`. | ABI numeric missing. |
| `FT_GLYPH_FORMAT_PLOTTER` | `0x706c6f74` (`'plot'`) | Missing. | Classify as unsupported unless a plotter renderer is added. |
| `FT_GLYPH_FORMAT_SVG` | `0x53564720` (`'SVG '`) | Missing. | Required as missing/unsupported until SVG glyphs. |

`FT_Glyph_Get_CBox` also needs exact bbox mode constants:
`FT_GLYPH_BBOX_UNSCALED = 0`, `FT_GLYPH_BBOX_SUBPIXELS = 0`,
`FT_GLYPH_BBOX_GRIDFIT = 1`, `FT_GLYPH_BBOX_TRUNCATE = 2`, and
`FT_GLYPH_BBOX_PIXELS = 3`.

## Current Fontdone Behavior To Preserve As Inputs

- `Face::load_char` maps the character code to a glyph index with
  `get_char_index` and delegates to `Face::load_glyph`.
- `Face::load_glyph` selects metrics from default/native TrueType,
  force-autohint, or no-hinting paths.
- `LoadFlags::RENDER` renders during load; this does not yet model
  `FT_Render_Glyph` as a separate operation on an existing slot.
- `NO_HINTING | RENDER` renders an unhinted outline and has an exact public
  parity row. Unknown raw `FT_LOAD_*` bits are rejected by the FFI flag parser
  before constructing core `LoadFlags`.
- `LoadFlags::render_mode` chooses mono, LCD, LCD_V, else normal. It does not
  support LIGHT, SDF, `FT_LOAD_MONOCHROME`, or invalid target diagnostics.
- `GlyphSlot::new` sets format to bitmap if rendered, none for zero-width and
  zero-height metrics, otherwise outline. C slot format is the loaded image
  format and must be verified independently for empty outlines, composites,
  embedded bitmaps, color/SVG glyphs, and no-recurse loads.
- `RenderedBitmap` carries FreeType-like bitmap dimensions, pitch, pixel mode,
  bearings, and bytes, but it is not `FT_Bitmap` layout-compatible.

## Required Output Comparisons

Every promoted symbol must have exact comparisons against the pinned C oracle.

| Surface | Required comparisons |
|---|---|
| `FT_Load_Glyph` / `FT_Load_Char` metrics | `FT_GlyphSlotRec::metrics.{width,height,horiBearingX,horiBearingY,horiAdvance,vertBearingX,vertBearingY,vertAdvance}`, `advance.x`, `advance.y`, `linearHoriAdvance`, `linearVertAdvance` when exposed, `lsb_delta`, `rsb_delta`, and public error code. |
| Glyph slot outline | `slot->format`, outline point count, contour endpoints, point tags, flags, and exact 26.6 coordinates after load target, hinting, transforms, and no-scale/no-hinting modes. |
| Glyph slot bitmap | `bitmap_left`, `bitmap_top`, `FT_Bitmap.rows`, `width`, `pitch`, `pixel_mode`, `num_grays`, and exact byte buffer contents. For mono/LCD, compare packed bytes and pitch, not only expanded pixels. |
| Render modes | For normal/light/mono/LCD/LCD_V/SDF: exact mode dispatch, output pixel mode, dimensions, bearings, pitch, byte buffer/hash, and errors for unsupported modes. LIGHT must compare both direct render and load-target behavior. |
| Load flags | Exact behavior for individual flags and meaningful combinations: default, render, no hinting, force autohint, target modes, monochrome, no bitmap, no recurse, vertical layout, linear design, no scale, color/SVG flags, and invalid combinations. |
| Heap glyph objects | `FT_Get_Glyph` copy from slot, deep copy identity for `FT_Glyph_Copy`, transform matrix/delta effects, `FT_Glyph_To_Bitmap` conversion bytes and origin, `destroy` ownership behavior, and `FT_Done_Glyph` lifetime/error safety. |
| CBox/BBox | Exact `FT_Glyph_Get_CBox` results for unscaled/subpixels, gridfit, truncate, and pixels modes across outline and bitmap glyphs, including negative coordinates and empty glyphs. |

Do not promote a semantic wrapper result as ABI compatibility unless the exact
C record layout, numeric constants, side effects, and output bytes have a
fixture or scalar test.

## Render-Mode Matrix Gap

`tests/render_mode_matrix.rs` is a static 16-row fixture matrix and currently
reports `16/16` for normal, mono, LCD, and LCD_V. That is useful coverage but
not complete C API parity:

- It exercises `Font::render_char_mode`, not the C sequence
  `FT_Load_Glyph` followed by `FT_Render_Glyph` on the existing slot.
- It does not cover `FT_RENDER_MODE_LIGHT`, `FT_RENDER_MODE_SDF`, invalid
  render modes, or `FT_LOAD_MONOCHROME`.
- It does not prove exact `FT_LOAD_TARGET_*` numeric decoding because the safe
  `LoadFlags` values are not C values.
- It covers a small static font/glyph set; it should remain a visible baseline
  and should not be weakened or relabeled as full render-mode ABI parity.

The ABI plan is to keep this matrix as an existing guard, then add C ABI
fixtures that compare the actual `FT_Load_Glyph`/`FT_Render_Glyph` call
sequence, slot mutation, bitmap record fields, and exact bytes for every render
mode and load-target combination.

# Outline And Bitmap C API/ABI Slice

Owner: Worker 6, outline and bitmap APIs.

Scope: plan the pure-Rust `fontdone` path to C-compatible FreeType outline and
bitmap behavior. This is a compatibility slice, not proof that the current safe
Rust API is ABI-compatible.

## C Symbols

Outline symbols from `ftoutln.h` and `ftbbox.h`:

| Symbol | C shape | Status in `tests/data/interface_map.json` | Current `fontdone` mapping |
|---|---|---|---|
| `FT_Outline_New` | `FT_Error (FT_Library library, FT_UInt numPoints, FT_Int numContours, FT_Outline* anoutline)` | planned | None. Needs owned outline allocation for future C ABI layer. |
| `FT_Outline_Done` | `FT_Error (FT_Library library, FT_Outline* outline)` | planned | None. Rust drops `Outline` values directly today. |
| `FT_Outline_Copy` | `FT_Error (const FT_Outline* source, FT_Outline* target)` | planned | `Outline: Clone` is semantic only; no ABI record copy yet. |
| `FT_Outline_Translate` | `void (const FT_Outline* outline, FT_Pos xOffset, FT_Pos yOffset)` | partial | Internal translation during scaling/rendering: `scaler::scale_glyph`, `render::translate_outline`. |
| `FT_Outline_Transform` | `void (const FT_Outline* outline, const FT_Matrix* matrix)` | planned | Scaling applies transform-like 16.16 multiplication, but no public matrix transform endpoint. |
| `FT_Outline_Embolden` | `FT_Error (FT_Outline* outline, FT_Pos strength)` | planned | None. |
| `FT_Outline_EmboldenXY` | `FT_Error (FT_Outline* outline, FT_Pos xstrength, FT_Pos ystrength)` | planned | None. |
| `FT_Outline_Reverse` | `void (FT_Outline* outline)` | planned | None. |
| `FT_Outline_Check` | `FT_Error (FT_Outline* outline)` | planned | Validation exists only as parser/rasterizer errors such as `FontError::InvalidOutline`. |
| `FT_Outline_Get_CBox` | `void (const FT_Outline* outline, FT_BBox* acbox)` | partial | `scaler::ScaledGlyph::{outline_cbox_*}` and local render cbox helpers. |
| `FT_Outline_Get_BBox` | `FT_Error (FT_Outline* outline, FT_BBox* abbox)` | planned | `scaler::ScaledGlyph::{outline_bbox_*}` stores exact outline bbox, but no public C-shaped call. |
| `FT_Outline_Decompose` | `FT_Error (FT_Outline* outline, const FT_Outline_Funcs* func_interface, void* user)` | partial | Private decomposers in `grays.rs` and `render.rs` walk contours for rasterization. |
| `FT_Outline_Get_Orientation` | `FT_Orientation (FT_Outline* outline)` | planned | Autohinter computes winding internally; no public orientation endpoint. |
| `FT_Outline_Get_Bitmap` | `FT_Error (FT_Library library, FT_Outline* outline, const FT_Bitmap* abitmap)` | planned | Rendering currently returns owned `RenderedBitmap`; no caller-supplied `FT_Bitmap` target. |
| `FT_Outline_Render` | `FT_Error (FT_Library library, FT_Outline* outline, FT_Raster_Params* params)` | planned | `grays::rasterize` and render-mode paths exist, but no `FT_Raster_Params` ABI surface. |

Bitmap symbols from `ftbitmap.h`:

| Symbol | C shape | Status in `tests/data/interface_map.json` | Current `fontdone` mapping |
|---|---|---|---|
| `FT_Bitmap_Init` | `void (FT_Bitmap* abitmap)` | partial | `GlyphMask`/`RenderedBitmap` constructors produce initialized Rust values. |
| `FT_Bitmap_New` | `void (FT_Bitmap* abitmap)` | planned | Same zero-initialization semantics as `FT_Bitmap_Init` needed for C ABI. |
| `FT_Bitmap_Done` | `FT_Error (FT_Library library, FT_Bitmap* bitmap)` | partial | Rust drops owned buffers; no library allocator-aware ABI call. |
| `FT_Bitmap_Copy` | `FT_Error (FT_Library library, const FT_Bitmap* source, FT_Bitmap* target)` | planned | `RenderedBitmap: Clone` is semantic only; no C allocator copy. |
| `FT_Bitmap_Convert` | `FT_Error (FT_Library library, const FT_Bitmap* source, FT_Bitmap* target, FT_Int alignment)` | partial | `render::unpack_mono_row` covers mono unpacking only. |
| `FT_Bitmap_Embolden` | `FT_Error (FT_Library library, FT_Bitmap* bitmap, FT_Pos xStrength, FT_Pos yStrength)` | planned | None. |
| `FT_Bitmap_Blend` | `FT_Error (FT_Library library, const FT_Bitmap* source, const FT_Vector source_offset, FT_Bitmap* target, FT_Vector* atarget_offset, FT_Color color)` | planned | None. |

Related records and constants in this slice:

- `FT_Outline`, `FT_Vector`, `FT_BBox`, `FT_Matrix`, `FT_Outline_Funcs`,
  `FT_Raster_Params`, `FT_Bitmap`, `FT_Pixel_Mode`, `FT_GlyphSlotRec`,
  `FT_Library`, `FT_Error`, `FT_Orientation`, `FT_Color`.
- Pixel mode constants requiring exact numeric checks:
  `FT_PIXEL_MODE_NONE`, `FT_PIXEL_MODE_MONO`, `FT_PIXEL_MODE_GRAY`,
  `FT_PIXEL_MODE_GRAY2`, `FT_PIXEL_MODE_GRAY4`, `FT_PIXEL_MODE_LCD`,
  `FT_PIXEL_MODE_LCD_V`, `FT_PIXEL_MODE_BGRA`.
- Outline flags requiring exact numeric checks:
  `FT_OUTLINE_NONE`, `FT_OUTLINE_OWNER`, `FT_OUTLINE_EVEN_ODD_FILL`,
  `FT_OUTLINE_REVERSE_FILL`, `FT_OUTLINE_IGNORE_DROPOUTS`,
  `FT_OUTLINE_SMART_DROPOUTS`, `FT_OUTLINE_INCLUDE_STUBS`,
  `FT_OUTLINE_HIGH_PRECISION`, `FT_OUTLINE_SINGLE_PASS`.

## Records Touched

Future C ABI records must be separate from current semantic Rust structs.

| C record | Required ABI fields | Current Rust data | Gap |
|---|---|---|---|
| `FT_Vector` | `x`, `y` as `FT_Pos` | `api::Vector { x: i32, y: i32 }` | Needs `#[repr(C)]`, exact `FT_Pos` alias, and layout test. |
| `FT_BBox` | `xMin`, `yMin`, `xMax`, `yMax` | `font::BBox`, `scaler::ScaledGlyph::*bbox*` | Rust names differ; add ABI record with C field order/names in generated header. |
| `FT_Matrix` | `xx`, `xy`, `yx`, `yy` as 16.16 fixed | Fixed helpers exist | Missing record and public transform endpoint. |
| `FT_Outline` | `n_contours`, `n_points`, `points`, `tags`, `contours`, `flags` | `outline::Outline { n_contours, contours, points, flags, cbox_* }` | Current `OutlinePoint` merges point and on-curve tag; ABI needs split arrays, cubic/conic tags, owner semantics, and no extra cbox fields. |
| `FT_Outline_Funcs` | `move_to`, `line_to`, `conic_to`, `cubic_to`, `shift`, `delta` | Private methods in rasterizers | Needs callback bridge and safe closure API. |
| `FT_Raster_Params` | `target`, `source`, `flags`, `gray_spans`, `black_spans`, `bit_test`, `bit_set`, `user`, `clip_box` | `grays::rasterize*` target slices | Missing caller-supplied target, spans, clipping flags, and callback path. |
| `FT_Bitmap` | `rows`, `width`, `pitch`, `buffer`, `num_grays`, `pixel_mode`, `palette_mode`, `palette` | `render::RenderedBitmap` | Missing `#[repr(C)]`; `palette_mode/palette`; borrowed vs owned buffer rules; signed pitch orientation handling. |
| `FT_GlyphSlotRec` | `bitmap`, `bitmap_left`, `bitmap_top`, `outline`, `format` and adjacent slot fields | `api::GlyphSlot` | Current slot may hold `bitmap: Option<RenderedBitmap>` but does not expose ABI inline bitmap/outline records. |

## Callback And Closure Design For Decompose

Expose two layers:

1. Safe Rust outline walker:

```rust
pub trait OutlineSink {
    type Error;

    fn move_to(&mut self, to: Vector) -> Result<(), Self::Error>;
    fn line_to(&mut self, to: Vector) -> Result<(), Self::Error>;
    fn conic_to(&mut self, control: Vector, to: Vector) -> Result<(), Self::Error>;
    fn cubic_to(&mut self, c1: Vector, c2: Vector, to: Vector) -> Result<(), Self::Error>;
}

pub fn outline_decompose<S: OutlineSink>(
    outline: &Outline,
    shift: i32,
    delta: Vector,
    sink: &mut S,
) -> Result<(), DecomposeError<S::Error>>;
```

2. C ABI bridge:

```rust
#[repr(C)]
pub struct FtOutlineFuncs {
    pub move_to: Option<unsafe extern "C" fn(*const FtVector, *mut c_void) -> FtError>,
    pub line_to: Option<unsafe extern "C" fn(*const FtVector, *mut c_void) -> FtError>,
    pub conic_to: Option<unsafe extern "C" fn(*const FtVector, *const FtVector, *mut c_void) -> FtError>,
    pub cubic_to: Option<unsafe extern "C" fn(*const FtVector, *const FtVector, *const FtVector, *mut c_void) -> FtError>,
    pub shift: i32,
    pub delta: FtPos,
}
```

The safe walker should own the contour state machine now duplicated in
`grays.rs` and `render.rs`. The ABI bridge should adapt `FT_Outline_Funcs` to
the safe sink without calling C FreeType. Callback return values must short
circuit and return the first non-zero `FT_Error`, matching C decompose
behavior. The walker must preserve FreeType's implied-point behavior:
conic-start contours may start at the last on-curve point or midpoint, cubic
start is invalid, consecutive conic controls synthesize midpoint endpoints, and
open contours close to `v_start`.

## Pixel And Byte Parity Outputs

Current exact bitmap evidence:

- `render_mode_matrix` reports 16/16 rows in the interface map.
- `PixelMode::Gray`: `width == pitch`, `num_grays == 256`, one byte per pixel.
- `PixelMode::Mono`: MSB-first packed rows, `num_grays == 2`, pitch
  `(((width + 15) >> 4) << 1)`.
- `PixelMode::Lcd`: horizontal Harmony LCD bytes, `width` is subpixel-tripled,
  pitch padded to 4 bytes with `(width + 3) & !3`.
- `PixelMode::LcdV`: vertical Harmony LCD bytes, `rows` is subpixel-tripled,
  pitch equals width.

Dynamic bitmap tests should compare:

- `rows`, `width`, signed `pitch`, `num_grays`, `pixel_mode`,
  `bitmap_left`, `bitmap_top`, buffer length, and SHA-256 of raw bytes.
- For negative-pitch inputs to `FT_Bitmap_Copy`, `FT_Bitmap_Convert`,
  `FT_Bitmap_Embolden`, and `FT_Bitmap_Blend`, compare row order and output
  pointer interpretation against C FreeType. Current render outputs only use
  positive pitch.
- For `FT_Bitmap_Convert`, cover mono-to-gray unpacking, alignment values
  1/2/4/8, gray-to-gray copy, LCD/LCD_V row alignment, empty bitmaps, and
  invalid pixel modes.
- For `FT_Bitmap_Embolden`, compare dimension growth, origin expectations when
  paired with slot placement, byte expansion for mono/gray/LCD/LCD_V, and
  zero-strength no-op behavior.
- For `FT_Bitmap_Blend`, compare color compositing into `FT_PIXEL_MODE_BGRA`,
  target growth, `atarget_offset` mutation, clipping/negative offsets, and
  source alpha interpretation.

Current exact outline evidence:

- The project has an `outline_cbox_matrix` lane for cbox/bbox-style geometry.
- `ScaledGlyph` stores raw `FT_Outline_Get_CBox`-style 26.6 coordinates in
  `outline_cbox_*` and exact outline bbox in `outline_bbox_*`.
- Internal decomposers already follow the `FT_Outline_Decompose` contour
  sequence needed by rasterization, but the callbacks are not public or shared.

Dynamic geometry tests should compare:

- `FT_Outline_Get_CBox`: exact `FT_BBox` 26.6 min/max for empty, one-contour,
  multiple-contour, negative-coordinate, translated, transformed, and composite
  glyph outlines.
- `FT_Outline_Get_BBox`: exact bbox including conic/cubic extrema, not just
  point cbox. Include curves whose extrema occur between endpoints.
- `FT_Outline_Translate` and `FT_Outline_Transform`: before/after point arrays,
  tag arrays, contours, flags, cbox, bbox, and decompose event stream.
- `FT_Outline_Embolden` and `FT_Outline_EmboldenXY`: output point geometry,
  orientation-sensitive behavior, error paths for invalid outlines, and bbox.
- `FT_Outline_Reverse`: point order, contour endpoints, tags, orientation, and
  rendered byte parity before/after expected fill-direction changes.
- `FT_Outline_Check`: exact error success/failure for nulls, bad contour
  endpoints, invalid point counts, invalid tags, and mismatched arrays.
- `FT_Outline_Render` and `FT_Outline_Get_Bitmap`: target bitmap bytes,
  clipping, span callbacks, direct/anti-aliased/mono render flags, and pitch
  semantics.

## Missing Variants

Outline gaps:

- No public `FT_Outline` allocation/lifetime API.
- No ABI record that splits `points`, `tags`, and `contours`.
- No public shared decompose callback API.
- No standalone cbox/bbox functions over caller-supplied outlines.
- No matrix transform, embolden, reverse, check, orientation, or caller-target
  render endpoints.
- No cubic tag support in `outline::OutlinePoint`; current point model stores
  only `on_curve: bool`, so ABI decompose cannot distinguish conic off-curve
  from cubic control points.
- No `FT_Raster_Params` support for span callbacks, clipping, direct rendering,
  or caller-managed target bitmaps.

Bitmap gaps:

- No exact `FT_Bitmap` ABI record or library allocator-backed ownership.
- No `palette_mode`/`palette`, `FT_PIXEL_MODE_GRAY2`, `FT_PIXEL_MODE_GRAY4`,
  or `FT_PIXEL_MODE_BGRA`.
- No public bitmap init/new/done/copy/convert/embolden/blend functions.
- No negative pitch input tests or ABI rules for row direction.
- No C ABI compile/link test proving existing C FreeType callers can import
  these symbols unchanged.

## Reusable Dynamic Test Shape

Use one generator shape for both geometry and bitmap APIs:

1. Load the same font bytes into pinned C FreeType and pure Rust `fontdone`.
2. For each row, record stable input fields:
   `font`, `face_index`, `size`, `dpi`, `glyph_index`, `load_flags`,
   `render_mode`, `transform`, `translate`, `outline_mutation`, bitmap
   operation, target alignment, target pitch sign, and source/target offsets.
3. Run the C symbol under test and the Rust implementation path.
4. Serialize outputs as structured scalar records plus raw byte files:
   outline points, tags, contours, flags, orientation, cbox, bbox, decompose
   event stream, bitmap metadata, bitmap bytes, mutated offsets, and error.
5. Compare exact scalar fields and byte hashes. For geometry, coordinates stay
   in 26.6 unless the C API defines pixel units. For bitmap, compare the whole
   addressable buffer in row order after applying C pitch semantics.
6. Name incomplete lanes explicitly, for example:
   `outline_api_geometry_matrix`, `outline_decompose_events_matrix`,
   `outline_render_target_matrix`, `bitmap_ops_matrix`,
   `bitmap_pitch_semantics_matrix`.

The test harness should emit reusable failure IDs with symbol, operation,
font, glyph, size, flags, transform, and pixel mode. That lets future workers
classify failures as `geometry`, `decompose stream`, `bitmap metadata`,
`pitch/row order`, `pixel bytes`, or `error path` without weakening exact
comparisons.

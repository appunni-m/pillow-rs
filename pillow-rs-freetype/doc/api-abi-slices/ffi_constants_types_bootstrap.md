# `fontdone::ffi` Constants And Types Bootstrap

Scope: the minimum first public `fontdone::ffi` constants and leaf record
types to expose before adding any C ABI functions.  This is a compatibility
facade over the pure Rust `fontdone` engine.  It must not call, link, load, or
wrap native FreeType at runtime.

The compatibility target is the pinned FreeType public C surface and oracle
behavior.  Piston `freetype-rs` is useful as a shape reference: its safe crate
re-exports an `ffi` module from `freetype-sys`, uses exact FreeType constants
for `LoadFlag`, `RenderMode`, and error values, and aliases raw records such
as `FT_Vector`, `FT_BBox`, and `FT_Glyph_Metrics`.  `fontdone` should copy the
separation of concerns, not the dependency model: `fontdone::ffi` is authored
inside this crate and backed by pure Rust conversions.

## Module Contract

- `fontdone::ffi` exposes C-named scalar aliases, constants, and `#[repr(C)]`
  leaf records with FreeType numeric values and field order.
- It is not a promise that exported `FT_*` C symbols exist yet.
- It is not a binding to `freetype-sys`, `bindgen`, `cc`, `dlopen`, or native
  FreeType.
- Safe/core APIs continue to use idiomatic Rust types such as `FontError`,
  `api::LoadFlags`, `render::RenderMode`, `render::PixelMode`,
  `api::GlyphFormat`, `api::Vector`, `font::BBox`,
  `font::GlyphSlotMetrics`, and `font::SizeMetrics`.
- Boundary conversion functions must be explicit because several current safe
  values are semantic, not ABI-exact.  In particular, `api::LoadFlags` uses
  local bit positions that conflict with `FT_LOAD_*`.

## Scalar Aliases To Add First

Use C ABI aliases, not Rust convenience widths, for every public `ffi` item.
The first slice only needs these aliases:

| Alias | Rust backing type | Used by | Notes |
| --- | --- | --- | --- |
| `FT_Error` | `core::ffi::c_int` | error constants and future function returns | Exact integer return code, separate from `FontError`. |
| `FT_Int32` | `i32` | `FT_LOAD_*` flags | FreeType load flags are signed 32-bit values. |
| `FT_UInt` | `core::ffi::c_uint` | render/pixel/glyph format enum families | Matches Piston/raw C shape. |
| `FT_UShort` | `core::ffi::c_ushort` | `FT_Size_Metrics::{x_ppem,y_ppem}` | C ABI width, not `u16` by assumption. |
| `FT_Long` | `core::ffi::c_long` | `FT_Pos`, `FT_Fixed`, `FT_F26Dot6` | Platform ABI width differs across targets. |
| `FT_Pos` | `FT_Long` | vectors, bbox, glyph metrics | Usually 26.6 pixels after scaling, endpoint-dependent. |
| `FT_Fixed` | `FT_Long` | size scales | 16.16 fixed point. |
| `FT_F26Dot6` | `FT_Long` | size requests and coordinates where needed next | 26.6 fixed point. |
| `FT_Glyph_Format` | `FT_UInt` | glyph format tag constants | Four-byte tags fit in `FT_UInt`. |
| `FT_Render_Mode` | `FT_UInt` | render mode constants | C enum-compatible integer family. |
| `FT_Pixel_Mode` | `FT_UInt` | bitmap pixel mode constants | C enum-compatible integer family. |

Do not expose `usize` in these aliases.  Safe APIs may keep `usize` for Rust
indices, but the `ffi` layer must preserve C target widths.

## `FT_Error` Codes

Expose the exact FreeType error constants generated from the pinned
`fterrdef.h` layout.  The first set should include the complete public error
table so every early facade function can return stable C-style codes even when
the safe/core error enum is smaller.

| Constant range | Values | Current safe/core mapping |
| --- | --- | --- |
| General success and format errors | `FT_Err_Ok = 0`, `Cannot_Open_Resource = 1`, `Unknown_File_Format = 2`, `Invalid_File_Format = 3`, `Invalid_Version = 4`, `Lower_Module_Version = 5`, `Invalid_Argument = 6`, `Unimplemented_Feature = 7`, `Invalid_Table = 8`, `Invalid_Offset = 9`, `Array_Too_Large = 10`, `Missing_Module = 11`, `Missing_Property = 12` | `FontError::InvalidFont` currently collapses several format/table cases; facade conversion must choose exact codes by call site, not enum discriminant. |
| Glyph and size errors | `Invalid_Glyph_Index = 16`, `Invalid_Character_Code = 17`, `Invalid_Glyph_Format = 18`, `Cannot_Render_Glyph = 19`, `Invalid_Outline = 20`, `Invalid_Composite = 21`, `Too_Many_Hints = 22`, `Invalid_Pixel_Size = 23` | `FontError::InvalidOutline` maps naturally to `Invalid_Outline`; unsupported render/load paths usually map to `Unimplemented_Feature` or measured C-compatible code. |
| Handle errors | `Invalid_Handle = 32`, `Invalid_Library_Handle = 33`, `Invalid_Driver_Handle = 34`, `Invalid_Face_Handle = 35`, `Invalid_Size_Handle = 36`, `Invalid_Slot_Handle = 37`, `Invalid_CharMap_Handle = 38`, `Invalid_Cache_Handle = 39`, `Invalid_Stream_Handle = 40` | No safe equivalent yet; future C ABI handle validation returns these directly. |
| Driver and memory errors | `Too_Many_Drivers = 48`, `Too_Many_Extensions = 49`, `Out_Of_Memory = 64`, `Unlisted_Object = 65` | Mostly future ABI/lifecycle errors. |
| Stream errors | `Cannot_Open_Stream = 81`, `Invalid_Stream_Seek = 82`, `Invalid_Stream_Skip = 83`, `Invalid_Stream_Read = 84`, `Invalid_Stream_Operation = 85`, `Invalid_Frame_Operation = 86`, `Nested_Frame_Access = 87`, `Invalid_Frame_Read = 88` | Safe APIs consume byte slices and owned parsed data; C stream/path facades map I/O/stream failures here. |
| Raster errors | `Raster_Uninitialized = 96`, `Raster_Corrupted = 97`, `Raster_Overflow = 98`, `Raster_Negative_Height = 99` | `FontError::RasterOverflow` maps to `Raster_Overflow`; the other codes are future raster API/error-path outputs. |
| Cache error | `Too_Many_Caches = 112` | Future cache facade only. |
| TrueType bytecode errors | `Invalid_Opcode = 128`, `Too_Few_Arguments = 129`, `Stack_Overflow = 130`, `Code_Overflow = 131`, `Bad_Argument = 132`, `Divide_By_Zero = 133`, `Invalid_Reference = 134`, `Debug_OpCode = 135`, `ENDF_In_Exec_Stream = 136`, `Nested_DEFS = 137`, `Invalid_CodeRange = 138`, `Execution_Too_Long = 139`, `Too_Many_Function_Defs = 140`, `Too_Many_Instruction_Defs = 141` | The bytecode interpreter should translate exact internal failure sites instead of folding them into `InvalidFont`. |
| SFNT/table-specific errors | `Table_Missing = 142`, `Horiz_Header_Missing = 143`, `Locations_Missing = 144`, `Name_Table_Missing = 145`, `CMap_Table_Missing = 146`, `Hmtx_Table_Missing = 147`, `Post_Table_Missing = 148`, `Invalid_Horiz_Metrics = 149`, `Invalid_CharMap_Format = 150`, `Invalid_PPem = 151`, `Invalid_Vert_Metrics = 152`, `Could_Not_Find_Context = 153`, `Invalid_Post_Table_Format = 154`, `Invalid_Post_Table = 155` | Existing `FontError::{InvalidFont,UnsupportedCmapFormat}` are semantic buckets; facade code needs lower-level parser error classification before exact parity can be claimed. |
| Type1/BDF-style errors | `Syntax_Error = 160`, `Stack_Underflow = 161`, `Ignore = 162`, `No_Unicode_Glyph_Name = 163`, `Missing_Startfont_Field = 176`, `Missing_Font_Field = 177`, `Missing_Size_Field = 178`, `Missing_Fontboundingbox_Field = 179`, `Missing_Chars_Field = 180`, `Missing_Startchar_Field = 181`, `Missing_Encoding_Field = 182`, `Missing_Bbx_Field = 183`, `Bbx_Too_Big = 184`, `Corrupted_Font_Header = 185`, `Corrupted_Font_Glyphs = 186`, `Max = 187` | Mostly future non-TrueType or compatibility reporting. Include now for complete `FT_Error` value stability. |

## `FT_LOAD_*` Flags

Expose exact C values as `FT_Int32` constants.  Do not reuse current
`api::LoadFlags` bits directly.

| Constant | Value | Current safe/core mapping |
| --- | ---: | --- |
| `FT_LOAD_DEFAULT` | `0x00000000` | `api::LoadFlags::DEFAULT`; exact value matches. |
| `FT_LOAD_NO_SCALE` | `0x00000001` | Missing; future no-scale load path returns font units where C does. |
| `FT_LOAD_NO_HINTING` | `0x00000002` | Convert to `api::LoadFlags::NO_HINTING`; safe value is `0x4`. |
| `FT_LOAD_RENDER` | `0x00000004` | Convert to `api::LoadFlags::RENDER`; safe value is `0x1`. |
| `FT_LOAD_NO_BITMAP` | `0x00000008` | Missing behavior; expose constant and classify as no embedded bitmap support for now. |
| `FT_LOAD_VERTICAL_LAYOUT` | `0x00000010` | Missing; future vertical metrics/advance behavior. |
| `FT_LOAD_FORCE_AUTOHINT` | `0x00000020` | Convert to `api::LoadFlags::FORCE_AUTOHINT`; safe value is `0x2`. |
| `FT_LOAD_CROP_BITMAP` | `0x00000040` | Missing; legacy/planned. |
| `FT_LOAD_PEDANTIC` | `0x00000080` | Missing strict error behavior. |
| `FT_LOAD_ADVANCE_ONLY` | `0x00000100` | Internal C flag; expose only if the pinned public headers expose it for this version, then classify behavior. |
| `FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH` | `0x00000200` | Missing/planned. |
| `FT_LOAD_NO_RECURSE` | `0x00000400` | Missing composite slot behavior. |
| `FT_LOAD_IGNORE_TRANSFORM` | `0x00000800` | Missing until transforms exist. |
| `FT_LOAD_MONOCHROME` | `0x00001000` | Missing; current mono rendering uses safe target flags instead. |
| `FT_LOAD_LINEAR_DESIGN` | `0x00002000` | Missing linear metrics behavior. |
| `FT_LOAD_SBITS_ONLY` | `0x00004000` | Missing embedded bitmap support. |
| `FT_LOAD_NO_AUTOHINT` | `0x00008000` | Missing; must differ from default and force-autohint. |
| `FT_LOAD_COLOR` | `0x00100000` | Missing color glyph behavior. |
| `FT_LOAD_COMPUTE_METRICS` | `0x00200000` | Missing; expose when pinned headers include it. |
| `FT_LOAD_BITMAP_METRICS_ONLY` | `0x00400000` | Missing; expose when pinned headers include it. |
| `FT_LOAD_SVG_ONLY` | `0x00800000` | Missing/internal C flag; expose when pinned headers include it. |
| `FT_LOAD_NO_SVG` | `0x01000000` | Missing; expose when pinned headers include it. |

Expose `FT_LOAD_TARGET_(mode)` equivalent constants:

| Constant | Value | Conversion |
| --- | ---: | --- |
| `FT_LOAD_TARGET_NORMAL` | `0x00000000` | `render::RenderMode::Normal` when rendering is requested. |
| `FT_LOAD_TARGET_LIGHT` | `0x00010000` | Missing safe render/load target; C rendering is normal-like but hinting differs. |
| `FT_LOAD_TARGET_MONO` | `0x00020000` | Convert to `api::LoadFlags::TARGET_MONO` only after decoding bits 16..19. |
| `FT_LOAD_TARGET_LCD` | `0x00030000` | Convert to `api::LoadFlags::TARGET_LCD` only after decoding bits 16..19. |
| `FT_LOAD_TARGET_LCD_V` | `0x00040000` | Convert to `api::LoadFlags::TARGET_LCD_V` only after decoding bits 16..19. |

Add a helper such as `ffi_load_target_mode(flags: FT_Int32) -> FT_Render_Mode`
that implements `((flags >> 16) & 15)`.  Do not infer render mode by checking
current safe low-order target bits.

## Render, Pixel, And Glyph Format Constants

| Family | Constants and values | Current safe/core mapping |
| --- | --- | --- |
| `FT_RENDER_MODE_*` | `NORMAL=0`, `LIGHT=1`, `MONO=2`, `LCD=3`, `LCD_V=4`, `SDF=5`, `MAX=6` | `render::RenderMode` has `Normal`, `Mono`, `Lcd`, `LcdV`; add explicit conversion and reject/classify `LIGHT`, `SDF`, and `MAX` until behavior exists. |
| `FT_PIXEL_MODE_*` | `NONE=0`, `MONO=1`, `GRAY=2`, `GRAY2=3`, `GRAY4=4`, `LCD=5`, `LCD_V=6`, `BGRA=7`, `MAX=8` | `render::PixelMode` has `Gray`, `Mono`, `Lcd`, `LcdV`; expose all C constants and convert unsupported modes to an explicit error. |
| `FT_GLYPH_FORMAT_*` | `NONE=0x00000000`, `COMPOSITE=0x636f6d70` (`'comp'`), `BITMAP=0x62697473` (`'bits'`), `OUTLINE=0x6f75746c` (`'outl'`), `PLOTTER=0x706c6f74` (`'plot'`), `SVG=0x53564720` (`'SVG '`) when present in pinned headers | `api::GlyphFormat` has `None`, `Outline`, `Bitmap`; map only those three today. Do not give the safe enum C tag discriminants unless it is deliberately changed. |

The bootstrap should include exact constant tests for every value above,
including `FT_RENDER_MODE_MAX = FT_RENDER_MODE_SDF + 1` and target constants
derived from render-mode values.

## Leaf Records

These records are the smallest useful C-shaped values.  They can be added
before handle graph records such as `FT_FaceRec`, `FT_SizeRec`, or
`FT_GlyphSlotRec`.

| `ffi` type | Required layout | Current safe/core type | Mapping rule |
| --- | --- | --- | --- |
| `FT_Vector` | `#[repr(C)] { x: FT_Pos, y: FT_Pos }` | `api::Vector { x: i32, y: i32 }` | Convert with checked/narrowing-aware casts at the boundary. Safe values are 26.6 pixels unless documented otherwise. |
| `FT_BBox` | `#[repr(C)] { xMin: FT_Pos, yMin: FT_Pos, xMax: FT_Pos, yMax: FT_Pos }` | `font::BBox { x_min, y_min, x_max, y_max }`; scaler cbox/bbox fields | Preserve C field names and order in `ffi`. Safe Rust keeps snake_case. Units depend on endpoint: font units, 26.6 subpixels, or pixels after bbox mode rounding. |
| `FT_Glyph_Metrics` | `#[repr(C)] { width, height, horiBearingX, horiBearingY, horiAdvance, vertBearingX, vertBearingY, vertAdvance: FT_Pos }` | `font::GlyphSlotMetrics` | Field order is conceptually the same; names and C aliases differ. Convert all eight fields explicitly. |
| `FT_Size_Metrics` | `#[repr(C)] { x_ppem: FT_UShort, y_ppem: FT_UShort, x_scale: FT_Fixed, y_scale: FT_Fixed, ascender: FT_Pos, descender: FT_Pos, height: FT_Pos, max_advance: FT_Pos }` | `font::SizeMetrics` | Map the first eight semantic fields only. Do not include Rust-only request metadata `x_dpi`, `y_dpi`, `char_width`, or `char_height` in the C-shaped record. |

Add `From`/`TryFrom` implementations only where they cannot hide lossy casts.
For platform-dependent aliases such as `FT_Pos = c_long`, prefer fallible
helpers or generated layout/value tests over assuming `i32`.

## Initial Conversion Surface

The first implementation should provide only constant/type definitions and
small conversion helpers:

| Helper | Purpose |
| --- | --- |
| `ft_error_from_font_error(err: &FontError) -> FT_Error` | Temporary semantic mapping for safe API calls; exact call-site errors still need deeper parser/load classification. |
| `try_load_flags_from_ffi(flags: FT_Int32) -> Result<api::LoadFlags, FT_Error>` | Decode exact `FT_LOAD_*` bits into current safe flags where behavior exists; reject or classify unsupported bits rather than silently dropping them. |
| `try_render_mode_from_ffi(mode: FT_Render_Mode) -> Result<render::RenderMode, FT_Error>` | Map `NORMAL`, `MONO`, `LCD`, `LCD_V`; classify `LIGHT`, `SDF`, invalid, and `MAX`. |
| `pixel_mode_to_ffi(mode: render::PixelMode) -> FT_Pixel_Mode` | Map current rendered bitmap modes to exact C values. |
| `glyph_format_to_ffi(format: api::GlyphFormat) -> FT_Glyph_Format` | Map `None`, `Bitmap`, and `Outline` to exact C tags. |
| `FT_Vector::from(api::Vector)` / checked reverse | Preserve 26.6 values with C field names. |
| `FT_BBox::from(font::BBox)` / checked reverse | Preserve min/max order while translating names. |
| `FT_Glyph_Metrics::from(font::GlyphSlotMetrics)` | Preserve all metric fields. |
| `FT_Size_Metrics::from(font::SizeMetrics)` | Drop only Rust request metadata; never reinterpret the full safe struct as ABI memory. |

Unsupported constants should still be public if they are in the pinned C
headers.  Unsupported behavior belongs in conversion/function results, not in
missing numeric definitions.

## Deliberately Out Of This Bootstrap

- Exported `extern "C"` `FT_*` functions.
- Opaque handles (`FT_Library`, `FT_Face`, `FT_Size`, `FT_GlyphSlot`) and
  handle graph records.
- `FT_Bitmap`, `FT_Outline`, `FT_GlyphSlotRec`, and `FT_FaceRec`; these need
  stable backing storage and ownership rules beyond leaf records.
- `FT_Matrix`, `FT_Generic`, charmap, kerning, bbox-mode, outline flags, curve
  tags, encoding, and face/style flag constants unless a follow-up slice
  includes them.
- Any dependency on native FreeType, Piston `freetype-sys`, `bindgen`, or
  generated Rust copied from a system installation.

## Verification To Require With The First Code Change

- Unit tests for exact numeric constants listed in this document.
- Layout tests for each `#[repr(C)]` leaf record: `size_of`, `align_of`, and
  field offsets against generated pinned-header data.
- Conversion tests proving current safe `LoadFlags` values are not exposed as
  `FT_LOAD_*` values.
- `make -C pillow-rs-freetype api-abi-audit`.
- `make fontdone-ffi` or the repository's no-runtime-FFI target.
- `cargo fmt --all -- --check` or the Makefile formatting target.

Those checks prove only the constants/types bootstrap.  They do not prove C
ABI replacement, glyph-slot mutation, exported symbol names, or output parity.

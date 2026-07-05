# FreeType Records And Constants Compatibility Slice

Scope: constants, typedefs, enums, and C record exactness for the future
FreeType C API/ABI layer.  The target is the pinned FreeType 2.14.3 public
headers.  Servo `rust-freetype` is a binding reference only; it shows C-shaped
Rust generated from FreeType headers, not the desired safe API design.

Current `fontdone` public types are semantic Rust types.  They are useful for
parity work, but they are not ABI records unless they are dedicated
`#[repr(C)]` types with C field names, field order, type widths, pointer
ownership, and units.

## Scalar Typedef Baseline

The ABI layer must define scalar aliases with C target widths, not convenience
Rust widths.  Generated layout tests must run on each supported target triple
because `FT_Long`, `FT_ULong`, `FT_Pos`, `FT_F26Dot6`, and `FT_Fixed` follow
the platform C ABI.

| C name | C target type | Units / role | Servo presence | fontdone current type | Status | Migration action |
| --- | --- | --- | --- | --- | --- | --- |
| `FT_Bool` | `unsigned char` | Boolean byte | Present | none | Missing | Add ABI alias to `c_uchar`. |
| `FT_Byte` / `FT_Bytes` | `unsigned char` / `const FT_Byte*` | Raw bytes | Present | `u8`, slices, `Vec<u8>` internally | Semantic only | Keep safe slices in core; expose ABI aliases in C layer. |
| `FT_Char` / `FT_String` | `signed char` / `char` | C chars and strings | Present | `String`, `Option<String>` | Semantic only | C records use raw `*mut FT_String`; safe wrapper owns strings. |
| `FT_Short` / `FT_UShort` | C `short` / `unsigned short` | 16-bit scalar on supported ABIs | Present | `i16`, `u16` in several fields | Partial | Use C aliases in ABI records; safe layer may keep Rust names. |
| `FT_Int` / `FT_UInt` | C `int` / `unsigned int` | Generic integer | Present | `i32`, `u32`, `usize` mixed | Partial | Use `c_int` / `c_uint`; avoid `usize` in ABI records. |
| `FT_Long` / `FT_ULong` | C `long` / `unsigned long` | Face counts, flags, stream sizes | Present | `usize`, `u32`, `i32` mixed | Missing for ABI | Use `c_long` / `c_ulong`; generated tests cover LP64/LLP64 differences. |
| `FT_Pos` | `FT_Long` | Font units or 26.6 pixels by context | Present | mostly `i32` | Missing for ABI | ABI records use `FT_Pos`; safe layer can convert with checked casts. |
| `FT_Fixed` | `FT_Long` | 16.16 fixed point | Present | `i32` in `SizeMetrics` and helpers | Missing for ABI | ABI records use `FT_Fixed`; core may keep checked `i32` where guaranteed. |
| `FT_F26Dot6` | `FT_Long` | 26.6 fixed point | Present | `i32` | Missing for ABI | Add alias; document conversions at ABI boundary. |
| `FT_Tag` | `FT_UInt32` | Four-byte tag | Present | `u32` | Exact semantically | Add ABI alias and `FT_MAKE_TAG` numeric tests. |
| `FT_Pointer` | `void*` | Typeless client pointer | Present | none | Missing | Add ABI alias for `FT_Generic`, lists, callbacks, and private handles. |
| Opaque handles (`FT_Library`, `FT_Face`, `FT_Size`, `FT_GlyphSlot`, etc.) | Pointer to incomplete or public record | C lifecycle handles | Present | owned `Library`, `Face`, `GlyphSlot` values | Missing for ABI | C layer owns pinned boxes/arenas and exposes raw handles; core stays pure Rust. |

## Record Exactness Matrix

`Exact` here means field names, order, C types, pointer shape, and ABI layout
match the public C header.  None of the current semantic Rust types should be
retrofitted in place if that would pollute core ownership; add a separate ABI
module or crate.

| C name | C fields in order, C types, units | Servo presence | fontdone current type | Status | Migration action |
| --- | --- | --- | --- | --- | --- |
| `FT_Vector` | `x: FT_Pos`, `y: FT_Pos`; coordinates in `FT_Pos`, usually 26.6 pixels after scaling | `FT_Vector_` exact `#[repr(C)]` | `api::Vector { x: i32, y: i32 }` | Field order exact, type width not ABI-exact | Add ABI `FT_Vector { x: FT_Pos, y: FT_Pos }`; convert to/from safe `Vector`. |
| `FT_BBox` | `xMin`, `yMin`, `xMax`, `yMax: FT_Pos`; font units or 26.6 depending endpoint | Exact | `font::BBox { x_min, y_min, x_max, y_max: i32 }` | Semantic only; field names and type aliases differ | Add ABI `FT_BBox` with C names; map from safe bbox. |
| `FT_Bitmap` | `rows: unsigned int`, `width: unsigned int`, `pitch: int`, `buffer: unsigned char*`, `num_grays: unsigned short`, `pixel_mode: unsigned char`, `palette_mode: unsigned char`, `palette: void*`; rows/width are physical pixels, pitch is bytes | Exact | `render::RenderedBitmap { width, rows, pitch, pixel_mode, num_grays, left, top, buffer: Vec<u8> }` | Not exact; order differs, buffer ownership differs, extra placement fields, missing palette fields | Add ABI bitmap backed by owned storage in slot/bitmap object; keep `left/top` in `FT_GlyphSlotRec`, not `FT_Bitmap`. |
| `FT_Glyph_Metrics` | `width`, `height`, `horiBearingX`, `horiBearingY`, `horiAdvance`, `vertBearingX`, `vertBearingY`, `vertAdvance: FT_Pos`; normally 26.6 pixels unless `FT_LOAD_NO_SCALE` | Exact | `font::GlyphSlotMetrics` with snake_case `i32` fields | Semantic only; order matches conceptually but names/types differ | Add ABI metrics record with C names and `FT_Pos`; generated tests assert offsets. |
| `FT_Size_Metrics` | `x_ppem`, `y_ppem: FT_UShort`; `x_scale`, `y_scale: FT_Fixed` 16.16; `ascender`, `descender`, `height`, `max_advance: FT_Pos` 26.6 pixels | Exact | `font::SizeMetrics` plus `x_dpi`, `y_dpi`, `char_width`, `char_height` | Extra fields make layout non-ABI | Add ABI metrics record and keep request metadata outside the C record. |
| `FT_Outline` | `n_contours: unsigned short`, `n_points: unsigned short`, `points: FT_Vector*`, `tags: unsigned char*`, `contours: unsigned short*`, `flags: int`; points in 26.6, contours are endpoint indices, tags carry curve bits | Exact in field set; Servo binding checked here uses signed C aliases for counts/contours | `outline::Outline { n_contours: i32, contours: Vec<i16>, points: Vec<OutlinePoint>, flags: u32, cbox_* }` | Not exact; owned vectors, no `n_points`, packed tags absent, extra cbox fields | Add ABI outline view over pinned point/tag/contour buffers; cbox cache stays internal. |
| `FT_FaceRec` | `num_faces`, `face_index`, `face_flags`, `style_flags`, `num_glyphs: FT_Long`; `family_name`, `style_name: FT_String*`; `num_fixed_sizes: FT_Int`; `available_sizes: FT_Bitmap_Size*`; `num_charmaps: FT_Int`; `charmaps: FT_CharMap*`; `generic: FT_Generic`; `bbox: FT_BBox`; `units_per_EM: FT_UShort`; face metrics `FT_Short`; `glyph`, `size`, `charmap` handles; private `driver`, `memory`, `stream`, `sizes_list`, `autohint`, `extensions`, `internal` | Exact | `api::Face` plus `font::FaceInfo` | Missing as ABI record; safe metadata omits handle graph and private fields | Add C-facing face object that owns core face plus C record mirrors, stable strings, charmaps, size, glyph slot, and private opaque handles. |
| `FT_GlyphSlotRec` | `library: FT_Library`, `face: FT_Face`, `next: FT_GlyphSlot`, `glyph_index: FT_UInt`, `generic: FT_Generic`, `metrics: FT_Glyph_Metrics`, `linearHoriAdvance`, `linearVertAdvance: FT_Fixed`, `advance: FT_Vector`, `format: FT_Glyph_Format`, `bitmap: FT_Bitmap`, `bitmap_left`, `bitmap_top: FT_Int`, `outline: FT_Outline`, `num_subglyphs: FT_UInt`, `subglyphs: FT_SubGlyph`, `control_data: void*`, `control_len: long`, `lsb_delta`, `rsb_delta: FT_Pos`, `other: void*`, `internal: FT_Slot_Internal` | Present; Servo names field 4 `reserved` in the checked binding, while FreeType 2.14.3 names it `glyph_index` | `api::GlyphSlot` snapshot | Missing as ABI record; safe type has subset only | Add ABI slot record. Field 4 must follow pinned header name `glyph_index`; the pinned C header wins over Servo for this mismatch. |
| `FT_CharMapRec` | `face: FT_Face`, `encoding: FT_Encoding`, `platform_id: FT_UShort`, `encoding_id: FT_UShort` | Exact | `font::CharmapInfo { index, platform_id, encoding_id, format }` | Semantic only; missing face and encoding tag, extra index/format | Add ABI charmap records owned by face; expose index through helper functions, not record layout. |
| `FT_SizeRec` | `face: FT_Face`, `generic: FT_Generic`, `metrics: FT_Size_Metrics`, `internal: FT_Size_Internal` | Exact | no ABI type; `Face::size_metrics()` returns value | Missing | Add size handle and record tied to face active size. |
| `FT_Matrix` | `xx`, `xy`, `yx`, `yy: FT_Fixed`; 16.16 transform matrix | Exact | no public matrix type | Missing | Add ABI matrix for transform APIs; core helpers may use internal numeric type. |
| `FT_Bitmap_Size` | `height`, `width: FT_Short` pixels; `size`, `x_ppem`, `y_ppem: FT_Pos` in 26.6 | Exact | no public strike record | Missing | Add for `FT_FaceRec::available_sizes`; unsupported bitmap strikes still require zero-count exactness. |
| `FT_Generic` | `data: void*`, `finalizer: FT_Generic_Finalizer` | Exact | none | Missing | Add to all ABI records that expose `generic`; call finalizers on destruction where FreeType does. |
| `FT_ListNodeRec` | `prev: FT_ListNode`, `next: FT_ListNode`, `data: void*` | Exact | none | Missing | Add if exposing face size list/private-compatible records. |
| `FT_ListRec` | `head: FT_ListNode`, `tail: FT_ListNode` | Exact | none | Missing | Add for `FT_FaceRec::sizes_list` layout even if list operations remain planned. |
| `FT_UnitVector` | `x`, `y: FT_F2Dot14` | Exact | no public type | Missing | Add with vector/trigonometry APIs. |
| `FT_Data` | `pointer: const FT_Byte*`, `length: FT_UInt` | Exact | slices internally | Missing | Add for APIs exposing raw table/data blobs. |
| `FT_Outline_Funcs` | `move_to`, `line_to`, `conic_to`, `cubic_to` callbacks; `shift`, `delta: int` | Exact | none | Missing | Add when `FT_Outline_Decompose` is exposed. |
| `FT_Raster_Params` | `target: const FT_Bitmap*`, `source: const void*`, `flags: int`, span/bit callbacks, `user: void*`, `clip_box: FT_BBox` | Exact | internal raster APIs | Missing | Add only in C ABI/raster surface; core must not call native rasterizers. |

## Enum And Constant Exactness

Generated tests should cover numeric value, Rust type width, and alias
relationships for each public constant family below.

| Family | C values / rule | Servo presence | fontdone current type | Status | Migration action |
| --- | --- | --- | --- | --- | --- |
| `FT_Pixel_Mode` | enum values: `NONE=0`, `MONO=1`, `GRAY=2`, `GRAY2=3`, `GRAY4=4`, `LCD=5`, `LCD_V=6`, `BGRA=7`, `MAX=8` | Present | `render::PixelMode::{Gray, Mono, Lcd, LcdV}` without explicit discriminants and missing variants | Partial semantic | Add ABI enum/constants with exact values; safe enum can stay smaller but conversion must reject unsupported modes. |
| `FT_Render_Mode` | `NORMAL=0`, `LIGHT=1`, `MONO=2`, `LCD=3`, `LCD_V=4`, `SDF=5`, `MAX=6` | Present | `render::RenderMode::{Normal, Mono, Lcd, LcdV}` without explicit discriminants | Partial semantic | Add ABI values including `LIGHT`, `SDF`, `MAX`; map `LIGHT` behavior per FreeType docs where applicable. |
| `FT_Glyph_Format` | four-byte tags made with `FT_IMAGE_TAG`, including `NONE=0`, `COMPOSITE`, `BITMAP`, `OUTLINE`, `PLOTTER`, `SVG` | Present | `api::GlyphFormat::{None, Outline, Bitmap}` | Partial semantic | Add exact ABI constants; do not equate Rust enum discriminants with tags. |
| `FT_Encoding` | four-byte tags made with `FT_ENC_TAG`; aliases such as `GB2312 == PRC` and `MS_SJIS == SJIS` | Present | selected charmap uses platform/encoding ids, no public encoding enum | Missing | Add exact constants and alias tests; derive tags with the same byte shifts as `FT_MAKE_TAG`. |
| `FT_LOAD_*` flags | `DEFAULT=0`, `NO_SCALE=1<<0`, `NO_HINTING=1<<1`, `RENDER=1<<2`, `NO_BITMAP=1<<3`, `VERTICAL_LAYOUT=1<<4`, `FORCE_AUTOHINT=1<<5`, `CROP_BITMAP=1<<6`, `PEDANTIC=1<<7`, `ADVANCE_ONLY=1<<8`, `IGNORE_GLOBAL_ADVANCE_WIDTH=1<<9`, `NO_RECURSE=1<<10`, `IGNORE_TRANSFORM=1<<11`, `MONOCHROME=1<<12`, `LINEAR_DESIGN=1<<13`, `SBITS_ONLY=1<<14`, `NO_AUTOHINT=1<<15`, `COLOR=1<<20`, `COMPUTE_METRICS=1<<21`, `BITMAP_METRICS_ONLY=1<<22`, `SVG_ONLY=1<<23`, `NO_SVG=1<<24` | Present | `api::LoadFlags` uses local bit positions: `RENDER=1<<0`, `FORCE_AUTOHINT=1<<1`, etc. | Not ABI-exact | Add ABI load constants exactly. Safe `LoadFlags` must either switch to C values or be explicitly converted at boundary. |
| `FT_LOAD_TARGET_*` | `FT_LOAD_TARGET_(mode) = (mode & 15) << 16`; normal/light/mono/lcd/lcd_v derive from render mode values | Present | local target bits are low-order safe flags | Not ABI-exact | Add computed constant tests and boundary conversion. |
| `FT_FACE_FLAG_*` | bit values `SCALABLE=1<<0` through `SBIX_OVERLAY=1<<18` in 2.14.3 | Present | `Font::face_flags()` sets subset with local constants | Partial semantic | Add all ABI constants; safe subset output must use exact numeric values. |
| `FT_STYLE_FLAG_*` | `ITALIC=1<<0`, `BOLD=1<<1` | Present | `Font::style_flags()` uses matching local constants | Semantic exact | Promote constants into ABI module and test exact values. |
| `FT_OUTLINE_*` | `NONE=0`, `OWNER=1`, `EVEN_ODD_FILL=2`, `REVERSE_FILL=4`, `IGNORE_DROPOUTS=8`, `SMART_DROPOUTS=16`, `INCLUDE_STUBS=32`, `OVERLAP=64`, `HIGH_PRECISION=256`, `SINGLE_PASS=512`; max counts are `USHRT_MAX` | Present | `outline::Outline.flags: u32` with no public constants | Missing | Add exact constants and use packed tag/flag values in ABI outlines. |
| `FT_CURVE_TAG_*` | `ON=1`, `CONIC=0`, `CUBIC=2`, `HAS_SCANMODE=4`, `TOUCH_X=8`, `TOUCH_Y=16`, `TOUCH_BOTH=24` | Present | `OutlinePoint.on_curve: bool` | Missing for ABI | Add tag constants and convert internal point kind to C tag bytes. |
| `FT_Kerning_Mode` | `DEFAULT=0`, `UNFITTED=1`, `UNSCALED=2` | Present | no public enum | Missing | Add ABI constants when kerning APIs are exposed. |
| `FT_Glyph_BBox_Mode` | `UNSCALED=0`, `SUBPIXELS=0`, `GRIDFIT=1`, `TRUNCATE=2`, `PIXELS=3` | Present | bbox/cbox helpers use dedicated methods | Missing | Add exact constants and alias equality tests. |
| Error constants `FT_Err_*` | generated from `fterrdef.h`, module/error layout controlled by FreeType config | Present | `FontError` categories | Semantic only | Generate exact error constants and C-facing return codes; do not expose Rust enum discriminants as ABI. |

## Generated Test Plan

Tests must validate the future C ABI surface without adding runtime FFI or
native FreeType calls to core/fontdone.

1. Generate a small C probe from the pinned headers under
   `pillow-rs-freetype/freetype/include` during `make api-abi-audit`.  The
   probe prints JSON for `sizeof`, `_Alignof`, and `offsetof` for every ABI
   record in this slice, plus numeric values for constants and enum variants.
   It links only the C compiler runtime and includes headers; it does not call
   FreeType functions or link `libfreetype`.
2. Generate Rust layout tests for the ABI module using `core::mem::size_of`,
   `core::mem::align_of`, and `memoffset::offset_of!` or a local equivalent.
   These tests compare against the C probe JSON generated under
   `target/api-abi-audit`, not against hand-written offsets.
3. Generate Rust constant tests from the same header inventory.  Macro
   expressions such as `FT_LOAD_TARGET_(FT_RENDER_MODE_MONO)`, `FT_MAKE_TAG`,
   and enum aliases must be evaluated by the C probe, then compared to ABI
   Rust constants.
4. Keep the tests in a C-ABI package/module, not in core parsing, hinting, or
   rasterization code.  The production library must not depend on `bindgen`,
   `freetype-sys`, `cc`, `dlopen`, or native FreeType.
5. Gate each record with all of these checks: field count, ordered field names
   in generated header output, Rust field types mapped to C aliases, `sizeof`,
   `alignof`, and every field offset.  A semantically equivalent safe wrapper
   does not satisfy this gate.
6. Gate constants with exact integer equality and alias equality.  Deprecated
   lowercase aliases such as `ft_pixel_mode_mono` and `ft_kerning_default`
   should be checked as aliases to the canonical uppercase names when exposed.

## Migration Order

1. Add a separate ABI type module with scalar aliases, opaque handles, and
   leaf records first: `FT_Vector`, `FT_BBox`, `FT_Matrix`, `FT_Generic`,
   `FT_ListNodeRec`, `FT_ListRec`, `FT_Bitmap_Size`.
2. Add exact constants/enums with generated numeric tests before wiring any
   functions that accept flags or modes.
3. Add buffer-backed records: `FT_Bitmap`, `FT_Outline`,
   `FT_Glyph_Metrics`, and `FT_Size_Metrics`.
4. Add handle graph records: `FT_CharMapRec`, `FT_SizeRec`,
   `FT_GlyphSlotRec`, and `FT_FaceRec`.
5. Only after records and constants are exact, expose C ABI functions that
   return pointers to those records.

## Open Risks

- Platform C width differences must be tested explicitly.  Assuming `FT_Long`
  is always `i32` would be wrong on LP64 targets.
- Servo's checked binding has `FT_GlyphSlotRec_::reserved` where the pinned
  FreeType 2.14.3 public header uses `glyph_index`; the pinned C header wins.
- The current safe `LoadFlags` numeric values conflict with C `FT_LOAD_*`.
  Reusing them directly in a C ABI would break callers.
- Owned Rust containers in safe types (`Vec`, `String`, `Option`) cannot appear
  in ABI records.  The C layer needs stable storage and explicit lifecycle
  rules.

# Face And Size API/ABI Slice

Scope: face creation/destruction and active size APIs for a future Rust-backed
FreeType C ABI layer.  The current `fontdone` API is a safe Rust semantic
facade, not a C ABI replacement: there are no exported `FT_*` symbols and no
`#[repr(C)]` public records for this slice yet.

Reference headers: FreeType public headers, primarily `freetype.h` and
`ftsizes.h`.  Servo `rust-freetype` is useful as a C binding checklist only;
the compatibility target is the pinned C FreeType surface and oracle behavior.

## C Symbols

| Symbol | C return | C params | Current fontdone mapping | ABI status |
|---|---|---|---|---|
| `FT_New_Face` | `FT_Error` | `FT_Library library, const char* filepathname, FT_Long face_index, FT_Face *aface` | none | Planned. Needs ABI path/file open wrapper outside core. |
| `FT_New_Memory_Face` | `FT_Error` | `FT_Library library, const FT_Byte* file_base, FT_Long file_size, FT_Long face_index, FT_Face *aface` | `Library::new_memory_face`, `Face::from_memory`, `Font::truetype_face` | Semantically mapped for positive zero-based face index and memory bytes. Not ABI exact. |
| `FT_Open_Face` | `FT_Error` | `FT_Library library, const FT_Open_Args* args, FT_Long face_index, FT_Face *aface` | none | Planned. Needs `FT_Open_Args`, driver/stream/parameter handling, negative index probe semantics. |
| `FT_Attach_File` | `FT_Error` | `FT_Face face, const char* filepathname` | none | Planned. No AFM/PFM attachment path today. |
| `FT_Attach_Stream` | `FT_Error` | `FT_Face face, const FT_Open_Args* parameters` | none | Planned. Depends on `FT_Open_Args` stream support. |
| `FT_Reference_Face` | `FT_Error` | `FT_Face face` | none | Planned. Current Rust `Face`/`Font` clone is not C refcount semantics. |
| `FT_Done_Face` | `FT_Error` | `FT_Face face` | drop `Face` / drop `Font` | Semantic cleanup only. Future ABI must decrement C face refcount and free owned slot/sizes only at zero. |
| `FT_Select_Size` | `FT_Error` | `FT_Face face, FT_Int strike_index` | none | Planned. No bitmap strike selection or `available_sizes` surface yet. |
| `FT_Request_Size` | `FT_Error` | `FT_Face face, FT_Size_Request req` | none | Planned. Current APIs cover only char-size and pixel-size helpers. |
| `FT_Set_Char_Size` | `FT_Error` | `FT_Face face, FT_F26Dot6 char_width, FT_F26Dot6 char_height, FT_UInt horz_resolution, FT_UInt vert_resolution` | `Face::set_char_size`, `Font::set_char_size` | Semantically mapped for supported SFNT scalable faces. Not ABI exact and has known normalization gaps. |
| `FT_Set_Pixel_Sizes` | `FT_Error` | `FT_Face face, FT_UInt pixel_width, FT_UInt pixel_height` | `Face::set_pixel_sizes`, `Font::set_pixel_sizes` | Semantically mapped for supported SFNT scalable faces. Not ABI exact. |
| `FT_New_Size` | `FT_Error` | `FT_Face face, FT_Size* size` | none | Planned dependency for `FT_Activate_Size`/`FT_Done_Size`, even though not separately owned by this worker brief. |
| `FT_Activate_Size` | `FT_Error` | `FT_Size size` | none | Planned. Current model has one active size embedded in `Font`. |
| `FT_Done_Size` | `FT_Error` | `FT_Size size` | none | Planned. Current model has no independent size object. |
| `FT_Face_Properties` | `FT_Error` | `FT_Face face, FT_UInt num_properties, FT_Parameter* properties` | none | Planned. Needed for face-scoped driver properties such as LCD filter weights or interpreter version. |
| `FT_Set_Transform` | `void` | `FT_Face face, FT_Matrix* matrix, FT_Vector* delta` | none | Planned if this slice owns face-scoped transforms. Current load/render paths have no persistent face transform. |

## Records Touched

### `FT_FaceRec`

C field order:

```text
num_faces, face_index, face_flags, style_flags, num_glyphs,
family_name, style_name, num_fixed_sizes, available_sizes,
num_charmaps, charmaps, generic, bbox, units_per_EM, ascender,
descender, height, max_advance_width, max_advance_height,
underline_position, underline_thickness, glyph, size, charmap,
driver, memory, stream, sizes_list, autohint, extensions, internal
```

Current mapping is split across `Face`, `Font`, and `FaceInfo`.  `FaceInfo`
contains semantic scalar metadata such as `num_faces`, `face_index`,
`family_name`, `style_name`, `font_format`, `units_per_em`, `num_glyphs`,
`bbox`, ascender/descender/height, advances, underline metrics, `face_flags`,
`style_flags`, and `fs_type_flags`.  It does not expose C pointer fields,
`glyph`, `size`, `charmap`, `generic`, `available_sizes`, `sizes_list`,
driver/memory/stream handles, or internal lifetime fields.

Exactness gaps:

- No `#[repr(C)] FT_FaceRec` with C field order and pointer-sized members.
- `FaceInfo` uses Rust-owned `String`/`Option<String>` instead of C string
  pointers whose lifetime is tied to the face.
- `num_fixed_sizes`/`available_sizes` are absent, so `FT_Select_Size` cannot
  be ABI-compatible yet.
- `face->glyph`, `face->size`, and `face->charmap` are not stable C pointers.
- Face flags are approximate for supported SFNT outline faces and do not yet
  cover every FreeType driver flag, variation flag, color flag, tricky flag,
  or internal `FT_FACE_FLAG_EXTERNAL_STREAM` behavior.

### `FT_SizeRec`

C field order:

```text
face, generic, metrics, internal
```

Current mapping is `Font::size_metrics()` returning a copied `SizeMetrics`.
There is no separate `FT_Size` handle, no per-size `generic`, no `internal`,
and no `face->size` pointer update.

Exactness gaps:

- No `#[repr(C)] FT_SizeRec`.
- No multiple size list per face.
- No `FT_New_Size`, `FT_Activate_Size`, or `FT_Done_Size` lifecycle.
- Current setters mutate the embedded active metrics directly.

### `FT_Size_Metrics`

C field order:

```text
x_ppem, y_ppem, x_scale, y_scale, ascender, descender, height, max_advance
```

Current `SizeMetrics` has those fields in the same semantic order, then adds
Rust-only request metadata:

```text
x_dpi, y_dpi, char_width, char_height
```

Exactness gaps:

- No C ABI record; extra fields mean current `SizeMetrics` cannot be reused as
  `FT_Size_Metrics`.
- `FT_Set_Char_Size` normalization in current Rust sets each zero DPI to 72,
  while C FreeType first mirrors the non-zero counterpart and only uses 72dpi
  when both resolutions are zero.
- `FT_Request_Size` has no implementation for `NOMINAL`, `REAL_DIM`, `BBOX`,
  `CELL`, or direct `SCALES` request types.
- Bitmap-strike size metrics selected by `FT_Select_Size` are absent.

### `FT_Size_RequestRec`

C field order:

```text
type, width, height, horiResolution, vertResolution
```

`FT_Request_Size` receives this record through the pointer typedef
`FT_Size_Request`.  The `type` enum values are
`FT_SIZE_REQUEST_TYPE_NOMINAL`, `FT_SIZE_REQUEST_TYPE_REAL_DIM`,
`FT_SIZE_REQUEST_TYPE_BBOX`, `FT_SIZE_REQUEST_TYPE_CELL`,
`FT_SIZE_REQUEST_TYPE_SCALES`, and `FT_SIZE_REQUEST_TYPE_MAX`.

Current mapping: none.  `Font::set_char_size` and `Font::set_pixel_sizes`
model two helper paths, but there is no public Rust request record and no
direct implementation of the non-helper request types.

### `FT_Open_Args`

C field order:

```text
flags, memory_base, memory_size, pathname, stream, driver, num_params, params
```

Current mapping exists only for the memory case through `Library::new_memory_face`
and `Face::from_memory`, with a Rust slice replacing `memory_base` plus
`memory_size`.  There is no C ABI `flags` parser, path open wrapper, custom
`FT_Stream`, forced driver, or open parameter list.

Exactness gaps:

- No handling for `FT_OPEN_MEMORY`, `FT_OPEN_STREAM`, `FT_OPEN_PATHNAME`,
  `FT_OPEN_DRIVER`, or `FT_OPEN_PARAMS`.
- No validation that exactly one source flag is selected.
- No C stream close-on-error behavior.
- No negative `face_index` probing through `FT_Open_Face`.
- No named-instance bits in `face_index` for variation fonts.

### `FT_Parameter`

C field order:

```text
tag, data
```

Current mapping: none.  This blocks `FT_Open_Face` parameters and
`FT_Face_Properties`.

Known parameter families for this slice include open-time flags from
`ftparams.h`, face properties such as LCD filter weights and random seed, and
driver properties such as TrueType interpreter version.  The future ABI layer
must preserve tag numeric values and treat `data` as an untyped C pointer with
tag-specific size and lifetime rules.

## Lifecycle And Ownership Semantics

- `FT_New_Face` owns the opened path stream internally; the future ABI wrapper
  may perform filesystem I/O, but core/fontdone must remain pure Rust and path
  free.
- `FT_New_Memory_Face` does not copy `file_base`; callers must keep the memory
  alive until `FT_Done_Face`.  Current Rust parses from `&[u8]` into
  `Arc<FontData>` that owns cloned raw bytes, so lifetime behavior is safer but
  not ABI-identical.
- `FT_Open_Face` creates a face, an initial glyph slot, and a default size
  object.  Current `Face` owns a `Font`; glyph slots are returned as snapshots
  rather than through `face->glyph`.
- `FT_Reference_Face` increments an internal C counter.  `FT_Done_Face`
  decrements and destroys only at zero.  Rust `Clone` is not equivalent because
  C callers expect the same handle and delayed destruction.
- `FT_Done_Face` also discards all child slots and sizes allocated for the
  face.  `FT_Done_Size` discards one size object; `FT_Done_Face` discards all
  remaining sizes.
- `FT_New_Size` creates an inactive size object.  `FT_Activate_Size` updates
  the parent face's `size` pointer; subsequent size setters and glyph loads use
  that active size.
- `FT_Set_Char_Size`, `FT_Set_Pixel_Sizes`, `FT_Request_Size`, and
  `FT_Select_Size` mutate the active `FT_Size` metrics, not just a copied
  metrics record.
- `FT_Attach_File` and `FT_Attach_Stream` mutate a face with driver-specific
  auxiliary data.  For Type 1 AFM/PFM this can affect kerning and metrics.
- `FT_Set_Transform` stores a face-scoped matrix and delta that affect future
  glyph loads/advances, while metrics in `face->glyph->metrics` are not
  transformed the same way as the advance vector.

## Current Test Coverage

`tests/data/interface_map.json` classifies:

- `face/open`: `FT_New_Memory_Face` and `FT_Done_Face` complete,
  `FT_New_Face`, `FT_Open_Face`, `FT_Attach_File`, `FT_Attach_Stream`, and
  `FT_Reference_Face` planned.  Coverage family `core_face_size_charmap`,
  `2/2`.
- `size/select`: `FT_Set_Char_Size` and `FT_Set_Pixel_Sizes` complete,
  `FT_Select_Size`, `FT_Activate_Size`, `FT_Request_Size`, `FT_New_Size`, and
  `FT_Done_Size` planned.  Coverage family `core_face_size_charmap`, `1/1`.

Those tests prove current semantic Rust behavior for the covered helpers; they
do not prove C ABI record layout, exported symbol names, pointer ownership, or
full FreeType error behavior.

As of 2026-07-13, the `FT_New_Size`, `FT_Done_Size`, and
`FT_Activate_Size` null-validation rows are real wrapper validation routes, but
the non-null success sequence rows are not real parity.  A correct
implementation needs a face-owned `FT_Size` handle model that preserves the
initial size, allocates inactive secondary sizes, switches active size by
handle identity, mutates the active size through `FT_Set_*` and
`FT_Request_Size`, and exposes direct C ABI and WASM ABI lifecycle calls.  A
Rust-FFI-only proof or a generic `Unimplemented_Feature` fallback must stay
classified as pending core work.

## Shared Dynamic C/Rust Size Test Shape

Add one generated manifest consumed by both a C oracle helper and a Rust test:

```text
font_path
face_index
operation sequence:
  new_memory_face
  set_char_size(width_26_6, height_26_6, x_dpi, y_dpi)
  set_pixel_sizes(pixel_width, pixel_height)
  request_size(type, width, height, hori_resolution, vert_resolution)
  select_size(strike_index)
  optional load_glyph(glyph_index, load_flags)
outputs:
  error code after each operation
  face->num_faces, face->face_index, face->face_flags, face->style_flags
  face->size pointer changed/unchanged marker for activate-size cases
  face->size->metrics.{x_ppem,y_ppem,x_scale,y_scale,ascender,descender,height,max_advance}
  optional glyph advance and glyph metrics after the final size selection
```

C side: compile a small helper against the pinned FreeType oracle.  It opens
the same font bytes with `FT_New_Memory_Face` or `FT_Open_Face`, applies each
operation in order, and writes the scalar outputs as JSON.

Rust side: read the same manifest and call the current safe API where it
exists (`Face::from_memory`, `set_char_size`, `set_pixel_sizes`).  Future ABI
work should extend the same runner for `FT_Request_Size`, `FT_Select_Size`,
multiple `FT_Size` handles, and `FT_Set_Transform`.

Comparison: exact equality for `FT_Error` values where mapped, all
`FT_Size_Metrics` fields, face scalar fields, and post-size glyph metrics.
Keep request cases that currently lack Rust implementation as explicit planned
or failing rows; do not narrow the manifest to passing helpers only.

# Multiple Master, OpenType Variations, and Variation Selectors

Scope: FreeType 2.14.3 public C API compatibility for `ftmm.h` Multiple
Master/OpenType variation entry points and `freetype.h` Unicode variation
selector queries.

This slice tracks implemented and planned C ABI replacement routes.  The
current `fontdone` Rust facade exposes an Adobe Type 1 MM descriptor route and
generated-fixture weight-vector plus design-coordinate state routes for the
maintained generated fixture; broader Multiple Master multi-scenario
coordinate mutation, glyph interpolation output, and `FT_MM_Var` allocation
remain planned.  It also
exposes `FT_Face_GetCharVariantIndex`,
`FT_Face_GetCharVariantIsDefault`, `FT_Face_GetVariantSelectors`,
`FT_Face_GetVariantsOfChar`, and `FT_Face_GetCharsOfVariant` through a compact
cmap format-14 parser/query path.  Servo `rust-freetype` is a binding
reference only; parity must be proven against the pinned C FreeType oracle.

## C Symbols

| Symbol | Header | C signature | Current mapping |
|---|---|---|---|
| `FT_Get_Multi_Master` | `freetype/ftmm.h` | `FT_Error FT_Get_Multi_Master(FT_Face face, FT_Multi_Master *amaster)` | Partially implemented for generated Type 1 MM descriptors: parses `BlendAxisTypes`, `BlendDesignPositions`, `BlendDesignMap`, and `WeightVector`; fills `FT_Multi_Master` counts and populated `FT_MM_Axis` slots through Rust FFI, C ABI, and WASM ABI. |
| `FT_Get_MM_Var` | `freetype/ftmm.h` | `FT_Error FT_Get_MM_Var(FT_Face face, FT_MM_Var **amaster)` | Planned; no Rust API, no `fvar` parser, and no ABI-owned `FT_MM_Var` allocation model yet. |
| `FT_Done_MM_Var` | `freetype/ftmm.h` | `FT_Error FT_Done_MM_Var(FT_Library library, FT_MM_Var *amaster)` | Planned for C ABI layer; depends on ABI-owned allocation from `FT_Get_MM_Var`. |
| `FT_Set_MM_Design_Coordinates` | `freetype/ftmm.h` | `FT_Error FT_Set_MM_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Long *coords)` | Partially implemented for generated Type 1 MM fixture state: design-map conversion, weight-vector recomputation, follow-up design/blend getter observation, and variation flag behavior are compared through Rust FFI, C ABI, and WASM ABI. Multi-scenario reset and glyph interpolation remain planned. |
| `FT_Set_Var_Design_Coordinates` | `freetype/ftmm.h` | `FT_Error FT_Set_Var_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed *coords)` | Planned; no active variation state or glyph/metric delta application. |
| `FT_Get_Var_Design_Coordinates` | `freetype/ftmm.h` | `FT_Error FT_Get_Var_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed *coords)` | Planned; no active variation state. |
| `FT_Set_MM_Blend_Coordinates` | `freetype/ftmm.h` | `FT_Error FT_Set_MM_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed *coords)` | Planned; no normalized blend coordinate model. |
| `FT_Get_MM_Blend_Coordinates` | `freetype/ftmm.h` | `FT_Error FT_Get_MM_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed *coords)` | Planned; no normalized blend coordinate model. |
| `FT_Set_Var_Blend_Coordinates` | `freetype/ftmm.h` | `FT_Error FT_Set_Var_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed *coords)` | Planned; C documents this as another name for `FT_Set_MM_Blend_Coordinates`. |
| `FT_Get_Var_Blend_Coordinates` | `freetype/ftmm.h` | `FT_Error FT_Get_Var_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed *coords)` | Planned; C documents this as another name for `FT_Get_MM_Blend_Coordinates`. |
| `FT_Set_Named_Instance` | `freetype/ftmm.h` | `FT_Error FT_Set_Named_Instance(FT_Face face, FT_UInt instance_index)` | Partially implemented: existing variable-font named-instance state routes remain, and generated Type 1 MM `FT_Set_Named_Instance(0)` reset-to-default state is compared after prior MM design-coordinate mutation. Broader glyph-output named-instance parity remains planned. |
| `FT_Get_Default_Named_Instance` | `freetype/ftmm.h` | `FT_Error FT_Get_Default_Named_Instance(FT_Face face, FT_UInt *instance_index)` | Planned; no named instance records or synthesized default instance. |
| `FT_Get_Var_Axis_Flags` | `freetype/ftmm.h` | `FT_Error FT_Get_Var_Axis_Flags(FT_MM_Var *master, FT_UInt axis_index, FT_UInt *flags)` | Planned; no `FT_MM_Var` ABI record or axis flags storage. |
| `FT_Set_MM_WeightVector` | `freetype/ftmm.h` | `FT_Error FT_Set_MM_WeightVector(FT_Face face, FT_UInt len, FT_Fixed *weightvector)` | Partially implemented for generated Type 1 MM fixture state: null/non-null validation, short/exact/long copy behavior, reset, zero-fill, unenforced weight sum, and variation flag toggling through Rust FFI, C ABI, and WASM ABI. Glyph interpolation remains planned. |
| `FT_Get_MM_WeightVector` | `freetype/ftmm.h` | `FT_Error FT_Get_MM_WeightVector(FT_Face face, FT_UInt *len, FT_Fixed *weightvector)` | Partially implemented as the observation half of the generated Type 1 MM weight-vector route: required-length error, output write, zero-fill, and current state reporting are compared against pinned C. Standalone legacy fixture success remains pending until its declared asset is resolved. |
| `FT_Face_GetCharVariantIndex` | `freetype/freetype.h` | `FT_UInt FT_Face_GetCharVariantIndex(FT_Face face, FT_ULong charcode, FT_ULong variantSelector)` | Implemented for scalar glyph-index lookup through the active Unicode charmap and cmap format 14 default/non-default UVS records. |
| `FT_Face_GetCharVariantIsDefault` | `freetype/freetype.h` | `FT_Int FT_Face_GetCharVariantIsDefault(FT_Face face, FT_ULong charcode, FT_ULong variantSelector)` | Implemented for scalar cmap format 14 default/non-default UVS classification through the selector charmap. |
| `FT_Face_GetVariantSelectors` | `freetype/freetype.h` | `FT_UInt32 *FT_Face_GetVariantSelectors(FT_Face face)` | Implemented for face-owned zero-terminated selector lists from cmap format-14 selector records. |
| `FT_Face_GetVariantsOfChar` | `freetype/freetype.h` | `FT_UInt32 *FT_Face_GetVariantsOfChar(FT_Face face, FT_ULong charcode)` | Implemented for face-owned zero-terminated selector lists active for one Unicode scalar value. |
| `FT_Face_GetCharsOfVariant` | `freetype/freetype.h` | `FT_UInt32 *FT_Face_GetCharsOfVariant(FT_Face face, FT_ULong variantSelector)` | Implemented for face-owned zero-terminated default plus non-default character lists for one selector. |

The audit inventory tracks all five cmap format-14 Unicode variation selector
symbols in this slice as implemented.  The Type 1 MM descriptor route covers
`FT_MM_Axis.populated_by_get_multi_master` and
`FT_Multi_Master.populated_by_adobe_mm_service`; the generated Adobe MM
weight-vector route covers the fixture-backed setter state rows and getter
observation used by those rows.  The generated Adobe MM design-coordinate
route covers the direct state row and named-instance reset-to-default state.
Multiple Master multi-scenario reset rows, `FT_MM_Var` allocation, OpenType
variation state APIs, standalone legacy fixture weight-vector success, and
glyph-output interpolation remain planned.

## ABI Records

The future C ABI layer must expose these records with `#[repr(C)]`, exact C
field order, C integer widths, pointer fields, and ownership rules.

```c
typedef struct FT_MM_Axis_ {
  FT_String *name;
  FT_Long minimum;
  FT_Long maximum;
} FT_MM_Axis;

typedef struct FT_Multi_Master_ {
  FT_UInt num_axis;
  FT_UInt num_designs;
  FT_MM_Axis axis[T1_MAX_MM_AXIS];
} FT_Multi_Master;

typedef struct FT_Var_Axis_ {
  FT_String *name;
  FT_Fixed minimum;
  FT_Fixed def;
  FT_Fixed maximum;
  FT_ULong tag;
  FT_UInt strid;
} FT_Var_Axis;

typedef struct FT_Var_Named_Style_ {
  FT_Fixed *coords;
  FT_UInt strid;
  FT_UInt psid;
} FT_Var_Named_Style;

typedef struct FT_MM_Var_ {
  FT_UInt num_axis;
  FT_UInt num_designs;
  FT_UInt num_namedstyles;
  FT_Var_Axis *axis;
  FT_Var_Named_Style *namedstyle;
} FT_MM_Var;
```

Record details that tests must pin:

- `FT_Multi_Master::axis` is an inline array of `T1_MAX_MM_AXIS` Adobe MM axis
  records; FreeType caps Adobe MM axes at 4 and designs at 16.
- `FT_Var_Axis::{minimum,def,maximum}` are 16.16 `FT_Fixed` values for
  TrueType GX/OpenType Variations and whole-number design values for Adobe MM.
- `FT_Var_Axis::tag` must preserve four-byte OpenType axis tags such as
  `wght`, `wdth`, `opsz`, and custom tags; `strid` is the name table ID.
- `FT_Var_Named_Style::coords` points to one 16.16 design coordinate per axis;
  `psid == 0xFFFF` means the PostScript name entry is missing.
- `FT_MM_Var::{axis,namedstyle}` are internally allocated by
  `FT_Get_MM_Var`; C callers release the full allocation with
  `FT_Done_MM_Var(library, amaster)`.
- `FT_VAR_AXIS_FLAG_HIDDEN` has numeric value `1` and is returned through
  `FT_Get_Var_Axis_Flags`.

## Required Core Work

The implementation needs a pure-Rust variation subsystem before the C ABI
symbols can become more than stubs:

- Parse `fvar` axes, named instances, axis flags, name IDs, PostScript name
  IDs, and default coordinates.
- Parse `avar` segment maps and convert design coordinates to normalized blend
  coordinates with FreeType-compatible clamping, defaults, and missing-axis
  behavior.
- Apply `gvar` glyph deltas to simple and composite glyph outlines before
  hinting and metrics finalization.
- Apply `cvar`, `HVAR`, `VVAR`, and `MVAR` deltas where FreeType changes CVT,
  advances, vertical metrics, and face/size metrics.
- Track active variation state on the face: selected named instance, design
  coordinates, normalized blend coordinates, and `FT_FACE_FLAG_VARIATION`
  semantics.
- Preserve FreeType `face_index` named-instance encoding: bits 16-30 hold the
  selected instance index, bit 31 stays clear, and direct coordinate changes do
  not update the named instance bits.
- Complete the remaining cmap format 14 UVS queries with Unicode-charmap
  checks, default/non-default distinction, sorted zero-terminated result lists,
  and face-owned scratch-buffer lifetime.  Scalar glyph-index lookup is already
  routed through `FT_Face_GetCharVariantIndex`.
- Add the C ABI allocation boundary for `FT_Get_MM_Var` and
  `FT_Done_MM_Var`; safe Rust wrappers may own `Vec`/`String` data, but ABI
  callers must see stable C pointers until `FT_Done_MM_Var`.

## Fixture Coverage

Add maintained fixtures through `pillow-rs-freetype/scripts/`; do not hand-edit
fixture JSON or oracle outputs.  C FreeType remains the oracle.

Minimum font coverage:

- A single-axis variable TrueType font with `wght`, at least one named instance,
  and glyph outlines affected by `gvar`.
- A multi-axis variable TrueType font covering at least `wght`, `wdth`, and
  `opsz`, with non-default `avar` mappings and several named instances.
- A variable font with hidden axis flags so `FT_Get_Var_Axis_Flags` proves both
  zero and `FT_VAR_AXIS_FLAG_HIDDEN`.
- A font with metric variation tables: at least one of `HVAR`, `VVAR`, and
  `MVAR`; prefer coverage for all three before marking metrics parity complete.
- A font with `cvar` so CVT variation is covered before native TrueType
  bytecode execution.
- A cmap format 14 font with both default and non-default UVS mappings,
  including at least one ideographic variation selector and one standardized
  variation selector where available.
- If Adobe MM support is in scope for the replacement layer, a deterministic
  Type 1 Multiple Master fixture that exercises `FT_Get_Multi_Master`,
  `FT_Set_MM_Design_Coordinates`, and `FT_Set/Get_MM_WeightVector`.

Fixture rows should include the font path, face index including named-instance
bits when relevant, axis tags, min/default/max, axis flags, named-style name
IDs, `psid`, design coordinates, normalized blend coordinates, glyph ID,
metrics, outline cbox/bbox, and rendered bitmap metadata/bytes for at least one
representative size.

## Dynamic Tests

Add narrow dynamic tests before broad matrix promotion:

- `FT_Get_MM_Var`/`FT_Done_MM_Var`: compare axis count, named-style count,
  tags, min/default/max, `strid`, `psid`, named-style coordinates, pointer
  nullability, and successful free behavior against C FreeType.
- Design coordinates: set fewer, exact, and extra coordinates; verify missing
  axes use defaults, extra values are ignored, `num_coords == 0 && coords ==
  NULL` resets state, and `FT_Get_Var_Design_Coordinates` zero-fills excess
  output coordinates.
- Blend coordinates: set/get normalized coordinates and verify alias behavior
  for `FT_Set_Var_Blend_Coordinates` and `FT_Get_Var_Blend_Coordinates`;
  excess get coordinates must be `0` for GX/OpenType and `0.5` for Adobe MM.
- Named instances: select index 0, the default named instance, and a
  non-default named instance; verify `face_index` bits, reset of prior
  coordinate variation, `FT_FACE_FLAG_VARIATION`, PostScript name behavior, and
  output metrics/glyph deltas.
- Axis flags: query every axis and one out-of-range index; compare
  `FT_VAR_AXIS_FLAG_HIDDEN` and error code behavior.
- Weight vector: for Adobe MM fixtures, verify shorter/exact/longer lengths,
  reset with length 0, non-enforcement of total weight sum, and
  `FT_Get_MM_WeightVector` required-length error behavior.
- Glyph output: for selected coordinates and named instances, compare glyph
  metrics, cbox/bbox, and render bytes so the API state is tied to actual
  interpolation output.
- UVS scalar queries: compare undefined char, undefined selector, default UVS,
  non-default UVS, selector larger than 32 bits, and non-Unicode current
  charmap behavior.
- UVS list queries: compare full zero-terminated arrays from
  `FT_Face_GetVariantSelectors`, `FT_Face_GetVariantsOfChar`, and
  `FT_Face_GetCharsOfVariant`, including empty/invalid `NULL` cases and
  overwrite-on-next-FreeType-call lifetime behavior for the C ABI layer.

## C Reference Areas

Use these FreeType source areas when implementing and debugging first
divergences:

- Public dispatch and service behavior: `src/base/ftmm.c`.
- TrueType/OpenType variation parsing, coordinate state, named instances, and
  glyph/metric deltas: `src/truetype/ttgxvar.c`.
- TrueType variation service table: `src/truetype/ttdriver.c`.
- Adobe Type 1 MM descriptors and weight vectors: `src/type1/t1load.c`.
- Variation selector public functions: `src/base/ftobjs.c`.
- cmap format 14 parser and query algorithms: `src/sfnt/ttcmap.c`.

## Current Risk

This slice is partially implemented for generated Adobe Type 1 MM descriptor,
weight-vector state, design-coordinate state, named-instance reset state, and
cmap format-14 UVS queries.  A compact generated
`fonts/type1-mm/adobe-mm-two-axis.pfb` fixture now exists and pinned C
FreeType opens it as a two-axis/four-design MM face.  Returning successful ABI
stubs before pure-Rust Type 1 MM parsing, state mutation, allocation
ownership, and oracle-backed dynamic tests exist would create false
compatibility.  Descriptor, weight-vector, design-coordinate, and reset rows
are real only where the same generated fixture passes through pinned C
FreeType, Rust FFI, thin C ABI, and WASM ABI with exact output; remaining
multi-scenario reset, `FT_MM_Var`, broader named-instance, and glyph-output
rows must stay pending until they have the same proof.  Glyph-output rows
additionally require real Type 1 MM interpolation.

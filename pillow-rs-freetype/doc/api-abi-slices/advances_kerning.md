# Advances And Kerning C API/ABI Slice

This slice covers FreeType-compatible advance retrieval, pair kerning, track
kerning, format naming, and the output-visible face/metric helper macros that
control those code paths. The C FreeType oracle remains the target; Servo
`rust-freetype` is useful only as a binding-shape reference.

## Scope

| Area | C symbols and macros | Current fontdone mapping | Status |
|---|---|---|---|
| Quick advances | `FT_Get_Advance`, `FT_Get_Advances`, `FT_ADVANCE_FLAG_FAST_ONLY` | `Font::glyph_hori_advance_26dot6`, `Font::getlength`, `Face::load_glyph(...).metrics.hori_advance` | Partial semantic coverage, no C ABI entry point |
| Pair kerning | `FT_Get_Kerning`, `FT_Kerning_Mode`, `FT_KERNING_DEFAULT`, `FT_KERNING_UNFITTED`, `FT_KERNING_UNSCALED` | `Font::getkerning`, private `Font::glyph_kerning`, `tt::kern::KernTable::get` | Planned C endpoint, partial Rust-only helper |
| Track kerning | `FT_Get_Track_Kerning` | None | Planned or Type 1/AFM-limited exclusion |
| Font format alias | `FT_Get_X11_Font_Format` | `Font::font_format` via `FT_Get_Font_Format` mapping | Interface map marks out of scope; ABI alias still needs a deliberate decision |
| Face/metric helpers | `FT_HAS_HORIZONTAL`, `FT_HAS_VERTICAL`, `FT_HAS_KERNING`, `FT_IS_SCALABLE`, `FT_IS_SFNT`, `FT_IS_FIXED_WIDTH`, `FT_HAS_GLYPH_NAMES` | `Font::face_flags`, `FaceInfo`, `GlyphSlotMetrics`, `SizeMetrics` | Partial; some output-visible flags are not set yet |

## C Surface And Units

### `FT_Get_Advance`

```c
FT_Error FT_Get_Advance(
  FT_Face   face,
  FT_UInt   gindex,
  FT_Int32  load_flags,
  FT_Fixed* padvance);
```

Output is one `FT_Fixed`. If `load_flags` select scaled output, the value is
16.16 pixels. With `FT_LOAD_NO_SCALE`, the value is in font design units.
`FT_LOAD_VERTICAL_LAYOUT` switches the result from horizontal advance to
vertical advance. The value is not transformed by `FT_Set_Transform`.

`FT_ADVANCE_FLAG_FAST_ONLY` is `0x20000000L`. If set, the function must fail
when the requested mode cannot be answered by the driver's quick-advance path.
The future Rust implementation must not silently fall back to full glyph load
when this flag is present and the C oracle reports failure.

### `FT_Get_Advances`

```c
FT_Error FT_Get_Advances(
  FT_Face   face,
  FT_UInt   start,
  FT_UInt   count,
  FT_Int32  load_flags,
  FT_Fixed* padvances);
```

`padvances` is caller-owned storage for at least `count` `FT_Fixed` values.
Each row follows the same units as `FT_Get_Advance`: scaled 16.16 pixels or
unscaled font units, horizontal by default, vertical when
`FT_LOAD_VERTICAL_LAYOUT` is set. Error behavior must match C for null face,
null output, invalid glyph range, unsupported flags, and `FAST_ONLY`.

### `FT_Get_Kerning`

```c
FT_Error FT_Get_Kerning(
  FT_Face    face,
  FT_UInt    left_glyph,
  FT_UInt    right_glyph,
  FT_UInt    kern_mode,
  FT_Vector* akerning);
```

`akerning` is an `FT_Vector`. This API supports horizontal kerning only. The
expected vector is normally `{ x: value, y: 0 }`.

| Mode | C value | Required units |
|---|---:|---|
| `FT_KERNING_DEFAULT` | 0 | Grid-fitted 26.6 pixels with FreeType's small-ppem kerning heuristic |
| `FT_KERNING_UNFITTED` | 1 | Scaled, un-grid-fitted 26.6 pixels using the current horizontal scale |
| `FT_KERNING_UNSCALED` | 2 | Original font units |

For TrueType/OpenType, C can use the legacy `kern` table and, when compiled
with `TT_CONFIG_OPTION_GPOS_KERNING`, basic pair-position GPOS kerning. If both
exist, the `kern` table wins. Current fontdone parses only classic version-0,
format-0 horizontal `kern` subtables and does not expose GPOS kerning.

Missing pairs and faces without extractable kerning must return success with
`akerning = { 0, 0 }`, matching the public C behavior. Invalid mode and null
output errors need explicit oracle rows before implementation.

### `FT_Get_Track_Kerning`

```c
FT_Error FT_Get_Track_Kerning(
  FT_Face   face,
  FT_Fixed  point_size,
  FT_Int    degree,
  FT_Fixed* akerning);
```

`point_size` is 16.16 fractional points. `degree` is the tightness selector:
negative is tighter, positive is looser, zero means no track kerning.
`akerning` is 16.16 fractional points applied uniformly between glyphs.

C FreeType currently supports this through Type 1 AFM track kerning. The
current crate has no Type 1/AFM support and no equivalent data path. For the
first ABI layer, this should either return the same no-data error that C
returns for the supported SFNT fixture set, or be documented as Type 1 planned
until Type 1/AFM loading exists.

### `FT_Get_X11_Font_Format`

```c
const char* FT_Get_X11_Font_Format(FT_Face face);
```

This is the deprecated alias of `FT_Get_Font_Format`. The current interface map
marks it out of scope, while `Font::font_format` already returns `"TrueType"`
or `"CFF"` for supported SFNT wrappers. A C ABI replacement should probably
export the alias and return the same stable string pointer as
`FT_Get_Font_Format`, or keep the out-of-scope decision with an ABI-linkage
reason.

## Current Rust Behavior

`Face::load_glyph` returns a `GlyphSlot` snapshot where
`slot.advance.x == slot.metrics.hori_advance` and `slot.advance.y == 0`.
This maps the common loaded-glyph slot fields, but it is not a replacement for
`FT_Get_Advance` because it returns 26.6 slot metrics after glyph loading, not
quick-advance 16.16 values.

`Font::glyph_hori_advance_26dot6(codepoint)` reads `hmtx.advance_width` for a
Unicode codepoint and scales it with `size_metrics.x_scale`. This is useful for
horizontal design advance checks, but it takes a codepoint instead of a glyph
index, has no flags, has no vertical path, and returns 26.6 rather than the
scaled 16.16 expected by `FT_Get_Advance`.

`Font::getlength(text)` sums `glyph_metrics_for_index_default(...).hori_advance`
for each nonzero glyph without implicit pair kerning. The interface map
currently lists it as the partial mapping for `FT_Get_Advance`, but it is a
text-run helper rather than a C-compatible one-glyph or range endpoint.

`Font::getkerning(left, right)` maps Unicode chars to glyph indices, reads the
legacy `kern` table through `glyph_kerning`, and scales with `x_scale` into
26.6 pixels. It does not expose glyph-index input, `FT_Kerning_Mode`, unscaled
font-unit output, default grid fitting, small-ppem shrink behavior, GPOS
fallback, vertical vector handling, or C error/null-pointer behavior.

`GlyphSlotMetrics` already carries `hori_advance` and `vert_advance` in 26.6
pixels. Vertical advances are read from `vmtx` when present, otherwise
synthesized from OS/2 typo or hhea ascender/descender. That supports loaded
slot metrics, but the quick-advance ABI still needs exact C unit conversion and
flag handling.

## Flags And Exact Output Requirements

| Input flag or macro | C requirement | Current gap |
|---|---|---|
| `FT_LOAD_NO_SCALE` | Return advance or kerning in font units where applicable | No public advance/kerning mode switch |
| `FT_LOAD_VERTICAL_LAYOUT` | Use vertical advances for quick-advance APIs | No public quick vertical advance API |
| `FT_LOAD_NO_HINTING` | Advance must match C's unhinted quick or loaded path and scaled 16.16 unit | Current helpers return 26.6 or loaded slot metrics |
| Default load flags | Match C hinted advance behavior, including native TrueType phantom-point effects | Slot metrics have parity lanes, quick 16.16 advance has no dedicated tests |
| `FT_ADVANCE_FLAG_FAST_ONLY` | Return C's fast-path success or failure exactly | Not implemented |
| `FT_KERNING_DEFAULT` | Return grid-fitted 26.6 kerning with small-ppem heuristic | Current helper scales but does not grid-fit or shrink exactly |
| `FT_KERNING_UNFITTED` | Return scaled, unrounded 26.6 kerning | Current helper is close for legacy `kern`, but lacks mode exposure and C errors |
| `FT_KERNING_UNSCALED` | Return original font units | Current helper always scales |
| `FT_HAS_KERNING(face)` | True iff `face_flags` has `FT_FACE_FLAG_KERNING` and kerning can be extracted | `Font::face_flags` never sets kerning today |
| `FT_HAS_VERTICAL(face)` | True iff real vertical metrics are present | `Font::face_flags` never sets vertical today, despite parsing `vhea`/`vmtx` |

Exact output means integer equality with C FreeType for:

- returned `FT_Error` code;
- written scalar or vector values;
- no writes beyond caller-provided output storage;
- output value preservation or zeroing behavior on error, after C oracle rows
  define it;
- face flag bits that callers use before selecting these APIs.

## Missing APIs And ABI Records

- Exported C ABI symbols for `FT_Get_Advance`, `FT_Get_Advances`,
  `FT_Get_Kerning`, `FT_Get_Track_Kerning`, and probably
  `FT_Get_X11_Font_Format`.
- Exact constants for `FT_ADVANCE_FLAG_FAST_ONLY`, `FT_KERNING_DEFAULT`,
  `FT_KERNING_UNFITTED`, `FT_KERNING_UNSCALED`, `FT_LOAD_VERTICAL_LAYOUT`,
  `FT_LOAD_NO_SCALE`, and any load-target bits accepted by quick advances.
- `#[repr(C)]` records and pointer aliases for `FT_Face`, `FT_Vector`,
  `FT_Fixed`, `FT_UInt`, `FT_Int32`, `FT_Int`, and `FT_Error`.
- A glyph-index based Rust core helper for horizontal and vertical quick
  advances with explicit load flags and return unit selection.
- A glyph-index based Rust core helper for kerning with explicit
  `FT_Kerning_Mode`.
- Face flag computation that sets `FT_FACE_FLAG_KERNING` and
  `FT_FACE_FLAG_VERTICAL` only when C would set them for the same face.
- GPOS kerning decision: implement basic pair-position extraction when the
  pinned C oracle has it enabled, or document the compile-option difference as
  an expected incompatibility until the parser exists.

## Dynamic Test Rows

Add oracle-generated dynamic rows rather than hard-coded constants. Each row
should record font path, face index, size request, glyph indices, load flags,
C error, and C output bytes or scalar values.

| Row | Font/input | C call | Expected coverage |
|---|---|---|---|
| `advance_h_default_A` | DejaVuSans, char `A`, default char size | `FT_Get_Advance(face, glyph_A, FT_LOAD_DEFAULT, &advance)` | Horizontal scaled 16.16 advance, default hinted path |
| `advance_h_no_hinting_A` | Same glyph and size | `FT_Get_Advance(face, glyph_A, FT_LOAD_NO_HINTING, &advance)` | Horizontal unhinted scaled 16.16 output |
| `advance_h_no_scale_A` | Same glyph, no scale | `FT_Get_Advance(face, glyph_A, FT_LOAD_NO_SCALE, &advance)` | Font-unit output, no accidental 26.6 conversion |
| `advances_range_default` | Consecutive glyph range containing `.notdef`, `A`, `V` when possible | `FT_Get_Advances(face, start, count, FT_LOAD_DEFAULT, advances)` | Range order, count handling, same values as single-call rows |
| `advance_v_real_metrics` | A fixture with `vhea` and `vmtx` | `FT_Get_Advance(..., FT_LOAD_VERTICAL_LAYOUT, &advance)` | Real vertical advance, `FT_HAS_VERTICAL` true |
| `advance_v_synthesized_metrics` | SFNT fixture without `vhea`/`vmtx` | `FT_Get_Advance(..., FT_LOAD_VERTICAL_LAYOUT, &advance)` | C's synthesized vertical advance and `FT_HAS_VERTICAL` false |
| `advance_fast_only_default` | DejaVuSans default hinted glyph | `FT_Get_Advance(..., FT_LOAD_DEFAULT | FT_ADVANCE_FLAG_FAST_ONLY, &advance)` | Exact C fast-path success or failure |
| `kerning_AV_default` | A font with legacy `kern` pair `A,V` | `FT_Get_Kerning(face, glyph_A, glyph_V, FT_KERNING_DEFAULT, &vec)` | Grid-fitted 26.6 x value, y zero, small-ppem heuristic |
| `kerning_AV_unfitted` | Same pair | `FT_Get_Kerning(..., FT_KERNING_UNFITTED, &vec)` | Scaled unrounded 26.6 x value |
| `kerning_AV_unscaled` | Same pair | `FT_Get_Kerning(..., FT_KERNING_UNSCALED, &vec)` | Raw font-unit x value |
| `kerning_missing_pair` | Valid glyph pair not present in kerning data | `FT_Get_Kerning(..., FT_KERNING_DEFAULT, &vec)` | Success with `{0, 0}` |
| `kerning_no_kern_face` | Font without `kern` and without basic GPOS kerning | `FT_Get_Kerning(..., FT_KERNING_DEFAULT, &vec)` | Success with `{0, 0}`, `FT_HAS_KERNING` false |
| `kerning_gpos_only_face` | Font with basic GPOS pair kerning and no `kern` | `FT_Get_Kerning(..., FT_KERNING_DEFAULT, &vec)` | Documents whether pinned C build exposes GPOS kerning |
| `track_kerning_sfnt` | DejaVuSans or LiberationSerif | `FT_Get_Track_Kerning(face, 12<<16, 0, &value)` | Exact C no-data behavior for supported SFNT fonts |
| `x11_font_format_truetype` | DejaVuSans | `FT_Get_X11_Font_Format(face)` | Alias string and null-on-error behavior |

Also include negative ABI rows for null output pointers, null face handles,
invalid kerning mode, `FT_Get_Advances` with `count == 0`, and out-of-range
glyph start/count once the C harness defines exact error and write behavior.

## Implementation Risks

- Unit mismatch is the largest risk: existing helpers mostly return 26.6 slot
  metrics, while quick-advance APIs return scaled 16.16 values unless
  unscaled output is requested.
- Default quick advances and loaded slot advances can diverge for hinted
  TrueType glyphs. The implementation needs C oracle rows before assuming that
  `slot.metrics.hori_advance` can be shifted into 16.16.
- `FT_ADVANCE_FLAG_FAST_ONLY` is observable. Implementing it as a no-op would
  hide driver capability differences and break callers that probe fast paths.
- Current kerning covers only legacy `kern` format 0. C FreeType may expose
  basic GPOS kerning depending on build options, and the pinned oracle's option
  must drive the compatibility target.
- `FT_KERNING_DEFAULT` is not just scaled `kern`; it is grid-fitted and has
  FreeType's small-size heuristic. This needs direct oracle fixtures at several
  ppem sizes.
- Face flags currently omit vertical and kerning bits, so callers using
  `FT_HAS_VERTICAL` or `FT_HAS_KERNING` would skip APIs that fontdone can
  partially answer.
- Vertical metrics are synthesized when real vertical tables are absent, but
  `FT_HAS_VERTICAL` must remain false in that case. Tests need both real and
  synthesized vertical rows.
- Track kerning is Type 1/AFM-specific in C. Returning success with zero for
  all faces may be wrong; the exact SFNT no-data error should be captured from
  C before adding the ABI symbol.
- `FT_Get_X11_Font_Format` is deprecated but still an ABI symbol that existing
  C clients can link. The current out-of-scope mapping may need to change from
  feature scope to alias support.

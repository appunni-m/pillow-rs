# Cache, Modules, And Exclusions Slice

Baseline: `d76593a2`

The migration target is the public FreeType C surface. A symbol should remain
`out_of_scope` only when it cannot affect ordinary C migration or when the
replacement crate intentionally does not support that FreeType subsystem. If C
applications can call it from installed FreeType headers, the default status is
`planned`.

## Decision Classes

| Class | Meaning | Test expectation |
| --- | --- | --- |
| `must_implement` | Needed for credible FreeType C replacement. | Dynamic C/Rust runner or C ABI compile/link check. |
| `planned_feature` | Public and valid, but can ship behind a documented feature milestone. | Audit-visible planned status with fixture plan. |
| `true_exclusion` | Not applicable to this pure Rust replacement or requires unsupported third-party subsystem. | Audit-visible exclusion reason and compile-time error/error-code policy. |

## Current Risk

The generated audit reports 105 `out_of_scope` functions. Several are not true
exclusions for a C-compatible replacement; they are public FreeType APIs that
must either be implemented or deliberately returned as unsupported with exact
documented behavior in the future C ABI layer.

## FTC Cache APIs

Symbols:

- `FTC_Manager_New`, `FTC_Manager_Done`, `FTC_Manager_Reset`
- `FTC_Manager_LookupFace`, `FTC_Manager_LookupSize`,
  `FTC_Manager_RemoveFaceID`
- `FTC_CMapCache_New`, `FTC_CMapCache_Lookup`
- `FTC_ImageCache_New`, `FTC_ImageCache_Lookup`,
  `FTC_ImageCache_LookupScaler`
- `FTC_SBitCache_New`, `FTC_SBitCache_Lookup`,
  `FTC_SBitCache_LookupScaler`
- `FTC_Node_Unref`

Decision: `planned_feature`.

Reason: cache APIs are public but are a convenience subsystem over face loading,
size selection, charmap lookup, image glyph creation, and small bitmap lookup.
They are not needed to prove glyph raster parity, but a C replacement must not
pretend they do not exist.

Implementation plan:

- define C ABI cache manager handles as opaque Rust-owned objects;
- require a callback bridge for `FTC_Face_Requester` in the C ABI layer;
- reuse existing face/size/glyph runners for cache result parity;
- compare cache lookup outputs, not internal eviction order, except where node
  reference behavior is public.

Tests:

- C compile/link test for every `FTC_*` symbol;
- deterministic cache rows with repeated face IDs, eviction pressure, cmap
  lookup, image glyph lookup, and sbit lookup;
- exact output parity for returned glyph/bitmap/charmap values.

## Module And Property APIs

Symbols:

- `FT_Add_Default_Modules`, `FT_Add_Module`, `FT_Remove_Module`,
  `FT_Get_Module`, `FT_New_Library`, `FT_Done_Library`,
  `FT_Reference_Library`
- `FT_Property_Get`, `FT_Property_Set`, `FT_Set_Default_Properties`
- `FT_Get_TrueType_Engine_Type`
- `FT_Set_Debug_Hook`
- `FT_Get_Renderer`, `FT_Set_Renderer`

Decision: mixed.

| Group | Decision | Reason |
| --- | --- | --- |
| library/module lifecycle | `must_implement` | C users can create libraries with custom memory and query/add modules. |
| property APIs | `must_implement` for known modules, `planned_feature` for unsupported modules | Autohint, TrueType interpreter, LCD filter, and driver properties affect public output. |
| renderer APIs | `planned_feature` | Renderer registration is public, but pure Rust may support only built-in renderers initially. |
| debug hook | `planned_feature` | Public but diagnostic. Must not crash and should preserve documented hook slots. |

Tests:

- constant record layout for `FT_Module`, `FT_Module_Class`, `FT_Parameter`;
- property set/get rows for supported interpreter/autohint/LCD properties;
- exact output rows proving properties alter render results where C does.

## List APIs

Symbols:

- `FT_List_Add`, `FT_List_Insert`, `FT_List_Find`, `FT_List_Remove`,
  `FT_List_Up`, `FT_List_Iterate`, `FT_List_Finalize`

Decision: `must_implement` for the C ABI layer.

Reason: list helpers are public utility APIs. They do not require glyph logic,
but C migration code may call them independently.

Tests:

- C ABI runner that mutates a small list and emits node order;
- iterator callback event stream comparison;
- destructor callback count and user data identity checks.

## Glyph Object And Stroker APIs

Symbols:

- `FT_New_Glyph`, `FT_Done_Glyph`, `FT_Get_Glyph`, `FT_Glyph_Copy`,
  `FT_Glyph_Transform`, `FT_Glyph_Get_CBox`, `FT_Glyph_To_Bitmap`
- `FT_Glyph_Stroke`, `FT_Glyph_StrokeBorder`
- `FT_Stroker_New`, `FT_Stroker_Done`, `FT_Stroker_Set`,
  `FT_Stroker_Rewind`, `FT_Stroker_ParseOutline`, `FT_Stroker_BeginSubPath`,
  `FT_Stroker_EndSubPath`, `FT_Stroker_LineTo`, `FT_Stroker_ConicTo`,
  `FT_Stroker_CubicTo`, `FT_Stroker_GetCounts`,
  `FT_Stroker_GetBorderCounts`, `FT_Stroker_Export`,
  `FT_Stroker_ExportBorder`

Decision: `must_implement`.

Reason: glyph object APIs are a normal part of FreeType usage. Stroker output
affects outlines and bitmaps and can be tested through the same outline/bitmap
schemas.

Tests:

- glyph copy/transform/cbox rows over outline and bitmap glyphs;
- stroked outline point/tag/contour parity;
- stroked bitmap parity after `FT_Glyph_To_Bitmap`;
- callback trace rows for path construction APIs.

## Color, Palette, COLR/CPAL, And SVG APIs

Symbols include:

- `FT_Palette_Data_Get`, `FT_Palette_Select`,
  `FT_Palette_Set_Foreground_Color`
- `FT_Get_Color_Glyph_Layer`, `FT_Get_Color_Glyph_Paint`,
  `FT_Get_Color_Glyph_ClipBox`, `FT_Get_Colorline_Stops`,
  `FT_Get_Paint`, `FT_Get_Paint_Layers`
- SVG document hooks and color paint records from `ftcolor.h` when exposed by
  the pinned header set.

Decision: `planned_feature`.

Reason: color fonts are public and user-visible. They can be staged after mono,
gray, LCD, metrics, and outline parity, but they must remain visible in the
audit.

Tests:

- fixture fonts with COLR v0, COLR v1, CPAL palettes, foreground color, and
  layered glyphs;
- exact paint graph JSON, palette records, clip boxes, and layer iteration;
- no pixel thresholding for rendered color output when implemented.

## Validation APIs

Symbols:

- `FT_OpenType_Validate`, `FT_OpenType_Free`
- `FT_TrueTypeGX_Validate`, `FT_TrueTypeGX_Free`
- `FT_ClassicKern_Validate`, `FT_ClassicKern_Free`

Decision: `planned_feature`.

Reason: validators are public diagnostics. They may not be required for common
rendering, but migration users can call them.

Tests:

- valid/invalid fixture fonts;
- exact error codes and returned table byte ownership/free behavior.

## Font-Format-Specific Helper APIs

Groups:

- BDF: `FT_Get_BDF_Charset_ID`, `FT_Get_BDF_Property`
- CID: `FT_Get_CID_Registry_Ordering_Supplement`,
  `FT_Get_CID_Is_Internally_CID_Keyed`, `FT_Get_CID_From_Glyph_Index`
- PFR: `FT_Get_PFR_Metrics`, `FT_Get_PFR_Kerning`, `FT_Get_PFR_Advance`
- Type 1/PostScript: `FT_Get_PS_Font_Info`, `FT_Get_PS_Font_Private`,
  `FT_Get_PS_Font_Value`, `FT_Has_PS_Glyph_Names`
- WinFNT: `FT_Get_WinFNT_Header`

Decision: `planned_feature` unless the project explicitly narrows supported
font formats for an initial release.

Reason: these APIs are public and format-specific. Excluding them globally would
break migration for users who rely on non-TrueType faces.

Tests:

- one fixture font per supported format;
- exact records, strings, and error codes for unsupported format calls on
  TrueType fonts;
- audit must show unsupported formats as planned, not silently absent.

## Compression Stream APIs

Symbols:

- `FT_Stream_OpenGzip`, `FT_Gzip_Uncompress`
- `FT_Stream_OpenLZW`
- `FT_Stream_OpenBzip2`

Decision: `planned_feature`.

Reason: these are optional FreeType helpers. A pure Rust implementation can use
Rust decompression crates, but runtime code must not call C libraries through
FreeType.

Tests:

- compressed font/table byte fixtures;
- exact decompressed bytes and error classification for corrupt streams;
- feature-gated dependency audit.

## Synthesis APIs

Symbols:

- `FT_GlyphSlot_Embolden`, `FT_GlyphSlot_Oblique`,
  `FT_GlyphSlot_AdjustWeight`, `FT_GlyphSlot_Slant`,
  `FT_GlyphSlot_Own_Bitmap`

Decision: `must_implement`.

Reason: these mutate public glyph slot state and directly affect bitmap/outline
outputs. They are not optional for output parity.

Tests:

- slot metrics, bbox, outline, and bitmap before/after synthesis;
- exact `bitmap_left`, `bitmap_top`, advance, and buffer bytes.

## Logging And Error Strings

Symbols:

- `FT_Error_String`
- `FT_Set_Log_Handler`, `FT_Set_Default_Log_Handler`

Decision: `planned_feature`.

Reason: diagnostics do not affect glyph output but are public. The C ABI layer
should provide stable behavior. If build-time FreeType C uses optional error
strings, the replacement must choose and document one exact behavior.

Tests:

- error code to string rows;
- handler invocation rows if logging support is enabled.

## True Exclusions Candidate List

No current public FreeType function should be permanently excluded without a
written compatibility policy. Acceptable future true exclusions may include:

- APIs for compile-time-disabled FreeType modules when the replacement publishes
  an explicit feature matrix and matching error behavior;
- debug-only hooks that FreeType itself excludes from release headers for a
  given configuration;
- native platform stream helpers if the replacement intentionally exposes only
  memory-buffer loading in its C ABI. This would be a migration tradeoff and
  must be approved at project level.

## Audit Changes Needed

The audit status map should be tightened:

- move glyph object, stroker, synthesis, list, and core module lifecycle APIs
  from `out_of_scope` to `planned` or `must_implement`;
- split color, validation, compression, and format-specific helpers into named
  planned feature groups;
- keep true exclusions visible with a reason and a future C ABI error policy;
- make the report fail when a newly discovered public C symbol lacks a decision.

## Remaining Risks

- Implementing all public helper modules increases scope beyond current
  TrueType/Pillow integration, but it is required for a credible FreeType C
  replacement.
- Callback-heavy APIs need careful ownership and panic-boundary handling in the
  future C ABI layer.
- Optional FreeType build configuration can change visible behavior. The oracle
  generator must record configure flags and module availability.

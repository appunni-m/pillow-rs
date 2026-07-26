# Font FreeType Surface Sync

This document tracks the boundary between Pillow's `ImageFont.FreeTypeFont`
behavior, Pillow's `_imagingft.c` FreeType calls, and `pillow-rs`'s pure-Rust
Font adapter.

## Rule

`pillow-rs` exposes Pillow-style Font APIs only. Raw FreeType-shaped access is
isolated to `pillow-rs/src/font/imagingft.rs`; Python and JavaScript bindings
must call `pillow_rs::...` root APIs only.

`pillow-rs-freetype` is not changed as part of this cleanup. Broader FreeType
compatibility exports remain there until a separate usage audit proves they are
unused by its own parity, ABI, or fixture workflows.

## Pillow `_imagingft.c` FreeType calls

Observed in Pillow 12.2.0 `src/_imagingft.c`:

| FreeType call | Pillow use | `pillow-rs` Font status |
| --- | --- | --- |
| `FT_Init_FreeType` | module/library setup | used |
| `FT_Library_Version` | exposes FreeType version | not used by Font behavior |
| `FT_New_Face` | path-based font open | binding reads bytes, core uses memory face |
| `FT_New_Memory_Face` | bytes-based font open | used |
| `FT_Request_Size` | nominal Pillow point size | used |
| `FT_Select_Charmap` | explicit encoding charmap selection | missing from current Font surface |
| `FT_Get_Char_Index` | BASIC layout glyph mapping | used |
| `FT_Load_Glyph` | layout, bbox, render | used |
| `FT_Get_Kerning` | BASIC layout kerning | used |
| `FT_Get_Glyph` | bbox/stroke glyph copy | not used; current bbox reads slot cbox |
| `FT_Glyph_Get_CBox` | bbox from copied glyph | not used; current bbox reads slot cbox |
| `FT_Done_Glyph` | copied glyph cleanup | not used |
| `FT_Glyph_To_Bitmap` | stroked glyph render | not used |
| `FT_Bitmap_Init` | conversion setup | not used |
| `FT_Bitmap_Convert` | low-bit-depth bitmap conversion | not used |
| `FT_Bitmap_Done` | conversion cleanup | not used |
| `FT_Stroker_New` | stroke rendering | missing from current Font surface |
| `FT_Stroker_Set` | stroke rendering | missing from current Font surface |
| `FT_Glyph_Stroke` | stroke rendering | missing from current Font surface |
| `FT_Glyph_StrokeBorder` | stroke rendering | missing from current Font surface |
| `FT_Stroker_Done` | stroke cleanup | missing from current Font surface |
| `FT_Palette_Set_Foreground_Color` | color glyph foreground | missing from current Font surface |
| `FT_Get_MM_Var` | variation names/axes | only `has_variations` is exposed |
| `FT_Done_MM_Var` | variation cleanup | not used |
| `FT_Get_Sfnt_Name_Count` | variation display names | not used |
| `FT_Get_Sfnt_Name` | variation display names | not used |
| `FT_Set_Named_Instance` | set variation by name | missing from current Font surface |
| `FT_Set_Var_Design_Coordinates` | set variation by axes | missing from current Font surface |
| `FT_Done_Face` | face cleanup | owned by Rust drop semantics |

## Current `pillow-rs` `fontdone::ffi` usage

Runtime use is isolated to `pillow-rs/src/font/imagingft.rs`.

Function endpoints used:

- `FT_Init_FreeType`
- `FT_New_Memory_Face`
- `FT_Request_Size`
- `FT_Get_Char_Index`
- `FT_Get_Kerning`
- `FT_Load_Glyph`

Constants, flags, error codes, and data structs are used only to drive those
paths or classify their results. They are not public `pillow-rs` API.

## Known Font parity gaps

The live Font parity harness currently passes for the maintained fixture corpus,
but the corpus does not yet cover every Pillow `FreeTypeFont` behavior.

Known gaps to add fixtures or implementation for:

- `getlength` mode-sensitive behavior for `"1"` and `"RGBA"`.
- `getlength` errors for unsupported `direction`, `features`, and `language`
  when RAQM is absent.
- `getbbox` via Pillow's `FT_Get_Glyph` + `FT_Glyph_Get_CBox` call path.
- Bitmap conversion for `FT_PIXEL_MODE_GRAY2` and `FT_PIXEL_MODE_GRAY4`.
- Color glyph/BGRA handling and foreground palette color.
- Stroke rendering.
- Variation names, axes, and setters.
- Constructor encoding/charmap selection.
- `font_variant`.

## `pillow-rs-freetype` later-audit candidates

These exports are not used by Pillow `_imagingft.c` Font parity directly and
should be reverse-mapped before keeping them public in `pillow-rs-freetype`:

- `FT_Outline_Glyph_CBox`
- `FT_Outline_Glyph_To_Bitmap`
- `FT_Outline_Glyph_Copy`
- `FT_Outline_Render_Direct_Spans`
- `FT_Outline_Decompose_Trace`
- `FT_Outline_Render_Error_Output`
- `FT_Glyph_To_Script_Map_Sample_For_Test`
- `FT_Palette_Set_Active_Entry_For_Test`
- `FT_GlyphSlot_Own_Bitmap_Copy_Allocation_Failure`
- `FT_New_Library_Without_Default_Modules`
- `FT_Library_Debug_Hook_Classes`
- `FT_Library_Synthetic_Module_Info`

Do not remove these until a separate `pillow-rs-freetype` audit proves they are
unused by its public manifest, C/WASM ABI compatibility gates, fixture
generators, tests, and internal implementation paths.

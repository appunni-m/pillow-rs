# FreeType C API/ABI Slice: Charmap, Glyph Names, and SFNT Tables

Baseline: `d76593a2`

Scope owner: Worker 5.  This slice covers character map selection and
iteration, glyph-name lookup, PostScript face names, SFNT table access, and CID
metadata when it intersects OpenType/CFF behavior.

The target is FreeType 2.14.3 public C behavior.  Servo `rust-freetype` is only
a binding reference; it is not the behavior oracle.

## Current State

fontdone has safe Rust semantic coverage for Unicode cmap lookup, cmap
selection by current internal index, cmap selection by platform/encoding pair,
format reporting, PostScript face names, SFNT table directory iteration, and raw
SFNT table byte loading.  It does not yet expose a C ABI layer, C-shaped face
and charmap records, glyph-name lookup, SFNT name records, `FT_Get_Sfnt_Table`
record pointers, cmap language IDs, or CID metadata.

Existing mappings:

| C symbol | Current fontdone mapping | Status | ABI exactness |
|---|---|---:|---|
| `FT_Get_Char_Index` | `Face::get_char_index`, `Font::char_index` | complete safe API | semantic only |
| `FT_Get_First_Char` | `Font::first_char` | complete safe API | semantic only |
| `FT_Get_Next_Char` | `Font::next_char` | complete safe API | semantic only |
| `FT_Select_Charmap` | `Font::select_charmap(platform_id, encoding_id)` | partial shape | semantic only; C selects by `FT_Encoding` |
| `FT_Set_Charmap` | `Font::set_charmap(index)` | partial shape | semantic only; C takes `FT_CharMap` |
| `FT_Get_Charmap_Index` | `Font::charmap_index` | partial shape | semantic only; C takes `FT_CharMap` |
| `FT_Get_CMap_Format` | `Font::charmaps().format` | complete safe API | semantic only |
| `FT_Get_CMap_Language_ID` | none | planned | missing |
| `FT_Get_Name_Index` | none | planned | missing |
| `FT_Get_Glyph_Name` | none | planned | missing |
| `FT_Has_PS_Glyph_Names` | none | out of current Type 1/CID scope, but relevant for TrueType `post` names | missing |
| `FT_Get_Postscript_Name` | `Font::postscript_name` | complete safe API | semantic only |
| `FT_Get_Sfnt_Table` | parsed table structs in `tt::*` | partial | missing C-owned record pointers |
| `FT_Load_Sfnt_Table` | `Font::load_sfnt_table` | complete safe API | semantic only |
| `FT_Sfnt_Table_Info` | `Font::sfnt_table_info`, `Font::sfnt_tables` | complete safe API | semantic only |
| `FT_Get_CID_Registry_Ordering_Supplement` | none | out of current non-SFNT CID scope; needed if OpenType/CFF CID is added | missing |
| `FT_Get_CID_Is_Internally_CID_Keyed` | none | out of current non-SFNT CID scope; needed if OpenType/CFF CID is added | missing |
| `FT_Get_CID_From_Glyph_Index` | none | out of current non-SFNT CID scope; needed if OpenType/CFF CID is added | missing |

## Exact C Interfaces

Charmap selection and iteration from `freetype.h`:

```c
FT_Error FT_Select_Charmap(FT_Face face, FT_Encoding encoding);
FT_Error FT_Set_Charmap(FT_Face face, FT_CharMap charmap);
FT_Int   FT_Get_Charmap_Index(FT_CharMap charmap);
FT_UInt  FT_Get_Char_Index(FT_Face face, FT_ULong charcode);
FT_ULong FT_Get_First_Char(FT_Face face, FT_UInt *agindex);
FT_ULong FT_Get_Next_Char(FT_Face face, FT_ULong char_code, FT_UInt *agindex);
```

Outputs and edge behavior:

| Symbol | Output | Required C behavior |
|---|---|---|
| `FT_Select_Charmap` | `FT_Error` | Sets `face->charmap`; for `FT_ENCODING_UNICODE`, selects the best Unicode cmap, preferring UCS-4 over UCS-2 when both exist. |
| `FT_Set_Charmap` | `FT_Error` | Sets `face->charmap` only if the pointer belongs to `face->charmaps`; rejects OpenType format 14. |
| `FT_Get_Charmap_Index` | `FT_Int` | Returns index into owning `face->charmaps`, or `-1` on error. |
| `FT_Get_Char_Index` | `FT_UInt` | Uses active `face->charmap`; returns glyph index or 0 for missing code. |
| `FT_Get_First_Char` | `FT_ULong`, writes `*agindex` | Returns first character code in active charmap.  Writes 0 if empty.  Return value can be 0 for either empty cmap or a valid first charcode 0; `*agindex` disambiguates. |
| `FT_Get_Next_Char` | `FT_ULong`, writes `*agindex` | Returns next character code strictly after `char_code`.  Writes 0 when exhausted. |

Glyph-name and PostScript name APIs from `freetype.h` and `t1tables.h`:

```c
FT_UInt     FT_Get_Name_Index(FT_Face face, const FT_String *glyph_name);
FT_Error    FT_Get_Glyph_Name(FT_Face face, FT_UInt glyph_index,
                              FT_Pointer buffer, FT_UInt buffer_max);
const char *FT_Get_Postscript_Name(FT_Face face);
FT_Int      FT_Has_PS_Glyph_Names(FT_Face face);
```

Outputs and edge behavior:

| Symbol | Output | Required C behavior |
|---|---|---|
| `FT_Get_Name_Index` | `FT_UInt` | Returns glyph index for a glyph name, or 0 if unavailable or unknown.  Requires glyph-name support. |
| `FT_Get_Glyph_Name` | `FT_Error`, writes `buffer` | Copies ASCII glyph name, truncates to fit, always NUL-terminates.  On failure, sets first byte of `buffer` to 0. |
| `FT_Get_Postscript_Name` | `const char *` | Returns a face-owned pointer, or `NULL`.  Pointer is invalid after `FT_Done_Face`; variation instance names can change after instance selection. |
| `FT_Has_PS_Glyph_Names` | `FT_Int` | Returns true only when PostScript glyph names are reliable; stricter than the `FT_HAS_GLYPH_NAMES` flag for some TrueType fonts. |

SFNT APIs from `tttables.h`:

```c
void    *FT_Get_Sfnt_Table(FT_Face face, FT_Sfnt_Tag tag);
FT_Error FT_Load_Sfnt_Table(FT_Face face, FT_ULong tag, FT_Long offset,
                            FT_Byte *buffer, FT_ULong *length);
FT_Error FT_Sfnt_Table_Info(FT_Face face, FT_UInt table_index,
                            FT_ULong *tag, FT_ULong *length);
FT_ULong FT_Get_CMap_Language_ID(FT_CharMap charmap);
FT_Long  FT_Get_CMap_Format(FT_CharMap charmap);
```

Outputs and edge behavior:

| Symbol | Output | Required C behavior |
|---|---|---|
| `FT_Get_Sfnt_Table` | `void *` | Returns a face-owned pointer to a parsed C record for `FT_SFNT_HEAD`, `MAXP`, `OS2`, `HHEA`, `VHEA`, `POST`, or `PCLT`; returns `NULL` if missing or not loaded. |
| `FT_Load_Sfnt_Table` | `FT_Error`, writes `buffer` and `*length` | Loads raw SFNT bytes.  `tag == 0` means whole font file; since 2.14, `tag == 1` means current face table directory.  If `length == NULL`, load whole table.  If `*length == 0`, write full size and do not copy. |
| `FT_Sfnt_Table_Info` | `FT_Error`, writes `*tag`, `*length` | With `tag == NULL`, ignores `table_index` and writes SFNT table count to `length`.  Invalid index returns `FT_Err_Table_Missing`.  Zero-length SFNT tables are treated as missing. |
| `FT_Get_CMap_Language_ID` | `FT_ULong` | Returns cmap language ID for SFNT charmaps; returns 0 for non-SFNT.  Format 14 returns `0xFFFFFFFF`. |
| `FT_Get_CMap_Format` | `FT_Long` | Returns SFNT cmap format; returns `-1` for non-SFNT or synthetic Unicode charmaps. |

CID APIs from `ftcid.h`:

```c
FT_Error FT_Get_CID_Registry_Ordering_Supplement(FT_Face face,
                                                 const char **registry,
                                                 const char **ordering,
                                                 FT_Int *supplement);
FT_Error FT_Get_CID_Is_Internally_CID_Keyed(FT_Face face, FT_Bool *is_cid);
FT_Error FT_Get_CID_From_Glyph_Index(FT_Face face, FT_UInt glyph_index,
                                     FT_UInt *cid);
```

These are currently out of scope for the TrueType glyf renderer and non-SFNT
Type 1/CID support.  They become in-scope if fontdone adds OpenType/CFF CID-keyed
faces.  At that point, C behavior must include successful
`FT_Get_CID_Is_Internally_CID_Keyed` for CID-keyed fonts in an SFNT wrapper.

## Records And Constants Needed

The future ABI layer must add exact `#[repr(C)]` records and preserve C field
order and widths:

```c
typedef struct FT_CharMapRec_ {
  FT_Face      face;
  FT_Encoding  encoding;
  FT_UShort    platform_id;
  FT_UShort    encoding_id;
} FT_CharMapRec;

typedef struct FT_SfntName_ {
  FT_UShort platform_id;
  FT_UShort encoding_id;
  FT_UShort language_id;
  FT_UShort name_id;
  FT_Byte  *string;
  FT_UInt   string_len;
} FT_SfntName;
```

`FT_Get_Sfnt_Table` also requires face-owned ABI records for at least:

| `FT_Sfnt_Tag` | Numeric value | Required pointed record |
|---|---:|---|
| `FT_SFNT_HEAD` | 0 | `TT_Header` |
| `FT_SFNT_MAXP` | 1 | `TT_MaxProfile` |
| `FT_SFNT_OS2` | 2 | `TT_OS2` |
| `FT_SFNT_HHEA` | 3 | `TT_HoriHeader` |
| `FT_SFNT_VHEA` | 4 | `TT_VertHeader` |
| `FT_SFNT_POST` | 5 | `TT_Postscript` |
| `FT_SFNT_PCLT` | 6 | `TT_PCLT` |
| `FT_SFNT_MAX` | 7 | sentinel |

Current fontdone has parsed Rust table structs for `head`, `hhea`, `maxp`,
`os2`, `post`, and related raw table-directory records.  Missing for C ABI:

- `FT_CharMapRec` storage in `FT_FaceRec::charmaps` plus active
  `FT_FaceRec::charmap`.
- Exact `FT_Encoding` constants and deprecated aliases:
  `FT_ENCODING_NONE = 0`, `FT_ENCODING_MS_SYMBOL = 'symb'`,
  `FT_ENCODING_UNICODE = 'unic'`, `FT_ENCODING_SJIS = 'sjis'`,
  `FT_ENCODING_PRC = 'gb  '`, `FT_ENCODING_BIG5 = 'big5'`,
  `FT_ENCODING_WANSUNG = 'wans'`, `FT_ENCODING_JOHAB = 'joha'`,
  `FT_ENCODING_ADOBE_STANDARD = 'ADOB'`, `FT_ENCODING_ADOBE_EXPERT = 'ADBE'`,
  `FT_ENCODING_ADOBE_CUSTOM = 'ADBC'`, `FT_ENCODING_ADOBE_LATIN_1 = 'lat1'`,
  `FT_ENCODING_OLD_LATIN_2 = 'lat2'`, `FT_ENCODING_APPLE_ROMAN = 'armn'`.
- C error mapping for missing charmaps, invalid charmap pointer, missing tables,
  invalid glyph names, and invalid glyph indices.
- `FT_Sfnt_Tag` constants and deprecated `ft_sfnt_*` aliases.
- TrueType platform, encoding, language, and name ID constants from
  `ttnameid.h`, especially values returned through `FT_CharMapRec` and
  `FT_Get_CMap_Language_ID`.
- `FT_SfntName` and `FT_SfntLangTag` if SFNT name APIs are promoted in this
  slice.
- `FT_FACE_FLAG_GLYPH_NAMES` exact flag population and reliable-name
  distinction for `FT_Has_PS_Glyph_Names`.
- `post` format 2.0 and 2.5 glyph-name parsing, Mac standard glyph-name table,
  and optional Adobe glyph-list fallback if parity requires configured
  PostScript names.
- CID records and output string ownership only if OpenType/CFF CID support is
  added.

## Ownership And Borrow Semantics

FreeType C exposes face-owned borrowed pointers for this slice.  The ABI layer
must pin all C-visible records for the lifetime of `FT_Face` and invalidate them
only at `FT_Done_Face`.

- `FT_CharMap` values are pointers into `face->charmaps`; callers do not free
  them.  `FT_Set_Charmap` must reject pointers not owned by the face.
- `face->charmap` is a borrowed pointer to the active charmap.  It changes after
  `FT_Select_Charmap` or `FT_Set_Charmap`.
- `FT_Get_Postscript_Name` returns a borrowed NUL-terminated string owned by the
  face.  The ABI layer must not return a pointer into a temporary Rust `String`.
- `FT_Get_Glyph_Name` copies into caller memory.  It does not transfer
  ownership and must NUL-terminate within `buffer_max`.
- `FT_Get_Sfnt_Table` returns borrowed pointers to parsed face-owned ABI
  records, not raw big-endian table bytes.  Those records must remain stable
  across calls.
- `FT_Load_Sfnt_Table` copies raw bytes into caller memory and writes length
  through `FT_ULong *`; it does not return a borrowed slice.
- `FT_Sfnt_Table_Info` writes scalar outputs only.
- `FT_SfntName.string`, when added, is a borrowed non-NUL-terminated byte slice
  owned by the face.
- CID registry and ordering strings, if added, are borrowed face-owned strings.

## Dynamic Fixture And Test Shape

The current `core_face_size_charmap` tests cover one fixed font and static
expected values.  C API/ABI compatibility needs dynamic oracle fixtures that
exercise the C call shapes and pointer/length semantics directly.

Recommended charmap fixture generator:

1. For each tracked SFNT test font and face index, open with pinned C FreeType.
2. Record `face->num_charmaps`, each charmap pointer index, `encoding`,
   `platform_id`, `encoding_id`, `FT_Get_CMap_Format`, and
   `FT_Get_CMap_Language_ID`.
3. Record default active charmap index after face creation.
4. For each tested `FT_Encoding`, call `FT_Select_Charmap` and record success
   or exact error plus resulting active index.
5. For each charmap pointer, call `FT_Set_Charmap`, `FT_Get_Charmap_Index`, and
   a bounded `FT_Get_First_Char`/`FT_Get_Next_Char` walk.
6. During iteration, record `(charcode, glyph_index)` pairs and include
   boundary probes: codepoint 0, ASCII letters, BMP edge values, supplementary
   Unicode values, and the final mapped codepoint.
7. Keep iteration bounded in fixture metadata for dense or format 13 cmaps;
   record total count separately when feasible.

Rust parity tests should replay the same face/charmap operations against
fontdone and compare exact active index transitions, returned glyph IDs,
iteration order, missing-code behavior, cmap format, language ID, and error
classification.

Recommended SFNT fixture generator:

1. For each font and face index, call `FT_Sfnt_Table_Info(face, 0, NULL,
   &count)` and record the table count.
2. For every table index, call `FT_Sfnt_Table_Info(face, index, &tag, &length)`
   and record exact tag order and length.
3. For each tag, call `FT_Load_Sfnt_Table(face, tag, 0, NULL, &length)` with
   `length = 0`, then load full bytes and selected slices with non-zero
   offsets.
4. Include `tag == 0` whole-font behavior and `tag == 1` table-directory
   behavior for FreeType 2.14+.
5. Record missing table and out-of-range offset error codes.
6. For `FT_Get_Sfnt_Table`, record which `FT_Sfnt_Tag` values return non-null
   and serialize the public record fields from C for `HEAD`, `MAXP`, `OS2`,
   `HHEA`, `VHEA`, `POST`, and `PCLT`.

Rust tests should compare raw bytes for `FT_Load_Sfnt_Table`, table order and
lengths for `FT_Sfnt_Table_Info`, and exact field values for ABI records returned
by `FT_Get_Sfnt_Table`.  The raw-byte tests must not reinterpret C records as
font-file table bytes; FreeType explicitly separates those surfaces.

Recommended glyph-name fixture generator:

1. Use fonts with TrueType `post` format 2.0 glyph names, fonts with no glyph
   names, and fonts whose names require truncation.
2. Record `FT_HAS_GLYPH_NAMES`, `FT_Has_PS_Glyph_Names`,
   `FT_Get_Postscript_Name`, sampled `FT_Get_Glyph_Name` outputs, truncation at
   small `buffer_max` values, and failure buffer contents.
3. Record inverse `FT_Get_Name_Index` for known names and unknown names.

This lane should remain separate from render parity.  Its pass criteria are
exact scalar values, exact bytes, exact pointer-validity semantics observable
through C tests, and exact error classes.

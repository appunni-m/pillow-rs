# Input JSON Review Checklist

One reviewed input JSON file is required for every subject in `tests/manifest.yaml`.

Files live under:

```text
pillow-rs-freetype/tests/fixtures/inputs/public-api/
```

See `input_json_review_contract.md` for the required shape and worker contract.

Legend:

```text
pending   not assigned
active    assigned to worker
returned  worker files received
merged    files accepted into the input model
blocked   worker found ambiguity needing owner decision
```

| offset | limit | status | owner | first subject | last subject |
| ---: | ---: | --- | --- | --- | --- |
| 0 | 20 | merged | input-wave1-0000 | freetype.FT_Encoding | freetype.FT_ENCODING_OLD_LATIN_2 |
| 20 | 20 | merged | input-wave1-0020 | freetype.FT_ENCODING_PRC | freetype.FT_SIZE_REQUEST_TYPE_SCALES |
| 40 | 20 | merged | input-wave1-0040 | freetype.FT_FACE_FLAG_CID_KEYED | freetype.FT_FSTYPE_BITMAP_EMBEDDING_ONLY |
| 60 | 20 | merged | input-wave1-0060 | freetype.FT_FSTYPE_EDITABLE_EMBEDDING | freetype.FT_LOAD_NO_RECURSE |
| 80 | 20 | merged | input-wave1-0080 | freetype.FT_LOAD_NO_SCALE | freetype.FT_STYLE_FLAG_BOLD |
| 100 | 20 | merged | input-wave1-0100 | freetype.FT_STYLE_FLAG_ITALIC | freetype.FT_Face_GetVariantsOfChar |
| 120 | 20 | merged | input-wave1-0120 | freetype.FT_Face_Properties | freetype.FT_MulDiv |
| 140 | 20 | merged | input-wave2-0140 | freetype.FT_MulFix | freetype.FT_HAS_HORIZONTAL |
| 160 | 20 | merged | input-wave2-0160 | freetype.FT_HAS_KERNING | freetype.FT_Parameter |
| 180 | 20 | merged | input-wave2-0180 | freetype.FT_SizeRec | ftbdf.FT_Get_BDF_Charset_ID |
| 200 | 20 | merged | input-wave2-0200 | ftbdf.FT_Get_BDF_Property | ftcache.FTC_Manager_RemoveFaceID |
| 220 | 20 | merged | input-wave2-0220 | ftcache.FTC_Manager_Reset | ftcid.FT_Get_CID_Is_Internally_CID_Keyed |
| 240 | 20 | merged | input-wave2-0240 | ftcid.FT_Get_CID_Registry_Ordering_Supplement | ftcolor.FT_COLR_COMPOSITE_DIFFERENCE |
| 260 | 20 | merged | input-wave2-0260 | ftcolor.FT_COLR_COMPOSITE_EXCLUSION | ftcolor.FT_COLR_PAINTFORMAT_COLR_GLYPH |
| 280 | 20 | merged | input-wave2-0280 | ftcolor.FT_COLR_PAINTFORMAT_COLR_LAYERS | ftcolor.FT_Get_Color_Glyph_Paint |
| 300 | 20 | merged | input-wave3-0300 | ftcolor.FT_Get_Colorline_Stops | ftcolor.FT_PaintLinearGradient |
| 320 | 20 | merged | input-wave3-0320 | ftcolor.FT_PaintRadialGradient | ftdriver.TT_INTERPRETER_VERSION_40 |
| 340 | 20 | merged | input-wave3-0340 | ftdriver.FT_Prop_GlyphToScriptMap | fterrdef.FT_Err_Hmtx_Table_Missing |
| 360 | 20 | merged | input-wave3-0360 | fterrdef.FT_Err_Horiz_Header_Missing | fterrdef.FT_Err_Invalid_Offset |
| 380 | 20 | merged | input-wave3-0380 | fterrdef.FT_Err_Invalid_Opcode | fterrdef.FT_Err_Lower_Module_Version |
| 400 | 20 | merged | input-wave3-0400 | fterrdef.FT_Err_Missing_Bbx_Field | fterrdef.FT_Err_Raster_Corrupted |
| 420 | 20 | merged | input-wave3-0420 | fterrdef.FT_Err_Raster_Negative_Height | fterrors.FT_ERR_BASE |
| 440 | 20 | merged | input-wave3-0440 | fterrors.FT_ERR_PREFIX | ftglyph.FT_GLYPH_BBOX_UNSCALED |
| 460 | 20 | merged | input-wave3-0460 | ftglyph.FT_Done_Glyph | ftgxval.FT_VALIDATE_CKERN |
| 480 | 20 | merged | input-wave3-0480 | ftgxval.FT_VALIDATE_GX | ftgxval.FT_VALIDATE_morx_INDEX |
| 500 | 20 | merged | input-wave3-0500 | ftgxval.FT_VALIDATE_opbd | ftimage.FT_Outline_LineTo_Func |
| 520 | 20 | merged | input-wave3-0520 | ftimage.FT_Outline_MoveTo_Func | ftimage.FT_PIXEL_MODE_LCD |
| 540 | 20 | merged | input-wave3-0540 | ftimage.FT_PIXEL_MODE_LCD_V | ftimage.FT_RASTER_FLAG_DIRECT |
| 560 | 20 | merged | input-wave3-0560 | ftimage.FT_RASTER_FLAG_SDF | ftimage.FT_Raster |
| 580 | 20 | merged | input-wave3-0580 | ftincrem.FT_Incremental_FuncsRec | ftlist.FT_List_Find |
| 600 | 20 | merged | input-wave3-0600 | ftlist.FT_List_Insert | ftmm.FT_Get_Var_Axis_Flags |
| 620 | 20 | merged | input-wave3-0620 | ftmm.FT_Get_Var_Blend_Coordinates | ftmodapi.FT_MODULE_HINTER |
| 640 | 20 | merged | input-wave3-0640 | ftmodapi.FT_MODULE_RENDERER | ftmodapi.FT_Module_Class |
| 660 | 20 | merged | input-wave3-0660 | ftmodapi.FT_Module_Interface | ftmoderr.FT_Mod_Err_PSnames |
| 680 | 20 | merged | input-wave3-0680 | ftmoderr.FT_Mod_Err_Raster | ftoutln.FT_ORIENTATION_FILL_LEFT |
| 700 | 20 | merged | input-wave3-0700 | ftoutln.FT_ORIENTATION_FILL_RIGHT | ftparams.FT_PARAM_TAG_IGNORE_PREFERRED_SUBFAMILY |
| 720 | 20 | merged | input-wave3-0720 | ftparams.FT_PARAM_TAG_IGNORE_SBIX | ftrender.FT_Renderer_Class |
| 740 | 20 | merged | input-wave3-0740 | ftsizes.FT_Activate_Size | ftstroke.FT_STROKER_LINEJOIN_MITER_VARIABLE |
| 760 | 20 | merged | input-wave3-0760 | ftstroke.FT_STROKER_LINEJOIN_ROUND | ftstroke.FT_Stroker |
| 780 | 20 | merged | input-wave3-0780 | ftsynth.FT_GlyphSlot_AdjustWeight | fttrigon.FT_Vector_Rotate |
| 800 | 20 | merged | input-wave3-0800 | fttrigon.FT_Vector_Unit | fttypes.FT_Byte |
| 820 | 20 | merged | input-wave3-0820 | fttypes.FT_Bytes | fttypes.FT_ULong |
| 840 | 20 | merged | input-wave3-0840 | fttypes.FT_UShort | ftwinfnt.FT_WinFNT_ID_SYMBOL |
| 860 | 20 | merged | input-wave3-0860 | ftwinfnt.FT_Get_WinFNT_Header | t1tables.T1_BLEND_STEM_SNAP_WIDTHS |
| 880 | 20 | merged | input-wave3-0880 | t1tables.T1_BLEND_UNDERLINE_POSITION | ttnameid.TT_MAC_LANGID_AYMARA |
| 900 | 20 | merged | input-wave3-0900 | ttnameid.TT_MAC_LANGID_AZERBAIJANI | ttnameid.TT_MAC_LANGID_ENGLISH |
| 920 | 20 | merged | input-wave3-0920 | ttnameid.TT_MAC_LANGID_ESPERANTO | ttnameid.TT_MAC_LANGID_ICELANDIC |
| 940 | 20 | merged | input-wave3-0940 | ttnameid.TT_MAC_LANGID_INDONESIAN | ttnameid.TT_MAC_LANGID_MALAGASY |
| 960 | 20 | merged | input-wave3-0960 | ttnameid.TT_MAC_LANGID_MALAYALAM | ttnameid.TT_MAC_LANGID_RUANDA |
| 980 | 20 | merged | input-wave3-0980 | ttnameid.TT_MAC_LANGID_RUNDI | ttnameid.TT_MAC_LANGID_THAI |
| 1000 | 20 | merged | input-wave3-1000 | ttnameid.TT_MAC_LANGID_TIBETAN | ttnameid.TT_NAME_ID_FONT_SUBFAMILY |
| 1020 | 20 | merged | input-wave3-1020 | ttnameid.TT_NAME_ID_FULL_NAME | ttnameid.TT_PLATFORM_ADOBE |
| 1040 | 20 | merged | input-wave3-1040 | ttnameid.TT_PLATFORM_APPLE_UNICODE | ttnameid.TT_UCR_BOPOMOFO |
| 1060 | 20 | merged | input-wave3-1060 | ttnameid.TT_UCR_BOX_DRAWING | ttnameid.TT_UCR_COUNTING_ROD_NUMERALS |
| 1080 | 20 | merged | input-wave3-1080 | ttnameid.TT_UCR_CUNEIFORM | ttnameid.TT_UCR_GURMUKHI |
| 1100 | 20 | merged | input-wave3-1100 | ttnameid.TT_UCR_HALFWIDTH_FULLWIDTH_FORMS | ttnameid.TT_UCR_LETTERLIKE_SYMBOLS |
| 1120 | 20 | merged | input-wave3-1120 | ttnameid.TT_UCR_LIMBU | ttnameid.TT_UCR_OL_CHIKI |
| 1140 | 20 | merged | input-wave3-1140 | ttnameid.TT_UCR_ORIYA | ttnameid.TT_UCR_SYLOTI_NAGRI |
| 1160 | 20 | merged | input-wave3-1160 | ttnameid.TT_UCR_SYRIAC | ttnameid.TT_APPLE_ID_FULL_UNICODE |
| 1180 | 20 | merged | input-wave3-1180 | ttnameid.TT_APPLE_ID_ISO_10646 | ttnameid.TT_MAC_ID_JAPANESE |
| 1200 | 20 | merged | input-wave3-1200 | ttnameid.TT_MAC_ID_KANNADA | ttnameid.TT_MAC_ID_TRADITIONAL_CHINESE |
| 1220 | 20 | merged | input-wave3-1220 | ttnameid.TT_MAC_ID_UNINTERP | ttnameid.TT_MS_LANGID_ARABIC_GENERAL |
| 1240 | 20 | merged | input-wave3-1240 | ttnameid.TT_MS_LANGID_ARABIC_IRAQ | ttnameid.TT_MS_LANGID_BASQUE_SPAIN |
| 1260 | 20 | merged | input-wave3-1260 | ttnameid.TT_MS_LANGID_BELARUSIAN_BELARUS | ttnameid.TT_MS_LANGID_CORSICAN_FRANCE |
| 1280 | 20 | merged | input-wave3-1280 | ttnameid.TT_MS_LANGID_CROATIAN_BOSNIA_HERZEGOVINA | ttnameid.TT_MS_LANGID_ENGLISH_IRELAND |
| 1300 | 20 | merged | input-wave3-1300 | ttnameid.TT_MS_LANGID_ENGLISH_JAMAICA | ttnameid.TT_MS_LANGID_FRENCH_COTE_D_IVOIRE |
| 1320 | 20 | merged | input-wave3-1320 | ttnameid.TT_MS_LANGID_FRENCH_FRANCE | ttnameid.TT_MS_LANGID_GERMAN_LIECHTENSTEI |
| 1340 | 20 | merged | input-wave3-1340 | ttnameid.TT_MS_LANGID_GERMAN_LIECHTENSTEIN | ttnameid.TT_MS_LANGID_IRISH_IRELAND |
| 1360 | 20 | merged | input-wave3-1360 | ttnameid.TT_MS_LANGID_ISIXHOSA_SOUTH_AFRICA | ttnameid.TT_MS_LANGID_KOREAN_EXTENDED_WANSUNG_KOREA |
| 1380 | 20 | merged | input-wave3-1380 | ttnameid.TT_MS_LANGID_KOREAN_JOHAB_KOREA | ttnameid.TT_MS_LANGID_MOLDAVIAN_MOLDAVIA |
| 1400 | 20 | merged | input-wave3-1400 | ttnameid.TT_MS_LANGID_MONGOLIAN_MONGOLIA | ttnameid.TT_MS_LANGID_QUECHUA_ECUADOR |
| 1420 | 20 | merged | input-wave3-1420 | ttnameid.TT_MS_LANGID_QUECHUA_PERU | ttnameid.TT_MS_LANGID_SERBIAN_BOSNIA_HERZ_CYRILLIC |
| 1440 | 20 | merged | input-wave3-1440 | ttnameid.TT_MS_LANGID_SERBIAN_BOSNIA_HERZ_LATIN | ttnameid.TT_MS_LANGID_SPANISH_COSTA_RICA |
| 1460 | 20 | merged | input-wave3-1460 | ttnameid.TT_MS_LANGID_SPANISH_DOMINICAN_REPUBLIC | ttnameid.TT_MS_LANGID_SWAHILI_KENYA |
| 1480 | 20 | merged | input-wave3-1480 | ttnameid.TT_MS_LANGID_SWEDISH_FINLAND | ttnameid.TT_MS_LANGID_TSWANA_SOUTH_AFRICA |
| 1500 | 20 | merged | input-wave3-1500 | ttnameid.TT_MS_LANGID_TURKISH_TURKEY | ttnameid.TT_MS_LANGID_YI_PRC |
| 1520 | 20 | merged | input-wave3-1520 | ttnameid.TT_MS_LANGID_YORUBA_NIGERIA | tttables.TT_MaxProfile |
| 1540 | 4 | merged | input-wave3-1540 | tttables.TT_OS2 | tttables.TT_VertHeader |

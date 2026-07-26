//! FreeType-shaped compatibility facade implemented on top of the safe API.
//!
//! This module intentionally keeps C-style names, integer constants, and record
//! field spelling because it is the parity target for callers migrating from
//! FreeType. It does not load or link native FreeType; every operation delegates
//! into this crate's pure-Rust [`crate::api`] layer.

mod constants;
mod convert;
mod handles;
mod types;

pub use constants::*;
pub use convert::{
    FT_LOAD_TARGET_MODE, glyph_format_from_core, load_flags_to_core, pixel_mode_from_core,
    render_mode_to_core,
};
pub use handles::{
    FT_Activate_Size, FT_Add_Default_Modules, FT_Add_Module, FT_Angle_Diff, FT_Atan2,
    FT_Attach_Stream, FT_Bitmap_Blend, FT_Bitmap_Convert, FT_Bitmap_Copy, FT_Bitmap_Done,
    FT_Bitmap_Embolden, FT_Bitmap_Glyph_Copy, FT_Bitmap_Init, FT_Bitmap_New,
    FT_Bitmap_Owned_Buffer_Bytes, FT_Bitmap_Set_Owned_Buffer, FT_CeilFix, FT_ClassicKern_Free,
    FT_ClassicKern_Validate, FT_Cos, FT_DivFix, FT_Done_Face, FT_Done_FreeType, FT_Done_Glyph,
    FT_Done_MM_Var, FT_Done_Size, FT_Error_String, FT_FACE_DRIVER_NAME, FT_Face,
    FT_Face_CheckTrueTypePatents, FT_Face_GetCharVariantIndex, FT_Face_GetCharVariantIsDefault,
    FT_Face_GetCharsOfVariant, FT_Face_GetVariantSelectors, FT_Face_GetVariantsOfChar,
    FT_Face_Properties, FT_Face_Properties_Get_State, FT_Face_Properties_State, FT_Face_Property,
    FT_Face_Property_Value, FT_Face_SetUnpatentedHinting, FT_FloorFix, FT_Get_Advance,
    FT_Get_Advances, FT_Get_BDF_Charset_ID, FT_Get_BDF_Property, FT_Get_Bitmap_Glyph,
    FT_Get_CID_From_Glyph_Index, FT_Get_CID_Is_Internally_CID_Keyed,
    FT_Get_CID_Registry_Ordering_Supplement, FT_Get_CMap_Format, FT_Get_CMap_Language_ID,
    FT_Get_Char_Index, FT_Get_Charmap_Index, FT_Get_Color_Glyph_ClipBox, FT_Get_Color_Glyph_Layer,
    FT_Get_Color_Glyph_Paint, FT_Get_Colorline_Stops, FT_Get_Default_Named_Instance,
    FT_Get_FSType_Flags, FT_Get_First_Char, FT_Get_Font_Format, FT_Get_Gasp, FT_Get_Glyph,
    FT_Get_Glyph_Name, FT_Get_Kerning, FT_Get_MM_Blend_Coordinates, FT_Get_MM_Var,
    FT_Get_MM_WeightVector, FT_Get_Module_Interface, FT_Get_Multi_Master, FT_Get_Name_Index,
    FT_Get_Next_Char, FT_Get_Outline_Glyph, FT_Get_PFR_Kerning, FT_Get_PS_Font_Info,
    FT_Get_PS_Font_Private, FT_Get_PS_Font_Value, FT_Get_Paint, FT_Get_Paint_Layers,
    FT_Get_Postscript_Name, FT_Get_Sfnt_LangTag, FT_Get_Sfnt_Name, FT_Get_Sfnt_Name_Count,
    FT_Get_Sfnt_Table, FT_Get_SubGlyph_Info, FT_Get_Track_Kerning, FT_Get_Transform,
    FT_Get_TrueType_Engine_Type, FT_Get_Var_Axis_Flags, FT_Get_Var_Blend_Coordinates,
    FT_Get_Var_Design_Coordinates, FT_Get_WinFNT_Header, FT_Get_X11_Font_Format, FT_Glyph_Copy,
    FT_Glyph_Get_CBox, FT_Glyph_To_Bitmap, FT_Glyph_Transform_Outline, FT_GlyphSlot,
    FT_GlyphSlot_AdjustWeight, FT_GlyphSlot_Embolden, FT_GlyphSlot_Oblique,
    FT_GlyphSlot_Own_Bitmap, FT_GlyphSlot_Slant, FT_Gzip_Stream_Close, FT_Gzip_Stream_Read,
    FT_Gzip_Uncompress, FT_Has_PS_Glyph_Names, FT_Init_FreeType, FT_Installed_Module_Info,
    FT_Library, FT_Library_SetLcdFilter, FT_Library_SetLcdFilterWeights, FT_Library_SetLcdGeometry,
    FT_Library_Version, FT_List_Add, FT_List_Finalize_Clear, FT_List_Finalize_Node,
    FT_List_Find_Node_Matches, FT_List_Insert, FT_List_Iterate_Next, FT_List_Remove, FT_List_Up,
    FT_Load_Char, FT_Load_Glyph, FT_Load_Sfnt_Table, FT_Matrix_Invert, FT_Matrix_Multiply,
    FT_Module_Callback_Behavior, FT_Module_Class_Info, FT_MulDiv, FT_MulFix, FT_New_Face,
    FT_New_Memory_Face, FT_New_Memory_Face_With_Name_Options, FT_New_Size,
    FT_Open_External_Stream_Face_With_Name_Options, FT_Open_Face_Name_Options, FT_OpenType_Free,
    FT_OpenType_Validate, FT_Outline_Check, FT_Outline_Copy, FT_Outline_Decompose_Trace,
    FT_Outline_Embolden, FT_Outline_EmboldenXY, FT_Outline_Get_BBox, FT_Outline_Get_Bitmap,
    FT_Outline_Get_CBox, FT_Outline_Get_Orientation, FT_Outline_GetInsideBorder,
    FT_Outline_GetOutsideBorder, FT_Outline_Glyph_CBox, FT_Outline_Glyph_Copy,
    FT_Outline_Glyph_Stroke, FT_Outline_Glyph_To_Bitmap, FT_Outline_Render,
    FT_Outline_Render_Direct_Spans, FT_Outline_Render_Error_Output, FT_Outline_Reverse,
    FT_Outline_Transform, FT_Outline_Translate, FT_Palette_Data_Get, FT_Palette_Select,
    FT_Palette_Set_Foreground_Color, FT_Property_Get, FT_Property_Get_GlyphToScriptMap,
    FT_Property_Get_IncreaseXHeight, FT_Property_Set, FT_Property_Set_IncreaseXHeight,
    FT_Reference_Face, FT_Reference_Library, FT_Render_Glyph, FT_Request_Size, FT_RoundFix,
    FT_Select_Charmap, FT_Select_Size, FT_Set_Char_Size, FT_Set_Charmap, FT_Set_Debug_Hook,
    FT_Set_Default_Properties, FT_Set_Default_Properties_From_Env, FT_Set_MM_Blend_Coordinates,
    FT_Set_MM_Design_Coordinates, FT_Set_MM_WeightVector, FT_Set_Named_Instance,
    FT_Set_Pixel_Sizes, FT_Set_Transform, FT_Set_Var_Blend_Coordinates,
    FT_Set_Var_Design_Coordinates, FT_Sfnt_Table_Info, FT_Sin, FT_Stream_OpenBzip2,
    FT_Stream_OpenGzip, FT_Stroker, FT_Stroker_BeginSubPath, FT_Stroker_ConicTo,
    FT_Stroker_CubicTo, FT_Stroker_Done, FT_Stroker_EndSubPath, FT_Stroker_Export,
    FT_Stroker_ExportBorder, FT_Stroker_GetBorderCounts, FT_Stroker_GetCounts, FT_Stroker_LineTo,
    FT_Stroker_New, FT_Stroker_ParseOutline, FT_Stroker_Rewind, FT_Stroker_Set, FT_Tan,
    FT_TrueTypeGX_Free, FT_Vector_From_Polar, FT_Vector_Length, FT_Vector_Polarize,
    FT_Vector_Rotate, FT_Vector_Transform, FT_Vector_Unit, FTC_Node_Unref, FTOutlineDecomposeEvent,
    FTOutlineDecomposeRun,
};

#[cfg(any(test, feature = "abi-test-support"))]
pub use handles::FT_ColrV1_Paint_Layer_Iterator_Copy;
#[cfg(any(test, feature = "abi-test-support"))]
pub use handles::FT_Empty_GlyphSlot;
#[cfg(feature = "abi-test-support")]
pub use handles::FT_Outline_GlyphSlot_With_Advance;
#[cfg(any(test, feature = "abi-test-support"))]
pub use handles::{
    FT_ColrV1_Paint_ColorLine_Copy, FT_ColrV1_Paint_Transform_Copy, FT_ColrV1_PaintGraph_Copy,
    FT_ColrV1_PaintGraph_Snapshot, FT_ColrV1_PaintNode_Snapshot, FT_ColrV1_PaintRecord_Snapshot,
    FT_ColrV1_PublicPaintSolid_Copy, FT_ColrV1_PublicPaintSolid_Snapshot,
    FT_Fvar_Named_Style_Coords, FT_Get_Sfnt_MaxProfile_Copy, FT_Get_Sfnt_VertHeader_Copy,
    FT_Glyph_To_Script_Map_Sample_For_Test, FT_GlyphSlot_Own_Bitmap_Copy_Allocation_Failure,
    FT_Library_Debug_Hook_Classes, FT_Library_Default_Module_Names, FT_Library_Has_Module,
    FT_Library_Has_TrueType_Engine_Service, FT_Library_Has_TrueType_Module,
    FT_Library_Module_Count, FT_Library_Module_Flags, FT_Library_Renderer_Class,
    FT_Library_Set_Renderer_By_Format, FT_Library_Synthetic_Module_Info,
    FT_Module_Requester_Service_Available, FT_New_Library_Without_Default_Modules,
    FT_Palette_Active_Entries_Copy, FT_Palette_Data_Copy, FT_Palette_Data_Snapshot,
    FT_Palette_Select_Copy, FT_Palette_Select_Snapshot, FT_Palette_Set_Active_Entry_For_Test,
    FT_Unsupported_GlyphSlot,
};
pub use handles::{FT_Done_Library, FT_Library_Memory, FT_Library_Refcount, FT_New_Library};
pub use types::*;

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
    FT_Activate_Size, FT_Angle_Diff, FT_Atan2, FT_CeilFix, FT_Charmap_Format, FT_Charmap_Info,
    FT_Charmap_Language_ID, FT_Cos, FT_DivFix, FT_Done_Face, FT_Done_FreeType, FT_Done_Size,
    FT_Face, FT_Face_Active_Charmap_Index, FT_Face_Charmap, FT_Face_Charmap_Count,
    FT_Face_Charmap_Info, FT_Face_CheckTrueTypePatents, FT_Face_Info, FT_Face_SetUnpatentedHinting,
    FT_FloorFix, FT_Get_Advance, FT_Get_Advances, FT_Get_CMap_Format, FT_Get_CMap_Language_ID,
    FT_Get_Char_Index, FT_Get_Charmap_Index, FT_Get_Charmap_Index_For_Face, FT_Get_FSType_Flags,
    FT_Get_First_Char, FT_Get_Gasp, FT_Get_Glyph_Name, FT_Get_Kerning, FT_Get_Name_Index,
    FT_Get_Next_Char, FT_Get_Postscript_Name, FT_Get_Sfnt_LangTag, FT_Get_Sfnt_Name,
    FT_Get_Sfnt_Name_Count, FT_Get_Sfnt_OS2, FT_Get_Sfnt_Table, FT_Get_SubGlyph_Info,
    FT_Get_Transform, FT_Get_TrueType_Engine_Type, FT_GlyphSlot, FT_GlyphSlot_AdjustWeight,
    FT_GlyphSlot_Embolden, FT_GlyphSlot_Oblique, FT_GlyphSlot_Slant, FT_Init_FreeType, FT_Library,
    FT_Library_SetLcdFilter, FT_Library_SetLcdFilterWeights, FT_Library_SetLcdGeometry,
    FT_Library_Version, FT_Load_Char, FT_Load_Glyph, FT_Load_Sfnt_Table, FT_Matrix_Invert,
    FT_Matrix_Multiply, FT_MulDiv, FT_MulFix, FT_New_Face, FT_New_Memory_Face, FT_New_Size,
    FT_OpenType_Free, FT_OpenType_Validate, FT_Outline_Get_CBox, FT_Reference_Face,
    FT_Render_Glyph, FT_Request_Size, FT_RoundFix, FT_Select_Charmap, FT_Select_Size,
    FT_Set_Char_Size, FT_Set_Charmap, FT_Set_Named_Instance, FT_Set_Pixel_Sizes, FT_Set_Transform,
    FT_Sfnt_Table_Count, FT_Sfnt_Table_Info, FT_Sin, FT_Size_Metrics, FT_Tan, FT_Vector_From_Polar,
    FT_Vector_Length, FT_Vector_Polarize, FT_Vector_Rotate, FT_Vector_Transform, FT_Vector_Unit,
};
pub use types::*;

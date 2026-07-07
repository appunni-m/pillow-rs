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
    FT_Face, FT_Get_Advance, FT_Get_Advances, FT_Get_CMap_Format, FT_Get_CMap_Language_ID,
    FT_Get_Char_Index, FT_Get_Sfnt_Table, FT_GlyphSlot, FT_Init_FreeType, FT_Library, FT_Load_Char,
    FT_Load_Glyph, FT_Load_Sfnt_Table, FT_New_Memory_Face, FT_Render_Glyph, FT_Set_Char_Size,
    FT_Set_Pixel_Sizes, FT_Sfnt_Table_Info, FT_Size_Metrics,
};
pub use types::*;

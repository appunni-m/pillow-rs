#![allow(non_upper_case_globals)]

use super::types::{FT_Error, FT_Glyph_Format, FT_Int32, FT_Pixel_Mode, FT_Render_Mode};

pub const FT_Err_Ok: FT_Error = 0x00;
pub const FT_Err_Cannot_Open_Resource: FT_Error = 0x01;
pub const FT_Err_Unknown_File_Format: FT_Error = 0x02;
pub const FT_Err_Invalid_File_Format: FT_Error = 0x03;
pub const FT_Err_Invalid_Argument: FT_Error = 0x06;
pub const FT_Err_Unimplemented_Feature: FT_Error = 0x07;
pub const FT_Err_Invalid_Table: FT_Error = 0x08;
pub const FT_Err_Invalid_Glyph_Index: FT_Error = 0x10;
pub const FT_Err_Invalid_Character_Code: FT_Error = 0x11;
pub const FT_Err_Invalid_Glyph_Format: FT_Error = 0x12;
pub const FT_Err_Cannot_Render_Glyph: FT_Error = 0x13;
pub const FT_Err_Invalid_Outline: FT_Error = 0x14;
pub const FT_Err_Invalid_Pixel_Size: FT_Error = 0x17;
pub const FT_Err_Invalid_Size_Handle: FT_Error = 0x24;
pub const FT_Err_Invalid_CharMap_Handle: FT_Error = 0x26;
pub const FT_Err_Out_Of_Memory: FT_Error = 0x40;
pub const FT_Err_Raster_Overflow: FT_Error = 0x62;
pub const FT_Err_Invalid_CharMap_Format: FT_Error = 0x96;
pub const FT_Err_Max: FT_Error = 0xBB;
pub const FT_CONFIG_OPTION_ERROR_STRINGS_ENABLED: bool = false;

pub const BDF_PROPERTY_TYPE_NONE: i32 = 0;
pub const BDF_PROPERTY_TYPE_ATOM: i32 = 1;
pub const BDF_PROPERTY_TYPE_INTEGER: i32 = 2;
pub const BDF_PROPERTY_TYPE_CARDINAL: i32 = 3;

pub const PS_DICT_ENCODING_TYPE: i32 = 9;
pub const PS_DICT_ENCODING_ENTRY: i32 = 10;

pub const FT_LOAD_DEFAULT: FT_Int32 = 0;
pub const FT_LOAD_NO_SCALE: FT_Int32 = 1 << 0;
pub const FT_LOAD_NO_HINTING: FT_Int32 = 1 << 1;
pub const FT_LOAD_RENDER: FT_Int32 = 1 << 2;
pub const FT_LOAD_NO_BITMAP: FT_Int32 = 1 << 3;
pub const FT_LOAD_VERTICAL_LAYOUT: FT_Int32 = 1 << 4;
pub const FT_LOAD_FORCE_AUTOHINT: FT_Int32 = 1 << 5;
pub const FT_LOAD_CROP_BITMAP: FT_Int32 = 1 << 6;
pub const FT_LOAD_PEDANTIC: FT_Int32 = 1 << 7;
pub const FT_LOAD_ADVANCE_ONLY: FT_Int32 = 1 << 8;
pub const FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH: FT_Int32 = 1 << 9;
pub const FT_LOAD_NO_RECURSE: FT_Int32 = 1 << 10;
pub const FT_LOAD_IGNORE_TRANSFORM: FT_Int32 = 1 << 11;
pub const FT_LOAD_MONOCHROME: FT_Int32 = 1 << 12;
pub const FT_LOAD_LINEAR_DESIGN: FT_Int32 = 1 << 13;
pub const FT_LOAD_SBITS_ONLY: FT_Int32 = 1 << 14;
pub const FT_LOAD_NO_AUTOHINT: FT_Int32 = 1 << 15;
pub const FT_LOAD_COLOR: FT_Int32 = 1 << 20;
pub const FT_LOAD_COMPUTE_METRICS: FT_Int32 = 1 << 21;
pub const FT_LOAD_BITMAP_METRICS_ONLY: FT_Int32 = 1 << 22;
pub const FT_LOAD_SVG_ONLY: FT_Int32 = 1 << 23;
pub const FT_LOAD_NO_SVG: FT_Int32 = 1 << 24;

pub const FT_RENDER_MODE_NORMAL: FT_Render_Mode = 0;
pub const FT_RENDER_MODE_LIGHT: FT_Render_Mode = 1;
pub const FT_RENDER_MODE_MONO: FT_Render_Mode = 2;
pub const FT_RENDER_MODE_LCD: FT_Render_Mode = 3;
pub const FT_RENDER_MODE_LCD_V: FT_Render_Mode = 4;
pub const FT_RENDER_MODE_SDF: FT_Render_Mode = 5;
pub const FT_RENDER_MODE_MAX: FT_Render_Mode = 6;

pub const FT_LOAD_TARGET_NORMAL: FT_Int32 = FT_RENDER_MODE_NORMAL << 16;
pub const FT_LOAD_TARGET_LIGHT: FT_Int32 = FT_RENDER_MODE_LIGHT << 16;
pub const FT_LOAD_TARGET_MONO: FT_Int32 = FT_RENDER_MODE_MONO << 16;
pub const FT_LOAD_TARGET_LCD: FT_Int32 = FT_RENDER_MODE_LCD << 16;
pub const FT_LOAD_TARGET_LCD_V: FT_Int32 = FT_RENDER_MODE_LCD_V << 16;

pub const FT_PIXEL_MODE_NONE: FT_Pixel_Mode = 0;
pub const FT_PIXEL_MODE_MONO: FT_Pixel_Mode = 1;
pub const FT_PIXEL_MODE_GRAY: FT_Pixel_Mode = 2;
pub const FT_PIXEL_MODE_GRAY2: FT_Pixel_Mode = 3;
pub const FT_PIXEL_MODE_GRAY4: FT_Pixel_Mode = 4;
pub const FT_PIXEL_MODE_LCD: FT_Pixel_Mode = 5;
pub const FT_PIXEL_MODE_LCD_V: FT_Pixel_Mode = 6;
pub const FT_PIXEL_MODE_BGRA: FT_Pixel_Mode = 7;
pub const FT_PIXEL_MODE_MAX: FT_Pixel_Mode = 8;

pub const FT_GLYPH_FORMAT_NONE: FT_Glyph_Format = 0x0000_0000;
pub const FT_GLYPH_FORMAT_COMPOSITE: FT_Glyph_Format = 0x636f_6d70;
pub const FT_GLYPH_FORMAT_BITMAP: FT_Glyph_Format = 0x6269_7473;
pub const FT_GLYPH_FORMAT_OUTLINE: FT_Glyph_Format = 0x6f75_746c;
pub const FT_GLYPH_FORMAT_PLOTTER: FT_Glyph_Format = 0x706c_6f74;
pub const FT_GLYPH_FORMAT_SVG: FT_Glyph_Format = 0x5356_4720;

pub(super) const LOAD_TARGET_MASK: FT_Int32 = 15 << 16;
// These accepted public load flags either have no currently observable core
// behavior or are handled by the wrapper boundary that owns the corresponding
// public surface.
pub(super) const LOAD_FLAGS_ACCEPTED_WITHOUT_CORE_BITS: FT_Int32 = FT_LOAD_CROP_BITMAP
    | FT_LOAD_ADVANCE_ONLY
    | FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH
    | FT_LOAD_IGNORE_TRANSFORM
    | FT_LOAD_LINEAR_DESIGN;

pub(super) const SUPPORTED_LOAD_FLAGS: FT_Int32 = FT_LOAD_RENDER
    | FT_LOAD_NO_SCALE
    | FT_LOAD_NO_HINTING
    | FT_LOAD_NO_RECURSE
    | FT_LOAD_VERTICAL_LAYOUT
    | FT_LOAD_FORCE_AUTOHINT
    | FT_LOAD_PEDANTIC
    | FT_LOAD_MONOCHROME
    | FT_LOAD_SBITS_ONLY
    | FT_LOAD_NO_BITMAP
    | FT_LOAD_NO_AUTOHINT
    | FT_LOAD_COLOR
    | FT_LOAD_COMPUTE_METRICS
    | FT_LOAD_BITMAP_METRICS_ONLY
    | FT_LOAD_NO_SVG
    | LOAD_FLAGS_ACCEPTED_WITHOUT_CORE_BITS
    | LOAD_TARGET_MASK;

include!("generated_constants.rs");

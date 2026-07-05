//! FreeType-shaped compatibility facade implemented on top of the safe API.
//!
//! This module intentionally keeps C-style names, integer constants, and record
//! field spelling because it is the parity target for callers migrating from
//! FreeType. It does not load or link native FreeType; every operation delegates
//! into this crate's pure-Rust [`crate::api`] layer.

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::os::raw::{c_int, c_long, c_uint, c_ulong, c_ushort};

use crate::api;
use crate::error::FontError;
use crate::font::{BBox, GlyphSlotMetrics, SizeMetrics};
use crate::render::{PixelMode, RenderMode, RenderedBitmap};

pub type FT_Error = c_int;
pub type FT_Int = c_int;
pub type FT_UInt = c_uint;
pub type FT_Int32 = i32;
pub type FT_Long = c_long;
pub type FT_ULong = c_ulong;
pub type FT_Pos = c_long;
pub type FT_Fixed = c_long;
pub type FT_F26Dot6 = c_long;
pub type FT_UShort = c_ushort;
pub type FT_Render_Mode = c_int;
pub type FT_Pixel_Mode = c_int;
pub type FT_Glyph_Format = c_int;

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
pub const FT_Err_Invalid_CharMap_Handle: FT_Error = 0x26;
pub const FT_Err_Out_Of_Memory: FT_Error = 0x40;
pub const FT_Err_Raster_Overflow: FT_Error = 0x62;
pub const FT_Err_Invalid_CharMap_Format: FT_Error = 0x96;

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

const LOAD_TARGET_MASK: FT_Int32 = 15 << 16;
const SUPPORTED_LOAD_FLAGS: FT_Int32 = FT_LOAD_RENDER
    | FT_LOAD_NO_HINTING
    | FT_LOAD_FORCE_AUTOHINT
    | FT_LOAD_MONOCHROME
    | LOAD_TARGET_MASK;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Vector {
    pub x: FT_Pos,
    pub y: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_BBox {
    pub xMin: FT_Pos,
    pub yMin: FT_Pos,
    pub xMax: FT_Pos,
    pub yMax: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Glyph_Metrics {
    pub width: FT_Pos,
    pub height: FT_Pos,
    pub horiBearingX: FT_Pos,
    pub horiBearingY: FT_Pos,
    pub horiAdvance: FT_Pos,
    pub vertBearingX: FT_Pos,
    pub vertBearingY: FT_Pos,
    pub vertAdvance: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Size_Metrics {
    pub x_ppem: FT_UShort,
    pub y_ppem: FT_UShort,
    pub x_scale: FT_Fixed,
    pub y_scale: FT_Fixed,
    pub ascender: FT_Pos,
    pub descender: FT_Pos,
    pub height: FT_Pos,
    pub max_advance: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FT_Bitmap {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub buffer: Vec<u8>,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FT_Library {
    inner: api::Library,
}

#[derive(Clone)]
pub struct FT_Face {
    inner: api::Face,
}

#[derive(Clone)]
pub struct FT_GlyphSlot {
    pub glyph_index: FT_UInt,
    pub metrics: FT_Glyph_Metrics,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub bitmap: Option<FT_Bitmap>,
    pub bitmap_left: FT_Int,
    pub bitmap_top: FT_Int,
    source_face: api::Face,
}

impl From<api::Vector> for FT_Vector {
    fn from(value: api::Vector) -> Self {
        Self {
            x: FT_Pos::from(value.x),
            y: FT_Pos::from(value.y),
        }
    }
}

impl From<BBox> for FT_BBox {
    fn from(value: BBox) -> Self {
        Self {
            xMin: FT_Pos::from(value.x_min),
            yMin: FT_Pos::from(value.y_min),
            xMax: FT_Pos::from(value.x_max),
            yMax: FT_Pos::from(value.y_max),
        }
    }
}

impl From<GlyphSlotMetrics> for FT_Glyph_Metrics {
    fn from(value: GlyphSlotMetrics) -> Self {
        Self {
            width: FT_Pos::from(value.width),
            height: FT_Pos::from(value.height),
            horiBearingX: FT_Pos::from(value.hori_bearing_x),
            horiBearingY: FT_Pos::from(value.hori_bearing_y),
            horiAdvance: FT_Pos::from(value.hori_advance),
            vertBearingX: FT_Pos::from(value.vert_bearing_x),
            vertBearingY: FT_Pos::from(value.vert_bearing_y),
            vertAdvance: FT_Pos::from(value.vert_advance),
        }
    }
}

impl From<SizeMetrics> for FT_Size_Metrics {
    fn from(value: SizeMetrics) -> Self {
        Self {
            x_ppem: value.x_ppem,
            y_ppem: value.y_ppem,
            x_scale: FT_Fixed::from(value.x_scale),
            y_scale: FT_Fixed::from(value.y_scale),
            ascender: FT_Pos::from(value.ascender),
            descender: FT_Pos::from(value.descender),
            height: FT_Pos::from(value.height),
            max_advance: FT_Pos::from(value.max_advance),
        }
    }
}

impl From<RenderedBitmap> for FT_Bitmap {
    fn from(value: RenderedBitmap) -> Self {
        Self {
            rows: value.rows,
            width: value.width,
            pitch: value.pitch,
            buffer: value.buffer,
            num_grays: value.num_grays,
            pixel_mode: pixel_mode_from_core(value.pixel_mode),
        }
    }
}

pub fn FT_LOAD_TARGET_MODE(flags: FT_Int32) -> FT_Render_Mode {
    (flags >> 16) & 15
}

pub fn FT_Init_FreeType() -> FT_Library {
    FT_Library {
        inner: api::Library::init(),
    }
}

pub fn FT_New_Memory_Face(
    library: &FT_Library,
    data: &[u8],
    face_index: FT_Long,
    size_pt: f32,
) -> Result<FT_Face, FT_Error> {
    let face_index = usize::try_from(face_index).map_err(|_| FT_Err_Invalid_Argument)?;
    library
        .inner
        .new_memory_face(data, face_index, size_pt)
        .map(|inner| FT_Face { inner })
        .map_err(error_to_ft)
}

pub fn FT_Set_Char_Size(
    face: &mut FT_Face,
    char_width: FT_F26Dot6,
    char_height: FT_F26Dot6,
    horz_resolution: FT_UInt,
    vert_resolution: FT_UInt,
) -> FT_Error {
    let Ok(char_width) = i32::try_from(char_width) else {
        return FT_Err_Invalid_Argument;
    };
    let Ok(char_height) = i32::try_from(char_height) else {
        return FT_Err_Invalid_Argument;
    };
    face.inner
        .set_char_size(char_width, char_height, horz_resolution, vert_resolution);
    FT_Err_Ok
}

pub fn FT_Set_Pixel_Sizes(
    face: &mut FT_Face,
    pixel_width: FT_UInt,
    pixel_height: FT_UInt,
) -> FT_Error {
    face.inner.set_pixel_sizes(pixel_width, pixel_height);
    FT_Err_Ok
}

pub fn FT_Get_Char_Index(face: &FT_Face, char_code: FT_ULong) -> FT_UInt {
    let Ok(char_code) = u32::try_from(char_code) else {
        return 0;
    };
    u32::from(face.inner.get_char_index(char_code))
}

pub fn FT_Load_Char(
    face: &FT_Face,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> Result<FT_GlyphSlot, FT_Error> {
    FT_Load_Glyph(face, FT_Get_Char_Index(face, char_code), load_flags)
}

pub fn FT_Load_Glyph(
    face: &FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> Result<FT_GlyphSlot, FT_Error> {
    let glyph_index = u16::try_from(glyph_index).map_err(|_| FT_Err_Invalid_Glyph_Index)?;
    let flags = load_flags_to_core(load_flags)?;
    face.inner
        .load_glyph(glyph_index, flags)
        .map(|slot| slot_to_ffi(face, slot))
        .map_err(error_to_ft)
}

pub fn FT_Render_Glyph(
    slot: FT_GlyphSlot,
    render_mode: FT_Render_Mode,
) -> Result<FT_GlyphSlot, FT_Error> {
    let mode = render_mode_to_core(render_mode).ok_or(FT_Err_Cannot_Render_Glyph)?;
    let glyph_index = u16::try_from(slot.glyph_index).map_err(|_| FT_Err_Invalid_Glyph_Index)?;
    let mut flags = api::LoadFlags::RENDER;
    flags |= load_flag_for_render_mode(mode);
    slot.source_face
        .load_glyph(glyph_index, flags)
        .map(|rendered| {
            slot_to_ffi(
                &FT_Face {
                    inner: slot.source_face,
                },
                rendered,
            )
        })
        .map_err(error_to_ft)
}

pub fn FT_Size_Metrics(face: &FT_Face) -> FT_Size_Metrics {
    face.inner.size_metrics().into()
}

pub fn load_flags_to_core(flags: FT_Int32) -> Result<api::LoadFlags, FT_Error> {
    if flags & !SUPPORTED_LOAD_FLAGS != 0 {
        return Err(FT_Err_Unimplemented_Feature);
    }

    let mut core = api::LoadFlags::DEFAULT;
    if flags & FT_LOAD_RENDER != 0 {
        core |= api::LoadFlags::RENDER;
    }
    if flags & FT_LOAD_NO_HINTING != 0 {
        core |= api::LoadFlags::NO_HINTING;
    }
    if flags & FT_LOAD_FORCE_AUTOHINT != 0 {
        core |= api::LoadFlags::FORCE_AUTOHINT;
    }
    if flags & FT_LOAD_MONOCHROME != 0 {
        core |= api::LoadFlags::TARGET_MONO;
    }
    core |= match FT_LOAD_TARGET_MODE(flags) {
        FT_RENDER_MODE_NORMAL | FT_RENDER_MODE_LIGHT => api::LoadFlags::DEFAULT,
        FT_RENDER_MODE_MONO => api::LoadFlags::TARGET_MONO,
        FT_RENDER_MODE_LCD => api::LoadFlags::TARGET_LCD,
        FT_RENDER_MODE_LCD_V => api::LoadFlags::TARGET_LCD_V,
        _ => return Err(FT_Err_Unimplemented_Feature),
    };
    Ok(core)
}

pub fn render_mode_to_core(mode: FT_Render_Mode) -> Option<RenderMode> {
    match mode {
        FT_RENDER_MODE_NORMAL | FT_RENDER_MODE_LIGHT => Some(RenderMode::Normal),
        FT_RENDER_MODE_MONO => Some(RenderMode::Mono),
        FT_RENDER_MODE_LCD => Some(RenderMode::Lcd),
        FT_RENDER_MODE_LCD_V => Some(RenderMode::LcdV),
        FT_RENDER_MODE_SDF | FT_RENDER_MODE_MAX => None,
        _ => None,
    }
}

pub fn pixel_mode_from_core(mode: PixelMode) -> FT_Pixel_Mode {
    match mode {
        PixelMode::Gray => FT_PIXEL_MODE_GRAY,
        PixelMode::Mono => FT_PIXEL_MODE_MONO,
        PixelMode::Lcd => FT_PIXEL_MODE_LCD,
        PixelMode::LcdV => FT_PIXEL_MODE_LCD_V,
    }
}

pub fn glyph_format_from_core(format: api::GlyphFormat) -> FT_Glyph_Format {
    match format {
        api::GlyphFormat::None => FT_GLYPH_FORMAT_NONE,
        api::GlyphFormat::Outline => FT_GLYPH_FORMAT_OUTLINE,
        api::GlyphFormat::Bitmap => FT_GLYPH_FORMAT_BITMAP,
    }
}

fn slot_to_ffi(face: &FT_Face, slot: api::GlyphSlot) -> FT_GlyphSlot {
    FT_GlyphSlot {
        glyph_index: FT_UInt::from(slot.glyph_index),
        metrics: slot.metrics.into(),
        advance: slot.advance.into(),
        format: glyph_format_from_core(slot.format),
        bitmap: slot.bitmap.map(Into::into),
        bitmap_left: slot.bitmap_left,
        bitmap_top: slot.bitmap_top,
        source_face: face.inner.clone(),
    }
}

fn load_flag_for_render_mode(mode: RenderMode) -> api::LoadFlags {
    match mode {
        RenderMode::Normal => api::LoadFlags::DEFAULT,
        RenderMode::Mono => api::LoadFlags::TARGET_MONO,
        RenderMode::Lcd => api::LoadFlags::TARGET_LCD,
        RenderMode::LcdV => api::LoadFlags::TARGET_LCD_V,
    }
}

fn error_to_ft(error: FontError) -> FT_Error {
    match error {
        FontError::InvalidFont(_) => FT_Err_Invalid_File_Format,
        FontError::UnsupportedCmapFormat(_) => FT_Err_Invalid_CharMap_Format,
        FontError::RasterOverflow => FT_Err_Raster_Overflow,
        FontError::InvalidOutline(_) => FT_Err_Invalid_Outline,
        FontError::UnsupportedLoadFlags(_) => FT_Err_Unimplemented_Feature,
    }
}

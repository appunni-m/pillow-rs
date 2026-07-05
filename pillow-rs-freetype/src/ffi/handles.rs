#![allow(non_camel_case_types, non_snake_case)]

use crate::api;

use super::constants::*;
use super::convert::{
    error_to_ft, glyph_format_from_core, load_flag_for_render_mode, load_flags_to_core,
    render_mode_to_core,
};
use super::types::{
    FT_Bitmap, FT_Error, FT_F26Dot6, FT_Glyph_Format, FT_Glyph_Metrics, FT_Int, FT_Int32, FT_Long,
    FT_Render_Mode, FT_Size_Metrics as FT_Size_MetricsRec, FT_UInt, FT_ULong, FT_Vector,
};

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

pub fn FT_Size_Metrics(face: &FT_Face) -> FT_Size_MetricsRec {
    face.inner.size_metrics().into()
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

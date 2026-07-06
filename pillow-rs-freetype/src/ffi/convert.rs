#![allow(non_snake_case)]

use crate::api;
use crate::error::FontError;
use crate::font::{BBox, GlyphSlotMetrics, SizeMetrics};
use crate::render::{PixelMode, RenderMode, RenderedBitmap};

use super::constants::*;
use super::types::{
    FT_BBox, FT_Bitmap, FT_Error, FT_Fixed, FT_Glyph_Format, FT_Glyph_Metrics, FT_Int32,
    FT_Pixel_Mode, FT_Pos, FT_Render_Mode, FT_Size_Metrics, FT_Vector,
};

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

pub fn load_flags_to_core(flags: FT_Int32) -> Result<api::LoadFlags, FT_Error> {
    if flags & !SUPPORTED_LOAD_FLAGS != 0 {
        return Err(FT_Err_Unimplemented_Feature);
    }

    let mut core = api::LoadFlags::DEFAULT;
    // FreeType resolves this dependency in `FT_Load_Glyph` before driver load:
    // `FT_LOAD_NO_SCALE` implies no hinting/no bitmap and clears
    // `FT_LOAD_RENDER` (`src/base/ftobjs.c`).
    if flags & FT_LOAD_NO_SCALE != 0 {
        core |= api::LoadFlags::NO_SCALE;
    }
    // FreeType resolves this dependency in `FT_Load_Glyph` before driver load:
    // `FT_LOAD_BITMAP_METRICS_ONLY` clears `FT_LOAD_RENDER`.
    if flags & FT_LOAD_RENDER != 0
        && flags & FT_LOAD_NO_SCALE == 0
        && flags & FT_LOAD_BITMAP_METRICS_ONLY == 0
    {
        core |= api::LoadFlags::RENDER;
    }
    if flags & FT_LOAD_NO_HINTING != 0 || flags & FT_LOAD_NO_SCALE != 0 {
        core |= api::LoadFlags::NO_HINTING;
    }
    if flags & FT_LOAD_FORCE_AUTOHINT != 0 {
        core |= api::LoadFlags::FORCE_AUTOHINT;
    }
    if flags & FT_LOAD_NO_AUTOHINT != 0 {
        core |= api::LoadFlags::NO_AUTOHINT;
    }
    if flags & FT_LOAD_VERTICAL_LAYOUT != 0 {
        core |= api::LoadFlags::VERTICAL_LAYOUT;
    }
    if flags & FT_LOAD_MONOCHROME != 0 {
        core |= api::LoadFlags::MONOCHROME_RENDER;
    }
    core |= match FT_LOAD_TARGET_MODE(flags) {
        FT_RENDER_MODE_NORMAL => api::LoadFlags::DEFAULT,
        // C `FT_Load_Glyph` routes LIGHT target loads through the auto-hinter
        // with `FT_RENDER_MODE_LIGHT` style flags (`src/base/ftobjs.c` and
        // `src/autofit/aflatin.c`).  It is not identical to
        // `FT_LOAD_FORCE_AUTOHINT`: light disables horizontal hinting and keeps
        // integer original advances.
        FT_RENDER_MODE_LIGHT => api::LoadFlags::TARGET_LIGHT,
        FT_RENDER_MODE_MONO => api::LoadFlags::TARGET_MONO,
        FT_RENDER_MODE_LCD => api::LoadFlags::TARGET_LCD,
        FT_RENDER_MODE_LCD_V => api::LoadFlags::TARGET_LCD_V,
        // FreeType does not reject unknown target nibbles during load-only
        // calls.  It only fails later if FT_LOAD_RENDER asks the renderer to
        // consume that invalid mode (ftobjs.c:1168-1176).
        _ if flags & FT_LOAD_RENDER != 0 => return Err(FT_Err_Cannot_Render_Glyph),
        _ => api::LoadFlags::DEFAULT,
    };
    Ok(core)
}

pub fn render_mode_to_core(mode: FT_Render_Mode) -> Option<RenderMode> {
    match mode {
        FT_RENDER_MODE_NORMAL | FT_RENDER_MODE_LIGHT => Some(RenderMode::Normal),
        FT_RENDER_MODE_MONO => Some(RenderMode::Mono),
        FT_RENDER_MODE_LCD => Some(RenderMode::Lcd),
        FT_RENDER_MODE_LCD_V => Some(RenderMode::LcdV),
        FT_RENDER_MODE_SDF => Some(RenderMode::Sdf),
        FT_RENDER_MODE_MAX => None,
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

pub(super) fn load_flag_for_render_mode(mode: RenderMode) -> api::LoadFlags {
    match mode {
        RenderMode::Normal => api::LoadFlags::DEFAULT,
        RenderMode::Mono => api::LoadFlags::TARGET_MONO,
        RenderMode::Lcd => api::LoadFlags::TARGET_LCD,
        RenderMode::LcdV => api::LoadFlags::TARGET_LCD_V,
        RenderMode::Sdf => api::LoadFlags::DEFAULT,
    }
}

pub(super) fn error_to_ft(error: FontError) -> FT_Error {
    match error {
        FontError::InvalidFont(_) => FT_Err_Invalid_File_Format,
        FontError::UnsupportedCmapFormat(_) => FT_Err_Invalid_CharMap_Format,
        FontError::RasterOverflow => FT_Err_Raster_Overflow,
        FontError::InvalidOutline(_) => FT_Err_Invalid_Outline,
        FontError::UnsupportedLoadFlags(_) => FT_Err_Unimplemented_Feature,
    }
}

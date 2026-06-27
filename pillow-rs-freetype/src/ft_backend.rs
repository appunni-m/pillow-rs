//! System FreeType rendering backend — matches PIL's `FT_LOAD_DEFAULT|FT_LOAD_RENDER` path.
//!
//! PIL's `ImageFont.getmask()` / `getbbox()` delegates to FreeType's built-in
//! autohinter and rasterizer via `FT_Load_Glyph` with `FT_LOAD_DEFAULT | FT_LOAD_RENDER`.
//! This module mirrors that by calling system FreeType through FFI.

use freetype_sys::{
    FT_Done_Face, FT_Done_FreeType, FT_Done_Glyph, FT_Get_Char_Index,
    FT_Get_Glyph, FT_Glyph_Get_CBox, FT_Init_FreeType, FT_Load_Glyph,
    FT_New_Memory_Face, FT_Request_Size, FT_Library, FT_Face,
    FT_GLYPH_BBOX_PIXELS,
    FT_Size_RequestRec, FT_SIZE_REQUEST_TYPE_NOMINAL,
};
use std::ptr;

use crate::error::FontError;
use crate::font::GlyphMask;

/// Holds a system FreeType library + face so we can render glyphs.
pub struct FtHandle {
    library: FT_Library,
    face: FT_Face,
}

impl FtHandle {
    pub fn new(font_bytes: &[u8], size_pt: f32) -> Result<Self, FontError> {
        let mut library: FT_Library = ptr::null_mut();
        let err = unsafe { FT_Init_FreeType(&mut library) };
        if err != 0 {
            return Err(FontError::InvalidFont(format!("FT_Init_FreeType failed: {}", err)));
        }

        let mut face: FT_Face = ptr::null_mut();
        let err = unsafe {
            FT_New_Memory_Face(library, font_bytes.as_ptr(), font_bytes.len() as _, 0, &mut face)
        };
        if err != 0 {
            unsafe { FT_Done_FreeType(library) };
            return Err(FontError::InvalidFont(format!("FT_New_Memory_Face failed: {}", err)));
        }

        // Match PIL: FT_Request_Size with NOMINAL, width=height=size*64, horiRes=vertRes=0
        let width = (size_pt * 64.0).round() as i64;
        let mut req = FT_Size_RequestRec {
            size_request_type: FT_SIZE_REQUEST_TYPE_NOMINAL,
            width,
            height: width,
            horiResolution: 0,
            vertResolution: 0,
        };
        let err = unsafe { FT_Request_Size(face, &mut req) };
        if err != 0 {
            unsafe { FT_Done_Face(face); FT_Done_FreeType(library); }
            return Err(FontError::InvalidFont(format!("FT_Request_Size failed: {}", err)));
        }

        Ok(FtHandle { library, face })
    }

    /// Get metrics: (ascender, descender) in pixels, both positive.
    pub fn getmetrics(&self) -> (u32, u32) {
        unsafe {
            let face = &*self.face;
            if let Some(size) = face.size.as_ref() {
                let asc = (size.metrics.ascender >> 6).max(0) as u32;
                let desc = ((-size.metrics.descender) >> 6).max(0) as u32;
                (asc, desc)
            } else {
                (0, 0)
            }
        }
    }

    /// Advance width for a string (in pixels, float).
    pub fn getlength(&self, text: &str) -> f32 {
        let mut total: f32 = 0.0;
        for ch in text.chars() {
            let glyph_index = unsafe { FT_Get_Char_Index(self.face, ch as u64) };
            if glyph_index == 0 { continue; }
            let err = unsafe { FT_Load_Glyph(self.face, glyph_index, 0) };
            if err != 0 { continue; }
            let glyph_slot = unsafe { &*(*self.face).glyph };
            total += glyph_slot.metrics.horiAdvance as f32 / 64.0;
        }
        total
    }

    /// Glyph bbox in PIL convention: `(left, top, right, bottom)` y-down from ascender.
    ///
    /// Mirrors PIL's `bounding_box_and_anchors`: FT_Load_Glyph → FT_Get_Glyph →
    /// FT_Glyph_Get_CBox(FT_GLYPH_BBOX_PIXELS).
    pub fn getbbox(&self, char_code: u32, asc_px: i32) -> (i32, i32, i32, i32) {
        let glyph_index = unsafe { FT_Get_Char_Index(self.face, char_code as u64) };
        if glyph_index == 0 {
            return (0, 0, 0, 0);
        }

        let err = unsafe { FT_Load_Glyph(self.face, glyph_index, 0) }; // FT_LOAD_DEFAULT
        if err != 0 { return (0, 0, 0, 0); }

        let glyph_slot = unsafe { &*(*self.face).glyph };
        let advance = glyph_slot.metrics.horiAdvance;

        let mut ft_glyph: freetype_sys::FT_Glyph = ptr::null_mut();
        let err = unsafe { FT_Get_Glyph((*self.face).glyph, &mut ft_glyph) };
        if err != 0 { return (0, 0, 0, 0); }

        let mut bbox = freetype_sys::FT_BBox { xMin: 0, yMin: 0, xMax: 0, yMax: 0 };
        unsafe { FT_Glyph_Get_CBox(ft_glyph, FT_GLYPH_BBOX_PIXELS as u32, &mut bbox) };
        unsafe { FT_Done_Glyph(ft_glyph) };

        let adv_px = ((advance + 32) & !63) >> 6;
        let left = 0_i32;
        let top = asc_px as i64 - bbox.yMax;
        let right = bbox.xMax.max(adv_px).max(0);
        let bottom = (asc_px as i64 - bbox.yMin).max(asc_px as i64);

        (left, top as i32, right as i32, bottom as i32)
    }

    /// Render glyph mask via `FT_LOAD_RENDER`, sized to the glyph's pixel bbox.
    pub fn getmask(&self, char_code: u32, asc_px: i32) -> Result<GlyphMask, FontError> {
        let glyph_index = unsafe { FT_Get_Char_Index(self.face, char_code as u64) };
        if glyph_index == 0 {
            return Ok(GlyphMask { width: 0, height: 0, pixels: Vec::new() });
        }

        // Get bbox first (without RENDER)
        let err = unsafe { FT_Load_Glyph(self.face, glyph_index, 0) };
        if err != 0 { return Err(FontError::InvalidOutline(format!("FT_Load_Glyph failed: {err}"))); }

        let glyph_slot = unsafe { &*(*self.face).glyph };
        let advance = glyph_slot.metrics.horiAdvance;

        let mut ft_glyph: freetype_sys::FT_Glyph = ptr::null_mut();
        let err = unsafe { FT_Get_Glyph((*self.face).glyph, &mut ft_glyph) };
        if err != 0 { return Err(FontError::InvalidOutline(format!("FT_Get_Glyph failed: {err}"))); }

        let mut bbox = freetype_sys::FT_BBox { xMin: 0, yMin: 0, xMax: 0, yMax: 0 };
        unsafe { FT_Glyph_Get_CBox(ft_glyph, FT_GLYPH_BBOX_PIXELS as u32, &mut bbox) };
        unsafe { FT_Done_Glyph(ft_glyph) };

        let adv_px = ((advance + 32) & !63) >> 6;
        let mask_width = adv_px.max(bbox.xMax).max(0) as u32;
        let mask_top = asc_px as i64 - bbox.yMax;
        let mask_bottom = (asc_px as i64 - bbox.yMin).max(asc_px as i64);
        let mask_height = (mask_bottom - mask_top).max(0) as u32;

        if mask_width == 0 || mask_height == 0 {
            return Ok(GlyphMask { width: mask_width, height: mask_height, pixels: Vec::new() });
        }

        // Now render
        let err = unsafe { FT_Load_Glyph(self.face, glyph_index, 0x4) }; // FT_LOAD_RENDER
        if err != 0 { return Err(FontError::InvalidOutline(format!("FT_Load_Glyph RENDER failed: {err}"))); }

        let glyph_slot2 = unsafe { &*(*self.face).glyph };
        let bmp = &glyph_slot2.bitmap;
        if bmp.width == 0 || bmp.rows == 0 || bmp.buffer.is_null() {
            return Ok(GlyphMask { width: mask_width, height: mask_height, pixels: vec![0u8; (mask_width * mask_height) as usize] });
        }

        let mut pixels = vec![0u8; (mask_width * mask_height) as usize];
        let src = unsafe { std::slice::from_raw_parts(bmp.buffer, (bmp.width * bmp.rows) as usize) };
        let x_offs = glyph_slot2.bitmap_left as usize;
        let y_offs = (asc_px as i64 - glyph_slot2.bitmap_top as i64 - mask_top).max(0) as usize;

        let w = bmp.width as usize;
        let rows = bmp.rows as usize;
        let mw = mask_width as usize;
        let mh = mask_height as usize;
        for row in 0..rows {
            let yr = y_offs.saturating_add(row);
            if yr >= mh { continue; }
            let src_off = row * w;
            let dst_off = yr.saturating_mul(mw).saturating_add(x_offs);
            if dst_off.saturating_add(w) <= pixels.len() && src_off + w <= src.len() {
                let copy = w.min(mw.saturating_sub(x_offs));
                pixels[dst_off..dst_off + copy].copy_from_slice(&src[src_off..src_off + copy]);
            }
        }

        Ok(GlyphMask { width: mask_width, height: mask_height, pixels })
    }
}

impl Drop for FtHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.face.is_null() { FT_Done_Face(self.face); }
            if !self.library.is_null() { FT_Done_FreeType(self.library); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ft_handle_basics() {
        let font_bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/input/fonts_nohint/DejaVuSans.ttf"
        )).unwrap();
        let handle = FtHandle::new(&font_bytes, 10.0).unwrap();
        let (asc, desc) = handle.getmetrics();
        assert_eq!(asc, 10);
        assert_eq!(desc, 3);
        // '-' (codepoint 45)
        let bbox = handle.getbbox(45, asc as i32);
        assert_eq!(bbox, (0, 7, 4, 10));
        let mask = handle.getmask(45, asc as i32).unwrap();
        assert_eq!(mask.width, 4);
        assert_eq!(mask.height, 3);
    }
}

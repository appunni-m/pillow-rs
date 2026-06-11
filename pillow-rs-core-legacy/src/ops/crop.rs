use crate::error::PilError;
use crate::image::Image;
use image::DynamicImage;

impl Image {
    /// Crop expects (x, y, width, height) — matching crop_imm format.
    /// Python wrapper converts Pillow's (left, top, right, bottom) to this format.
    /// Uses puhu's raw-byte memcpy pattern for 8-bit modes (avoids image crate overhead).
    pub fn crop(&self, box_coords: (u32, u32, u32, u32)) -> Result<Image, PilError> {
        let (x, y, w, h) = box_coords;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError(
                "crop box must have positive dimensions".into(),
            ));
        }
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let (img_w, img_h) = (img.width(), img.height());

        if x + w > img_w || y + h > img_h {
            return Err(PilError::ValueError(format!(
                "crop box (x={}, y={}, w={}, h={}) exceeds image bounds ({}x{})",
                x, y, w, h, img_w, img_h
            )));
        }

        // Fast path for RGB8/RGBA8: raw byte memcpy (puhu pattern)
        let cropped = if let Some(rgb) = img.as_rgb8() {
            crop_raw(rgb.as_raw(), img_w as usize, x as usize, y as usize, w as usize, h as usize, 3)
                .map(|raw| DynamicImage::ImageRgb8(
                    image::RgbImage::from_raw(w, h, raw).unwrap()))
        } else if let Some(rgba) = img.as_rgba8() {
            crop_raw(rgba.as_raw(), img_w as usize, x as usize, y as usize, w as usize, h as usize, 4)
                .map(|raw| DynamicImage::ImageRgba8(
                    image::RgbaImage::from_raw(w, h, raw).unwrap()))
        } else if let Some(gray) = img.as_luma8() {
            crop_raw(gray.as_raw(), img_w as usize, x as usize, y as usize, w as usize, h as usize, 1)
                .map(|raw| DynamicImage::ImageLuma8(
                    image::GrayImage::from_raw(w, h, raw).unwrap()))
        } else if let Some(gray_a) = img.as_luma_alpha8() {
            crop_raw(gray_a.as_raw(), img_w as usize, x as usize, y as usize, w as usize, h as usize, 2)
                .map(|raw| DynamicImage::ImageLumaA8(
                    image::GrayAlphaImage::from_raw(w, h, raw).unwrap()))
        } else {
            // Fallback: use image crate's generic crop
            Some(img.crop_imm(x, y, w, h))
        };

        cropped.map(|c| Image {
            inner: crate::lazy::LazyImage::Loaded(c),
            format: self.format,
        }).ok_or_else(|| PilError::ValueError("crop: failed to construct output".into()))
    }
}

/// Row-by-row memcpy for crop (puhu pattern). Avoids image crate trait overhead.
fn crop_raw(raw: &[u8], src_width: usize, x: usize, y: usize, w: usize, h: usize, bpp: usize) -> Option<Vec<u8>> {
    let row_bytes = w * bpp;
    let mut dst = Vec::with_capacity(h * row_bytes);
    for row in 0..h {
        let start = ((y + row) * src_width + x) * bpp;
        dst.extend_from_slice(raw.get(start..start + row_bytes)?);
    }
    Some(dst)
}

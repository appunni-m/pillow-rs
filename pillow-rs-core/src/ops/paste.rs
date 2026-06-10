//! Paste operations — image overlay, color fill, and mask-based alpha blending.

use image::{DynamicImage, GenericImage, GenericImageView};

use crate::error::PilError;
use crate::image::Image;

/// Source for paste: another image or a solid RGBA color.
pub enum PasteSource {
    Image(Image),
    Color((u8, u8, u8, u8)),
}

impl Image {
    /// Paste source image or color onto self (mutates in-place, matching Pillow).
    ///
    /// `box_coords`: None = (0,0), 4-tuple = (left,top,right,bottom).
    pub fn paste(
        &mut self,
        source: PasteSource,
        box_coords: Option<(i32, i32, i32, i32)>,
        mask: Option<&Image>,
    ) -> Result<(), PilError> {
        match source {
            PasteSource::Image(ref src_img) => {
                paste_image(self, src_img, box_coords, mask)?;
            }
            PasteSource::Color(rgba) => {
                paste_color_fill(self, rgba, box_coords)?;
            }
        }
        Ok(())
    }
}

/// Paste one image onto another at the given position.
fn paste_image(
    dest: &mut Image,
    src: &Image,
    box_coords: Option<(i32, i32, i32, i32)>,
    mask: Option<&Image>,
) -> Result<(), PilError> {
    let mut src_clone = src.clone();
    let src_img = src_clone.ensure_loaded()?;
    let (src_w, src_h) = (src_img.width(), src_img.height());

    let (paste_x, paste_y) = match box_coords {
        None => (0i64, 0i64),
        Some((x1, y1, x2, y2)) => {
            if x2 > x1 || y2 > y1 {
                (x1 as i64, y1 as i64)
            } else {
                (x1 as i64, y1 as i64)
            }
        }
    };

    if let Some(mask) = mask {
        let mut mask_clone = mask.clone();
        let mask_img = mask_clone.ensure_loaded()?;
        let mask_gray = mask_img.to_luma8();
        let mut dest_clone = dest.ensure_loaded()?.clone();

        for py in 0..src_h {
            for px in 0..src_w {
                let mask_val = mask_gray.get_pixel(px, py)[0];
                if mask_val == 0 {
                    continue;
                }
                let sp = src_img.get_pixel(px, py);
                let dx = (paste_x + px as i64) as u32;
                let dy = (paste_y + py as i64) as u32;
                if dx >= dest_clone.width() || dy >= dest_clone.height() {
                    continue;
                }
                if mask_val == 255 {
                    dest_clone.put_pixel(dx, dy, sp);
                } else {
                    let inv_alpha = 255u16 - mask_val as u16;
                    let dp = dest_clone.get_pixel(dx, dy);
                    let blended = image::Rgba([
                        blend_u8(sp[0], dp[0], mask_val, inv_alpha),
                        blend_u8(sp[1], dp[1], mask_val, inv_alpha),
                        blend_u8(sp[2], dp[2], mask_val, inv_alpha),
                        blend_u8(
                            sp.0.get(3).copied().unwrap_or(255),
                            dp.0.get(3).copied().unwrap_or(255),
                            mask_val,
                            inv_alpha,
                        ),
                    ]);
                    dest_clone.put_pixel(dx, dy, blended);
                }
            }
        }
        dest.inner = crate::lazy::LazyImage::Loaded(dest_clone);
    } else {
        let mut dest_clone = dest.ensure_loaded()?.clone();
        image::imageops::overlay(&mut dest_clone, src_img, paste_x, paste_y);
        dest.inner = crate::lazy::LazyImage::Loaded(dest_clone);
    }

    Ok(())
}

/// Fill a region with a solid color (matching Pillow's color paste).
fn paste_color_fill(
    dest: &mut Image,
    (r, g, b, a): (u8, u8, u8, u8),
    box_coords: Option<(i32, i32, i32, i32)>,
) -> Result<(), PilError> {
    let (x, y, w, h) = match box_coords {
        Some((x1, y1, x2, y2)) if x2 > x1 && y2 > y1 => {
            (x1 as u32, y1 as u32, (x2 - x1) as u32, (y2 - y1) as u32)
        }
        _ => {
            return Err(PilError::ValueError(
                "color paste requires a 4-tuple box to define the region".into(),
            ));
        }
    };

    let mut dest_clone = dest.ensure_loaded()?.clone();
    let (end_x, end_y) = ((x + w).min(dest_clone.width()), (y + h).min(dest_clone.height()));

    if a == 255 {
        for py in y..end_y {
            for px in x..end_x {
                dest_clone.put_pixel(px, py, image::Rgba([r, g, b, 255]));
            }
        }
    } else if a > 0 {
        for py in y..end_y {
            for px in x..end_x {
                let existing = dest_clone.get_pixel(px, py);
                let inv_a = 255u16 - a as u16;
                let blended = image::Rgba([
                    blend_u8(r, existing[0], a, inv_a),
                    blend_u8(g, existing[1], a, inv_a),
                    blend_u8(b, existing[2], a, inv_a),
                    blend_u8(a, existing.0.get(3).copied().unwrap_or(255), a, inv_a),
                ]);
                dest_clone.put_pixel(px, py, blended);
            }
        }
    }

    dest.inner = crate::lazy::LazyImage::Loaded(dest_clone);
    Ok(())
}

#[inline]
fn blend_u8(src: u8, dst: u8, alpha: u8, inv_alpha: u16) -> u8 {
    let a = alpha as u16;
    (((src as u16 * a) + (dst as u16 * inv_alpha) + 127) / 255) as u8
}

//! Paste operations — image overlay, color fill, and mask-based alpha blending.

use std::sync::Arc;

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Source for paste: another image or a solid RGBA color.
pub enum PasteSource {
    Image(Image),
    Color((u8, u8, u8, u8)),
}

impl PasteSource {
    /// Build from extracted Rust values. Image takes priority over raw color values.
    pub fn from_parts(image: Option<Image>, r: u8, g: u8, b: u8, a: u8) -> Self {
        if let Some(img) = image {
            PasteSource::Image(img)
        } else {
            PasteSource::Color((r, g, b, a))
        }
    }
}

impl Image {
    /// Paste source image or color onto self (mutates in-place, matching Pillow).
    pub fn paste(
        &mut self,
        source: PasteSource,
        box_coords: Option<(i32, i32, i32, i32)>,
        mask: Option<&Image>,
    ) -> Result<(), PilError> {
        let (src_img, x, y, w, h) = match source {
            PasteSource::Image(ref img) => {
                let size = img.size()?;
                match box_coords {
                    None => {
                        let (sw, sh) = size;
                        (img.clone(), 0i32, 0i32, sw as i32, sh as i32)
                    }
                    Some((x1, y1, x2, y2)) => (img.clone(), x1, y1, x2 - x1, y2 - y1),
                }
            }
            PasteSource::Color(rgba) => {
                let (bx, by, bw, bh) = match box_coords {
                    Some((x1, y1, x2, y2)) if x2 > x1 && y2 > y1 => (x1, y1, x2 - x1, y2 - y1),
                    _ => {
                        return Err(PilError::ValueError(
                            "color paste requires a 4-tuple box to define the region".into(),
                        ));
                    }
                };
                let (r, g, b, a) = rgba;
                let color_img = Image::new(bw as u32, bh as u32, "RGBA", (r, g, b, a))?;
                (color_img, bx, by, bw, bh)
            }
        };

        let mask_arc = mask.map(|m| Arc::new(m.clone()));
        let new_self = Image::push_op(
            self,
            PipelineOp::Paste {
                source: Arc::new(src_img),
                x,
                y,
                w,
                h,
                mask: mask_arc,
            },
        );
        *self = new_self;
        Ok(())
    }

    /// Alpha composite source image onto self (mutates in-place).
    /// dest: offset into self, src: offset into source.
    pub fn alpha_composite(
        &mut self,
        source: &Image,
        dest: (i32, i32),
        src: (i32, i32),
    ) -> Result<(), PilError> {
        let new_self = Image::push_op(
            self,
            PipelineOp::AlphaComposite {
                source: Arc::new(source.clone()),
                dest,
                src,
            },
        );
        *self = new_self;
        Ok(())
    }
}

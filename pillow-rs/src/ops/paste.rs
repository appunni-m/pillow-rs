//! Paste operations — image overlay, color fill, and mask-based alpha blending.

use std::sync::Arc;

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Source pixels for [`Image::paste`].
pub enum PasteSource {
    /// Paste pixels from another image.
    Image(Image),
    /// Paste a solid RGBA color.
    Color((u8, u8, u8, u8)),
}

impl PasteSource {
    /// Builds a paste source from binding-normalized values.
    ///
    /// When `image` is present it takes priority. Otherwise the RGBA tuple is
    /// used as a solid color source.
    pub fn from_parts(image: Option<Image>, r: u8, g: u8, b: u8, a: u8) -> Self {
        if let Some(img) = image {
            PasteSource::Image(img)
        } else {
            PasteSource::Color((r, g, b, a))
        }
    }
}

impl Image {
    /// Queues a Pillow-style paste into this image.
    ///
    /// Image sources default to pasting at `(0, 0)` when `box_coords` is absent.
    /// Color sources require a four-coordinate box so the fill region has a
    /// defined width and height. `mask`, when present, is carried into the
    /// pipeline for masked paste semantics.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when a color paste has no valid box.
    /// Returns other [`PilError`] values when source size lookup or color-image
    /// construction fails.
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

    /// Queues alpha compositing of `source` over this image.
    ///
    /// `dest` is the offset in the destination image and `src` is the offset in
    /// the source image.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when source and destination dimensions
    /// differ, or another [`PilError`] when size lookup fails.
    pub fn alpha_composite(
        &mut self,
        source: &Image,
        dest: (i32, i32),
        src: (i32, i32),
    ) -> Result<(), PilError> {
        let (w1, h1) = self.size()?;
        let (w2, h2) = source.size()?;
        if (w1, h1) != (w2, h2) {
            return Err(PilError::ValueError("images do not match".into()));
        }
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

//! Paste operations — image overlay, color fill, and mask-based alpha blending.

use std::sync::Arc;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Source pixels for [`Image::paste`].
#[derive(Debug, Clone)]
pub enum PasteSource {
    /// Paste pixels from another image.
    Image(Image),
    /// Paste a scalar pixel value.
    Scalar(u8),
    /// Paste a two-band luma/alpha pixel value.
    LumaAlpha(u8, u8),
    /// Paste a three-band pixel value.
    Rgb(u8, u8, u8),
    /// Paste a four-band pixel value.
    Rgba(u8, u8, u8, u8),
    /// Backwards-compatible spelling for a solid RGBA color.
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
            PasteSource::Rgba(r, g, b, a)
        }
    }

    fn solid_color(&self, mode: &str) -> Result<(u8, u8, u8, u8), PilError> {
        let bad_single =
            || PilError::TypeError("color must be int or single-element tuple".to_owned());
        let bad_la =
            || PilError::TypeError("color must be int, or tuple of one or two elements".to_owned());
        let bad_multi = || {
            PilError::TypeError(
                "color must be int, or tuple of one, three or four elements".to_owned(),
            )
        };

        match (mode, self) {
            ("1" | "L" | "P", PasteSource::Scalar(value)) => Ok((*value, *value, *value, 255)),
            ("1" | "L" | "P", _) => Err(bad_single()),
            ("LA", PasteSource::Scalar(value)) => Ok((*value, *value, *value, 0)),
            ("LA", PasteSource::LumaAlpha(luma, alpha)) => Ok((*luma, *luma, *luma, *alpha)),
            ("LA", _) => Err(bad_la()),
            ("RGB" | "RGBA" | "CMYK" | "YCbCr" | "HSV", PasteSource::Scalar(value)) => {
                Ok((*value, 0, 0, 0))
            }
            ("RGB" | "YCbCr" | "HSV", PasteSource::Rgb(r, g, b)) => Ok((*r, *g, *b, 255)),
            ("RGB" | "YCbCr" | "HSV", PasteSource::Rgba(r, g, b, _))
            | ("RGB" | "YCbCr" | "HSV", PasteSource::Color((r, g, b, _))) => Ok((*r, *g, *b, 255)),
            ("RGBA" | "CMYK", PasteSource::Rgb(r, g, b)) => Ok((*r, *g, *b, 255)),
            ("RGBA" | "CMYK", PasteSource::Rgba(r, g, b, a))
            | ("RGBA" | "CMYK", PasteSource::Color((r, g, b, a))) => Ok((*r, *g, *b, *a)),
            ("RGB" | "RGBA" | "CMYK" | "YCbCr" | "HSV", _) => Err(bad_multi()),
            ("I" | "F", PasteSource::Scalar(value)) => Ok((*value, 0, 0, 0)),
            ("I" | "F", _) => Err(bad_single()),
            (_, _) => Err(PilError::ValueError(format!(
                "unsupported paste destination mode {mode}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PastePlacement {
    Position(i32, i32),
    Region(i32, i32, i32, i32),
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
        let placement = box_coords.map_or(PastePlacement::Position(0, 0), |coords| {
            PastePlacement::Region(coords.0, coords.1, coords.2, coords.3)
        });
        self.paste_impl(source, placement, mask)
    }

    /// Queues a paste using Pillow's two-coordinate upper-left form.
    ///
    /// Image sources derive the region size from the source image. Solid values
    /// derive it from `mask`, when supplied; without either size source Pillow
    /// reports that a four-item box is required.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Image::paste`].
    pub fn paste_at(
        &mut self,
        source: PasteSource,
        position: Option<(i32, i32)>,
        mask: Option<&Image>,
    ) -> Result<(), PilError> {
        let (x, y) = position.unwrap_or((0, 0));
        self.paste_impl(source, PastePlacement::Position(x, y), mask)
    }

    fn paste_impl(
        &mut self,
        source: PasteSource,
        placement: PastePlacement,
        mask: Option<&Image>,
    ) -> Result<(), PilError> {
        let destination_mode = self.mode()?;
        let source_size = match &source {
            PasteSource::Image(image) => Some(image.size()?),
            _ => None,
        };
        let mask_size = mask.map(Image::size).transpose()?;
        let (x, y, width, height) = match placement {
            PastePlacement::Position(x, y) => {
                let (width, height) = source_size.or(mask_size).ok_or_else(|| {
                    PilError::ValueError("cannot determine region size; use 4-item box".to_owned())
                })?;
                (x, y, width, height)
            }
            PastePlacement::Region(left, top, right, bottom) => {
                let width = right
                    .checked_sub(left)
                    .and_then(|value| u32::try_from(value).ok())
                    .filter(|value| *value != 0)
                    .ok_or_else(|| PilError::ValueError("images do not match".to_owned()))?;
                let height = bottom
                    .checked_sub(top)
                    .and_then(|value| u32::try_from(value).ok())
                    .filter(|value| *value != 0)
                    .ok_or_else(|| PilError::ValueError("images do not match".to_owned()))?;
                (left, top, width, height)
            }
        };

        if source_size.is_some_and(|size| size != (width, height))
            || mask_size.is_some_and(|size| size != (width, height))
        {
            return Err(PilError::ValueError("images do not match".to_owned()));
        }

        let mask_alpha = if let Some(mask_image) = mask {
            match mask_image.mode()?.as_str() {
                "1" | "L" => false,
                "LA" | "RGBA" | "RGBa" => true,
                _ => {
                    return Err(PilError::ValueError("bad transparency mask".to_owned()));
                }
            }
        } else {
            false
        };

        let source_image = match source {
            PasteSource::Image(image) => {
                let source_mode = image.mode()?;
                if source_mode == destination_mode
                    || (destination_mode == "RGB"
                        && matches!(source_mode.as_str(), "LA" | "RGBA" | "RGBa"))
                {
                    image
                } else {
                    // Pillow Image.py converts a mismatched source before entering
                    // libImaging/Paste.c. Keep that conversion shared by every
                    // backend so CPU, GPU, and SIMD receive identical bytes.
                    image.convert(&destination_mode, None, None, None, None)?
                }
            }
            solid => {
                let color = solid.solid_color(&destination_mode)?;
                if destination_mode == "P" {
                    let dims = CheckedDims::new(width, height, 1)?;
                    let mut indices = dims.alloc_buffer();
                    indices.fill(color.0);
                    Image::frombytes("P", (width, height), &indices)?
                } else {
                    Image::new(width, height, &destination_mode, color)?
                }
            }
        };

        let w = i32::try_from(width)
            .map_err(|_| PilError::ValueError("images do not match".to_owned()))?;
        let h = i32::try_from(height)
            .map_err(|_| PilError::ValueError("images do not match".to_owned()))?;
        let new_self = Image::push_op(
            self,
            PipelineOp::Paste {
                source: Arc::new(source_image),
                x,
                y,
                w,
                h,
                mask: mask.map(|image| Arc::new(image.clone())),
                mask_alpha,
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

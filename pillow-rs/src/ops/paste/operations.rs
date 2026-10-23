use std::sync::Arc;

use super::input::{
    coordinate, invalid_component_error, paste_scalar_source, paste_source_from_color_string,
};
use super::types::{
    AlphaCompositeBox, PastePlacement, PasteSource, PythonPasteBox, PythonPasteMask,
    PythonPasteSource, ResolvedPasteColor,
};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;
use crate::ops::crop::check_crop_extent;
use crate::pipeline::PipelineOp;

fn coordinate_overflow() -> PilError {
    PilError::OverflowError("signed integer is greater than maximum".into())
}

impl Image {
    /// Applies the Python `Image.paste` input contract before queuing paste.
    ///
    /// Python and PyO3 bindings only classify host values into these neutral
    /// records. Coordinate arity, abbreviated-box semantics, color arity,
    /// mask errors, and the actual paste path all remain shared Rust logic.
    pub fn paste_with_input(
        &mut self,
        source: PythonPasteSource,
        box_input: PythonPasteBox,
        mask_input: PythonPasteMask,
    ) -> Result<(), PilError> {
        let destination_mode = self.mode()?;
        let source = match source {
            PythonPasteSource::Image(image) => PasteSource::from_parts(Some(*image), 0, 0, 0, 255),
            PythonPasteSource::Scalar(value) => paste_scalar_source(value, &destination_mode),
            PythonPasteSource::Float(value) => PasteSource::RawFloat(value),
            PythonPasteSource::Components(values) => match values.as_slice() {
                [value] => paste_scalar_source(*value, &destination_mode),
                [luma, alpha] => match (u8::try_from(*luma), u8::try_from(*alpha)) {
                    (Ok(luma), Ok(alpha)) => PasteSource::LumaAlpha(luma, alpha),
                    _ => PasteSource::RawLumaAlpha(*luma, *alpha),
                },
                [r, g, b] => match (u8::try_from(*r), u8::try_from(*g), u8::try_from(*b)) {
                    (Ok(r), Ok(g), Ok(b)) => PasteSource::Rgb(r, g, b),
                    _ => PasteSource::RawRgb(*r, *g, *b),
                },
                [r, g, b, a] => match (
                    u8::try_from(*r),
                    u8::try_from(*g),
                    u8::try_from(*b),
                    u8::try_from(*a),
                ) {
                    (Ok(r), Ok(g), Ok(b), Ok(a)) => PasteSource::from_parts(None, r, g, b, a),
                    _ => PasteSource::RawRgba(*r, *g, *b, *a),
                },
                _ => return Err(invalid_component_error(&destination_mode)),
            },
            PythonPasteSource::String(value) => {
                paste_source_from_color_string(&value, &destination_mode)?
            }
            PythonPasteSource::Invalid => {
                return Err(invalid_component_error(&destination_mode));
            }
        };

        let abbreviated_mask = match &box_input {
            PythonPasteBox::Image(mask) => Some((**mask).clone()),
            _ => None,
        };
        if abbreviated_mask.is_some() && !matches!(&mask_input, PythonPasteMask::None) {
            return Err(PilError::ValueError(
                "If using second argument as mask, third argument must be None".to_owned(),
            ));
        }
        let mask = if let Some(image) = abbreviated_mask {
            Some(image)
        } else {
            match mask_input {
                PythonPasteMask::None => None,
                PythonPasteMask::Image(image) => Some(*image),
                PythonPasteMask::Invalid(type_name) => {
                    return Err(PilError::AttributeError(format!(
                        "'{type_name}' object has no attribute 'load'"
                    )));
                }
            }
        };

        match box_input {
            PythonPasteBox::None | PythonPasteBox::Image(_) => {
                self.paste_at(source, None, mask.as_ref())
            }
            PythonPasteBox::Values(values) => match values.as_slice() {
                [x, y] => {
                    // Pillow rejects a two-coordinate solid-color paste for
                    // lack of a region size before it validates the unused
                    // coordinates. Keep that error ordering in core so a
                    // huge coordinate does not change the public error.
                    if !matches!(&source, PasteSource::Image(_)) && mask.is_none() {
                        return self.paste_at(source, None, None);
                    }
                    self.paste_at(
                        source,
                        Some((coordinate(*x)?, coordinate(*y)?)),
                        mask.as_ref(),
                    )
                }
                [left, top, right, bottom] => self.paste(
                    source,
                    Some((
                        coordinate(*left)?,
                        coordinate(*top)?,
                        coordinate(*right)?,
                        coordinate(*bottom)?,
                    )),
                    mask.as_ref(),
                ),
                _ => Err(PilError::TypeError(format!(
                    "argument 2 must be sequence of length 4, not {}",
                    values.len()
                ))),
            },
            PythonPasteBox::Invalid { length, type_name } => match length {
                Some(length) => Err(PilError::TypeError(format!(
                    "argument 2 must be sequence of length 4, not {length}"
                ))),
                None => Err(PilError::TypeError(format!(
                    "object of type '{type_name}' has no len()"
                ))),
            },
        }
    }

    /// Performs Pillow's high-level in-place alpha composite workflow.
    ///
    /// The low-level [`Image::alpha_composite`] primitive requires equal-sized
    /// images. Pillow's public method first crops the source, crops or creates
    /// the destination background, composites the matching regions, and pastes
    /// the result back. Keeping that geometry here prevents Python/FFI copies
    /// of the same validation and clipping rules.
    pub fn alpha_composite_public(
        &mut self,
        source: &Image,
        dest: AlphaCompositeBox,
        source_box: AlphaCompositeBox,
    ) -> Result<(), PilError> {
        let source_box = match source_box {
            AlphaCompositeBox::Values(values) if matches!(values.len(), 2 | 4) => values,
            AlphaCompositeBox::Values(_) => {
                return Err(PilError::ValueError(
                    "Source must be a sequence of length 2 or 4".into(),
                ));
            }
            AlphaCompositeBox::Invalid => {
                return Err(PilError::ValueError(
                    "Source must be a list or tuple".into(),
                ));
            }
        };
        let dest = match dest {
            AlphaCompositeBox::Values(values) if values.len() == 2 => values,
            AlphaCompositeBox::Values(_) => {
                return Err(PilError::ValueError(
                    "Destination must be a sequence of length 2".into(),
                ));
            }
            AlphaCompositeBox::Invalid => {
                return Err(PilError::ValueError(
                    "Destination must be a list or tuple".into(),
                ));
            }
        };
        if source_box.iter().any(|value| *value < 0) {
            return Err(PilError::ValueError("Source must be non-negative".into()));
        }

        let (source_width, source_height) = source.size()?;
        let source_width = i64::try_from(source_width).map_err(|_| coordinate_overflow())?;
        let source_height = i64::try_from(source_height).map_err(|_| coordinate_overflow())?;
        let source_box = if source_box.len() == 2 {
            [source_box[0], source_box[1], source_width, source_height]
        } else {
            [source_box[0], source_box[1], source_box[2], source_box[3]]
        };
        if source_box[2] < source_box[0] {
            return Err(PilError::ValueError(
                "Coordinate 'right' is less than 'left'".into(),
            ));
        }
        if source_box[3] < source_box[1] {
            return Err(PilError::ValueError(
                "Coordinate 'lower' is less than 'upper'".into(),
            ));
        }
        check_crop_extent(source_box[2] - source_box[0], source_box[3] - source_box[1])?;
        let source_box = (
            i32::try_from(source_box[0]).map_err(|_| coordinate_overflow())?,
            i32::try_from(source_box[1]).map_err(|_| coordinate_overflow())?,
            i32::try_from(source_box[2]).map_err(|_| coordinate_overflow())?,
            i32::try_from(source_box[3]).map_err(|_| coordinate_overflow())?,
        );
        let overlay = if source_box == (0, 0, source_width as i32, source_height as i32) {
            source.clone()
        } else {
            source.crop(Some(source_box))?
        };

        let dest_x = i32::try_from(dest[0]).map_err(|_| coordinate_overflow())?;
        let dest_y = i32::try_from(dest[1]).map_err(|_| coordinate_overflow())?;
        let (overlay_width, overlay_height) = overlay.size()?;
        let right = dest_x
            .checked_add(i32::try_from(overlay_width).map_err(|_| coordinate_overflow())?)
            .ok_or_else(coordinate_overflow)?;
        let bottom = dest_y
            .checked_add(i32::try_from(overlay_height).map_err(|_| coordinate_overflow())?)
            .ok_or_else(coordinate_overflow)?;
        let box_coords = (dest_x, dest_y, right, bottom);
        // Pillow's high-level method treats an empty cropped overlay as a
        // no-op.  The low-level alpha compositor rejects zero-sized buffers
        // as a mode/size mismatch, so avoid entering that primitive after the
        // public geometry has already been validated.
        if overlay_width == 0 || overlay_height == 0 {
            return Ok(());
        }
        let destination_size = self.size()?;
        if box_coords
            == (
                0,
                0,
                i32::try_from(destination_size.0).map_err(|_| coordinate_overflow())?,
                i32::try_from(destination_size.1).map_err(|_| coordinate_overflow())?,
            )
        {
            // A full-canvas unmasked paste would replace every output byte.
            // After the same geometry/empty checks, queue only its composite;
            // the low-level primitive retains mode and size validation. The
            // immutable source graph still protects shared/copy-on-write data.
            return self.alpha_composite(&overlay, (0, 0), (0, 0));
        }
        let background = self.crop(Some(box_coords))?;

        let mut result = background.copy();
        result.alpha_composite(&overlay, (0, 0), (0, 0))?;
        self.paste(PasteSource::Image(Box::new(result)), Some(box_coords), None)
    }

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
        let is_image = matches!(&source, PasteSource::Image(_));
        let Some((x, y, width, height)) =
            resolve_paste_placement(placement, source_size, mask_size, is_image)?
        else {
            return Ok(());
        };

        if source_size.is_some_and(|size| size != (width, height))
            || mask_size.is_some_and(|size| size != (width, height))
        {
            return Err(PilError::ValueError("images do not match".to_owned()));
        }
        let mask_alpha = paste_mask_uses_alpha(mask)?;
        let source_image =
            resolve_paste_source(source, &destination_mode, width, height, mask.is_some())?;

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
        // Pillow 12.2.0 `libImaging/AlphaComposite.c::ImagingAlphaComposite`
        // validates the destination mode first, then requires the source mode
        // and dimensions to match exactly.
        let dest_mode = self.mode()?;
        if dest_mode != "RGBA" && dest_mode != "LA" {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let source_mode = source.mode()?;
        if source_mode != dest_mode {
            return Err(PilError::ValueError("images do not match".into()));
        }
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

fn resolve_paste_placement(
    placement: PastePlacement,
    source_size: Option<(u32, u32)>,
    mask_size: Option<(u32, u32)>,
    is_image: bool,
) -> Result<Option<(i32, i32, u32, u32)>, PilError> {
    match placement {
        PastePlacement::Position(x, y) => {
            let (width, height) = source_size.or(mask_size).ok_or_else(|| {
                PilError::ValueError("cannot determine region size; use 4-item box".to_owned())
            })?;
            Ok(Some((x, y, width, height)))
        }
        PastePlacement::Region(left, top, right, bottom) => {
            let raw_width = right
                .checked_sub(left)
                .and_then(|value| u32::try_from(value).ok());
            let raw_height = bottom
                .checked_sub(top)
                .and_then(|value| u32::try_from(value).ok());
            let degenerate = raw_width.map_or(true, |value| value == 0)
                || raw_height.map_or(true, |value| value == 0);
            // Pillow Paste.c no-ops a degenerate solid fill; image pastes still
            // report the size mismatch.
            if degenerate && !is_image {
                return Ok(None);
            }
            let width = raw_width
                .filter(|value| *value != 0)
                .ok_or_else(|| PilError::ValueError("images do not match".to_owned()))?;
            let height = raw_height
                .filter(|value| *value != 0)
                .ok_or_else(|| PilError::ValueError("images do not match".to_owned()))?;
            Ok(Some((left, top, width, height)))
        }
    }
}

fn paste_mask_uses_alpha(mask: Option<&Image>) -> Result<bool, PilError> {
    let Some(mask_image) = mask else {
        return Ok(false);
    };
    match mask_image.mode()?.as_str() {
        "1" | "L" => Ok(false),
        "LA" | "RGBA" | "RGBa" => Ok(true),
        _ => Err(PilError::ValueError("bad transparency mask".to_owned())),
    }
}

fn resolve_paste_source(
    source: PasteSource,
    destination_mode: &str,
    width: u32,
    height: u32,
    has_mask: bool,
) -> Result<Image, PilError> {
    match source {
        PasteSource::Image(image) => resolve_image_source(image, destination_mode, has_mask),
        solid => resolve_solid_source(solid, destination_mode, width, height),
    }
}

fn resolve_image_source(
    image: Box<Image>,
    destination_mode: &str,
    has_mask: bool,
) -> Result<Image, PilError> {
    let source_mode = image.mode()?;
    if source_mode == destination_mode
        || (destination_mode == "RGB" && matches!(source_mode.as_str(), "LA" | "RGBA" | "RGBa"))
    {
        Ok(*image)
    } else if destination_mode == "RGBA" && source_mode == "RGB" && !has_mask {
        // Keep RGB source bytes native; the destination writer supplies opaque alpha.
        Ok(*image)
    } else if destination_mode == "PA" && source_mode == "P" {
        // Pillow promotes P sources to opaque PA samples and retains each index.
        let mut promoted = *image;
        promoted.putalpha(255)?;
        Ok(promoted)
    } else {
        // Convert before backend execution so every backend receives identical bytes.
        image.convert(destination_mode, None, None, None, None)
    }
}

fn resolve_solid_source(
    source: PasteSource,
    destination_mode: &str,
    width: u32,
    height: u32,
) -> Result<Image, PilError> {
    let color = source.solid_color(destination_mode)?;
    match color {
        ResolvedPasteColor::Bytes(color) if destination_mode == "P" => {
            let dims = CheckedDims::new(width, height, 1)?;
            let mut indices = dims.alloc_buffer();
            indices.fill(color.0);
            Image::frombytes("P", (width, height), &indices)
        }
        ResolvedPasteColor::Bytes(color) if destination_mode == "PA" => {
            // PA uses the LA physical layout while retaining its palette tag.
            Image::new(width, height, "LA", color)
        }
        ResolvedPasteColor::Integer(value) => {
            let bytes = value.to_le_bytes();
            Image::new(width, height, "I", (bytes[0], bytes[1], bytes[2], bytes[3]))
        }
        ResolvedPasteColor::Float(value) => {
            let bytes = value.to_le_bytes();
            Image::new(width, height, "F", (bytes[0], bytes[1], bytes[2], bytes[3]))
        }
        ResolvedPasteColor::Luma16(sample) => {
            let pixels = crate::raster::ImageBuffer::from_pixel(
                width,
                height,
                crate::raster::Luma([sample]),
            );
            Ok(Image::from_dynamic(
                crate::raster::DynamicImage::ImageLuma16(pixels),
                Some(destination_mode.to_owned()),
            ))
        }
        ResolvedPasteColor::Bytes(color) => Image::new(width, height, destination_mode, color),
    }
}

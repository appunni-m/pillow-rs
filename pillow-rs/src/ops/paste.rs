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

/// Host-neutral source input for the Python `Image.paste` wrapper.
#[derive(Debug, Clone)]
pub enum PythonPasteSource {
    /// Another image object.
    Image(Image),
    /// A scalar color value before mode-specific validation.
    Scalar(i64),
    /// A tuple/list color before mode-specific arity validation.
    Components(Vec<i64>),
    /// A Pillow color string before mode-specific conversion.
    String(String),
    /// A value that is neither an image nor a supported color value.
    Invalid,
}

/// Host-neutral box input for the Python `Image.paste` wrapper.
#[derive(Debug, Clone)]
pub enum PythonPasteBox {
    /// No second argument was supplied.
    None,
    /// The abbreviated `(image, mask)` form.
    Image(Image),
    /// A coordinate list/tuple.
    Values(Vec<i64>),
    /// A value that could not be interpreted as a coordinate sequence.
    Invalid {
        /// The sequence length when the host object exposed one.
        length: Option<usize>,
        /// The host type name for objects without a length.
        type_name: String,
    },
}

/// Host-neutral mask input for the Python `Image.paste` wrapper.
#[derive(Debug, Clone)]
pub enum PythonPasteMask {
    /// No mask was supplied.
    None,
    /// An image mask.
    Image(Image),
    /// A non-image object.
    Invalid(String),
}

/// Host-neutral coordinate input for public `Image.alpha_composite` calls.
#[derive(Debug, Clone)]
pub enum AlphaCompositeBox {
    /// A Python list/tuple represented as integer coordinates.
    Values(Vec<i64>),
    /// A value that was not an integer coordinate sequence.
    Invalid,
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
            ("LA" | "PA", PasteSource::Scalar(value)) => Ok((*value, *value, *value, 0)),
            ("LA" | "PA", PasteSource::LumaAlpha(luma, alpha)) => Ok((*luma, *luma, *luma, *alpha)),
            ("LA" | "PA", _) => Err(bad_la()),
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
            ("I;16" | "I;16L" | "I;16B" | "I;16N", PasteSource::Scalar(value)) => {
                Ok((*value, 0, 0, 0))
            }
            ("I;16" | "I;16L" | "I;16B" | "I;16N", _) => Err(bad_single()),
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
            PythonPasteSource::Image(image) => PasteSource::Image(image),
            PythonPasteSource::Scalar(value) => PasteSource::Scalar(byte_color(value)?),
            PythonPasteSource::Components(values) => match values.as_slice() {
                [value] => PasteSource::Scalar(byte_color(*value)?),
                [luma, alpha] => PasteSource::LumaAlpha(byte_color(*luma)?, byte_color(*alpha)?),
                [r, g, b] => PasteSource::Rgb(byte_color(*r)?, byte_color(*g)?, byte_color(*b)?),
                [r, g, b, a] => PasteSource::Rgba(
                    byte_color(*r)?,
                    byte_color(*g)?,
                    byte_color(*b)?,
                    byte_color(*a)?,
                ),
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
            PythonPasteBox::Image(mask) => Some(mask.clone()),
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
                PythonPasteMask::Image(image) => Some(image),
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
                [x, y] => self.paste_at(
                    source,
                    Some((coordinate(*x)?, coordinate(*y)?)),
                    mask.as_ref(),
                ),
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
        let source_box = if source_box.len() == 2 {
            (
                i32::try_from(source_box[0])
                    .map_err(|_| PilError::ValueError("Source coordinate overflow".into()))?,
                i32::try_from(source_box[1])
                    .map_err(|_| PilError::ValueError("Source coordinate overflow".into()))?,
                i32::try_from(source_width)
                    .map_err(|_| PilError::ValueError("Source coordinate overflow".into()))?,
                i32::try_from(source_height)
                    .map_err(|_| PilError::ValueError("Source coordinate overflow".into()))?,
            )
        } else {
            (
                i32::try_from(source_box[0])
                    .map_err(|_| PilError::ValueError("Source coordinate overflow".into()))?,
                i32::try_from(source_box[1])
                    .map_err(|_| PilError::ValueError("Source coordinate overflow".into()))?,
                i32::try_from(source_box[2])
                    .map_err(|_| PilError::ValueError("Source coordinate overflow".into()))?,
                i32::try_from(source_box[3])
                    .map_err(|_| PilError::ValueError("Source coordinate overflow".into()))?,
            )
        };
        let overlay = if source_box == (0, 0, source_width as i32, source_height as i32) {
            source.clone()
        } else {
            source.crop(Some(source_box))?
        };

        let dest_x = i32::try_from(dest[0])
            .map_err(|_| PilError::ValueError("Destination coordinate overflow".into()))?;
        let dest_y = i32::try_from(dest[1])
            .map_err(|_| PilError::ValueError("Destination coordinate overflow".into()))?;
        let (overlay_width, overlay_height) = overlay.size()?;
        let right = dest_x
            .checked_add(
                i32::try_from(overlay_width)
                    .map_err(|_| PilError::ValueError("Destination coordinate overflow".into()))?,
            )
            .ok_or_else(|| PilError::ValueError("Destination coordinate overflow".into()))?;
        let bottom = dest_y
            .checked_add(
                i32::try_from(overlay_height)
                    .map_err(|_| PilError::ValueError("Destination coordinate overflow".into()))?,
            )
            .ok_or_else(|| PilError::ValueError("Destination coordinate overflow".into()))?;
        let box_coords = (dest_x, dest_y, right, bottom);
        let destination_size = self.size()?;
        let background = if box_coords
            == (
                0,
                0,
                i32::try_from(destination_size.0)
                    .map_err(|_| PilError::ValueError("Destination coordinate overflow".into()))?,
                i32::try_from(destination_size.1)
                    .map_err(|_| PilError::ValueError("Destination coordinate overflow".into()))?,
            ) {
            self.clone()
        } else {
            self.crop(Some(box_coords))?
        };

        let mut result = background.copy();
        result.alpha_composite(&overlay, (0, 0), (0, 0))?;
        self.paste(PasteSource::Image(result), Some(box_coords), None)
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
        let (x, y, width, height) = match placement {
            PastePlacement::Position(x, y) => {
                let (width, height) = source_size.or(mask_size).ok_or_else(|| {
                    PilError::ValueError("cannot determine region size; use 4-item box".to_owned())
                })?;
                (x, y, width, height)
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
                // Pillow Paste.c no-ops a solid-color fill whose box is
                // degenerate (zero area or inverted edges); image pastes still
                // raise the mismatch error.
                if degenerate && !matches!(source, PasteSource::Image(_)) {
                    return Ok(());
                }
                let width = raw_width
                    .filter(|value| *value != 0)
                    .ok_or_else(|| PilError::ValueError("images do not match".to_owned()))?;
                let height = raw_height
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
                } else if destination_mode == "PA" && source_mode == "P" {
                    // Pillow promotes a P source to opaque PA samples before
                    // pasting, retaining the source index byte verbatim.
                    let mut promoted = image;
                    promoted.putalpha(255)?;
                    promoted
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
                } else if destination_mode == "PA" {
                    // PA's two raw bands use the same physical layout as LA;
                    // the destination retains the palette and PA mode tag.
                    Image::new(width, height, "LA", color)?
                } else if destination_mode == "F" {
                    // Pillow's Paste.c writes scalar F-mode colors as the
                    // destination's float32 sample, not as an integer byte.
                    // Image::new stores F samples as their four raw LE bytes.
                    let bytes = f32::from(color.0).to_le_bytes();
                    Image::new(width, height, "F", (bytes[0], bytes[1], bytes[2], bytes[3]))?
                } else if matches!(
                    destination_mode.as_str(),
                    "I;16" | "I;16L" | "I;16B" | "I;16N"
                ) {
                    // Pillow's Paste.c keeps I;16 scalar fills in unsigned
                    // 16-bit storage. Construct the source with the same
                    // native sample width; routing it through Image::new's
                    // RGBA8 fallback would discard the high byte.
                    // Image.paste receives an 8-bit scalar here. Pillow's
                    // I;16 getink path writes that byte to both bytes of the
                    // unsigned sample (7 becomes 0x0707), rather than
                    // interpreting it as the numeric sample 0x0007.
                    let sample = u16::from(color.0) * 0x0101;
                    let pixels = crate::raster::ImageBuffer::from_pixel(
                        width,
                        height,
                        crate::raster::Luma([sample]),
                    );
                    Image::from_dynamic(
                        crate::raster::DynamicImage::ImageLuma16(pixels),
                        Some(destination_mode.clone()),
                    )
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

fn byte_color(value: i64) -> Result<u8, PilError> {
    u8::try_from(value).map_err(|_| PilError::TypeError("im must be Image or color".to_owned()))
}

fn paste_source_from_color_string(value: &str, mode: &str) -> Result<PasteSource, PilError> {
    // Pillow resolves a string through ImageColor.getcolor for the
    // destination mode before entering Paste.c. Preserve that distinction
    // from tuple colors: luma/alpha and RGBA modes carry different arities.
    let (r, g, b, a) = crate::color::parse_color_str_unclamped(value)?;
    let color = crate::color::getcolor(r, g, b, a, mode)?;
    match color {
        crate::color::ColorValue::Gray(value) => {
            Ok(PasteSource::Scalar(byte_color(i64::from(value))?))
        }
        crate::color::ColorValue::GrayAlpha(value, alpha) => Ok(PasteSource::LumaAlpha(
            byte_color(i64::from(value))?,
            byte_color(i64::from(alpha))?,
        )),
        crate::color::ColorValue::Rgb(r, g, b) | crate::color::ColorValue::Hsv(r, g, b) => {
            Ok(PasteSource::Rgb(
                byte_color(i64::from(r))?,
                byte_color(i64::from(g))?,
                byte_color(i64::from(b))?,
            ))
        }
        crate::color::ColorValue::Rgba(r, g, b, a) => Ok(PasteSource::Rgba(
            byte_color(i64::from(r))?,
            byte_color(i64::from(g))?,
            byte_color(i64::from(b))?,
            byte_color(i64::from(a))?,
        )),
    }
}

fn coordinate(value: i64) -> Result<i32, PilError> {
    i32::try_from(value).map_err(|_| PilError::TypeError("coordinates must be integers".to_owned()))
}

fn invalid_component_error(mode: &str) -> PilError {
    let message = match mode {
        "F" => "must be real number, not tuple",
        "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" => {
            "color must be int or single-element tuple"
        }
        "LA" | "PA" => "color must be int, or tuple of one or two elements",
        _ => "color must be int, or tuple of one, three or four elements",
    };
    PilError::TypeError(message.to_owned())
}

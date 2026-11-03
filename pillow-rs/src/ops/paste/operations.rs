use std::sync::Arc;

use super::input::{
    coordinate, invalid_component_error, paste_float_source_error, paste_invalid_type_error,
    paste_list_source_error, paste_scalar_source, paste_source_from_color_string,
    paste_tuple_source, wide_integer_scalar_source,
};
use super::types::{
    AlphaCompositeBox, PasteBoxInput, PasteMaskInput, PastePlacement, PasteSource,
    PasteSourceInput, ResolvedPasteColor,
};
use crate::checked_dims::CheckedDims;
use crate::compute::registry::support::OperationId;
use crate::error::PilError;
use crate::image::Image;
use crate::ops::crop::check_crop_extent;
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

fn coordinate_overflow() -> PilError {
    PilError::OverflowError("signed integer is greater than maximum".into())
}

/// Normalized box syntax held between Pillow's staged Paste preparation steps.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct PasteBoxPlan {
    input: PasteBoxInput,
    abbreviated_mask: Option<Box<Image>>,
}

/// Mode-resolved source held until destination and mask preparation complete.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct PasteSourcePlan {
    destination_mode: Mode,
    source: PasteSource,
    resolved_color: Option<ResolvedPasteColor>,
}

impl Image {
    /// Applies the Python `Image.paste` input contract before queuing paste.
    ///
    /// The box is validated before source resolution. Image-source conversion
    /// and solid-color resolution are prepared before the caller realizes the
    /// destination and mask; the final method then validates dimensions and
    /// queues the mode-preserving Paste operation.
    pub fn paste_with_input(
        &mut self,
        source: PasteSourceInput,
        box_input: PasteBoxInput,
        mask_input: PasteMaskInput,
    ) -> Result<(), PilError> {
        let has_mask = !matches!(&mask_input, PasteMaskInput::None);
        let position_size_available = matches!(&source, PasteSourceInput::Image(_))
            || matches!(&mask_input, PasteMaskInput::Image(_))
            || matches!(&box_input, PasteBoxInput::Image(_));
        let box_plan = Self::prepare_paste_box(box_input, has_mask, position_size_available)?;
        let source_plan = self.prepare_paste_source(
            source,
            has_mask || box_plan.abbreviated_mask.is_some(),
        )?;
        self.paste_prepared(box_plan, source_plan, mask_input)
    }

    /// Validates Paste box syntax before source loading or conversion.
    #[doc(hidden)]
    pub fn prepare_paste_box(
        box_input: PasteBoxInput,
        has_mask: bool,
        position_size_available: bool,
    ) -> Result<PasteBoxPlan, PilError> {
        let (input, abbreviated_mask) = match box_input {
            PasteBoxInput::Image(mask) => {
                if has_mask {
                    return Err(PilError::ValueError(
                        "If using second argument as mask, third argument must be None".to_owned(),
                    ));
                }
                (PasteBoxInput::None, Some(mask))
            }
            input => (input, None),
        };
        match &input {
            PasteBoxInput::Values(values) if matches!(values.len(), 2 | 4) => {}
            PasteBoxInput::Values(values) => {
                return Err(PilError::TypeError(format!(
                    "argument 2 must be sequence of length 4, not {}",
                    values.len()
                )));
            }
            PasteBoxInput::Invalid { length, type_name } => {
                return Err(match length {
                    Some(length) => PilError::TypeError(format!(
                        "argument 2 must be sequence of length 4, not {length}"
                    )),
                    None => PilError::TypeError(format!(
                        "object of type '{type_name}' has no len()"
                    )),
                });
            }
            PasteBoxInput::None | PasteBoxInput::Image(_) => {}
        }
        if !position_size_available
            && (matches!(&input, PasteBoxInput::None)
                || matches!(&input, PasteBoxInput::Values(values) if values.len() == 2))
        {
            return Err(PilError::ValueError(
                "cannot determine region size; use 4-item box".to_owned(),
            ));
        }
        Ok(PasteBoxPlan {
            input,
            abbreviated_mask,
        })
    }

    /// Resolves an image or color source in the destination's exact mode.
    #[doc(hidden)]
    pub fn prepare_paste_source(
        &self,
        source_input: PasteSourceInput,
        has_mask: bool,
    ) -> Result<PasteSourcePlan, PilError> {
        let destination_mode = self.mode_identity()?;
        let source = match source_input {
            PasteSourceInput::Image(image) => PasteSource::from_parts(Some(*image), 0, 0, 0, 255),
            PasteSourceInput::Scalar(value) => paste_scalar_source(value, destination_mode),
            PasteSourceInput::WideInteger { float_value, .. } => {
                wide_integer_scalar_source(destination_mode, float_value)?
            }
            PasteSourceInput::Float(value) => {
                if destination_mode == Mode::ImageF32 {
                    PasteSource::RawFloat(value)
                } else {
                    return Err(paste_float_source_error(destination_mode));
                }
            }
            PasteSourceInput::TupleComponents(values) => {
                paste_tuple_source(&values, destination_mode)?
            }
            PasteSourceInput::List => return Err(paste_list_source_error(destination_mode)),
            PasteSourceInput::InvalidType(type_name) => {
                return Err(paste_invalid_type_error(destination_mode, &type_name));
            }
            PasteSourceInput::Components(values) => match values.as_slice() {
                [value] => paste_scalar_source(*value, destination_mode),
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
                _ => return Err(invalid_component_error(destination_mode)),
            },
            PasteSourceInput::String(value) => {
                paste_source_from_color_string(&value, destination_mode)?
            }
            PasteSourceInput::Invalid => {
                return Err(invalid_component_error(destination_mode));
            }
        };
        let (source, resolved_color) = match source {
            PasteSource::Image(image) => (
                PasteSource::Image(Box::new(resolve_image_source(
                    image,
                    destination_mode,
                    has_mask,
                )?)),
                None,
            ),
            solid => {
                let color = solid.solid_color(destination_mode)?;
                (solid, Some(color))
            }
        };
        Ok(PasteSourcePlan {
            destination_mode,
            source,
            resolved_color,
        })
    }

    /// Captures an image source whose Pillow mode conversion has already run.
    ///
    /// The Python adapter uses this after Pillow's virtual `Image.convert`
    /// call, or when Pillow keeps the original source core by reference. This
    /// captures the Python-selected raster before destination/mask callbacks.
    /// The returned source mode is retained as-is, including a mode-changing
    /// override result. This reproduces Pillow 12.3.0's observed, undocumented
    /// compatibility behavior: Paste admits matching physical pixel sizes;
    /// unmasked paths copy raw bytes and masked paths blend physical lanes.
    /// The parity input builder models an instance override that returns this
    /// same image and emits the RGBA→CMYK, RGB→RGBA, L→P, RGBA→L, and masked
    /// LA→RGBA cases. The active shared oracle is Pillow 12.2.0; the user has
    /// independently reproduced the same behavior on Pillow 12.3.0. Runtime
    /// parity remains gated on CPU source completion.
    #[doc(hidden)]
    pub fn prepare_resolved_paste_image_source(
        &self,
        image: Image,
    ) -> Result<PasteSourcePlan, PilError> {
        Ok(PasteSourcePlan {
            destination_mode: self.mode_identity()?,
            source: PasteSource::Image(Box::new(image)),
            resolved_color: None,
        })
    }

    /// Executes a Paste after box/source preparation and destination/mask loads.
    #[doc(hidden)]
    pub fn paste_prepared(
        &mut self,
        box_plan: PasteBoxPlan,
        source_plan: PasteSourcePlan,
        mask_input: PasteMaskInput,
    ) -> Result<(), PilError> {
        self.paste_prepared_with_box_mask(box_plan, source_plan, mask_input, None, true)
    }

    /// Applies a staged Paste with the already-loaded abbreviated box mask.
    #[doc(hidden)]
    pub fn paste_prepared_with_box_mask(
        &mut self,
        box_plan: PasteBoxPlan,
        source_plan: PasteSourcePlan,
        mask_input: PasteMaskInput,
        loaded_box_mask: Option<Box<Image>>,
        use_planned_box_mask: bool,
    ) -> Result<(), PilError> {
        let abbreviated_mask = match (
            box_plan.abbreviated_mask,
            loaded_box_mask,
            use_planned_box_mask,
        ) {
            (Some(mask), None, true) => Some(*mask),
            (Some(_), loaded, false) => loaded.map(|mask| *mask),
            (None, None, _) => None,
            (Some(_), Some(_), true) => {
                return Err(PilError::InternalError(
                    "planned Paste mask cannot be replaced and reused".into(),
                ));
            }
            (None, Some(_), _) => {
                return Err(PilError::InternalError(
                    "loaded abbreviated Paste mask has no box plan".into(),
                ));
            }
        };
        let mask = match (abbreviated_mask, mask_input) {
            (Some(mask), PasteMaskInput::None) => Some(mask),
            (Some(_), _) => {
                return Err(PilError::ValueError(
                    "If using second argument as mask, third argument must be None".to_owned(),
                ));
            }
            (None, PasteMaskInput::None) => None,
            (None, PasteMaskInput::Image(mask)) => Some(*mask),
            (None, PasteMaskInput::Invalid(type_name)) => {
                return Err(PilError::AttributeError(format!(
                    "'{type_name}' object has no attribute 'load'"
                )));
            }
        };
        let placement = match box_plan.input {
            PasteBoxInput::None | PasteBoxInput::Image(_) => PastePlacement::Position(0, 0),
            PasteBoxInput::Values(values) => match values.as_slice() {
                [x, y] => PastePlacement::Position(coordinate(*x)?, coordinate(*y)?),
                [left, top, right, bottom] => PastePlacement::Region(
                    coordinate(*left)?,
                    coordinate(*top)?,
                    coordinate(*right)?,
                    coordinate(*bottom)?,
                ),
                _ => {
                    return Err(PilError::InternalError(
                        "prepared Paste box changed after validation".into(),
                    ));
                }
            },
            PasteBoxInput::Invalid { .. } => {
                return Err(PilError::InternalError(
                    "prepared Paste box changed after validation".into(),
                ));
            }
        };
        self.paste_prepared_source(source_plan, placement, mask.as_ref())
    }

    fn paste_prepared_source(
        &mut self,
        plan: PasteSourcePlan,
        placement: PastePlacement,
        mask: Option<&Image>,
    ) -> Result<(), PilError> {
        let destination_mode = self.mode_identity()?;
        let source_size = match &plan.source {
            PasteSource::Image(image) => Some(image.size()?),
            _ => None,
        };
        let mask_size = mask.map(Image::size).transpose()?;
        let is_image = matches!(&plan.source, PasteSource::Image(_));
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
        let mask_alpha = paste_mask_uses_alpha(destination_mode, mask)?;
        let source_image = match plan.source {
            PasteSource::Image(image) => *image,
            _ => resolve_solid_source(
                plan.resolved_color.ok_or_else(|| {
                    PilError::InternalError("prepared solid Paste lost its color".into())
                })?,
                plan.destination_mode,
                width,
                height,
            )?,
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
        let source_width = i64::from(source_width);
        let source_height = i64::from(source_height);
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
        self.paste_impl(source, placement, mask, false)
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
        self.paste_impl(source, PastePlacement::Position(x, y), mask, false)
    }

    /// Queues a bitmap solid-color source as one pixel for native masked-Paste
    /// backends. The mask defines the full region; CPU fallback expands the
    /// sample before applying the ordinary Paste semantics.
    pub(crate) fn paste_bitmap_solid_masked_at(
        &mut self,
        source: PasteSource,
        position: (i32, i32),
        mask: &Image,
    ) -> Result<(), PilError> {
        if !matches!(
            mask.mode_identity()?,
            Mode::One | Mode::ImageLuma8
        ) {
            return Err(PilError::ValueError("bad transparency mask".to_owned()));
        }
        self.paste_impl(
            source,
            PastePlacement::Position(position.0, position.1),
            Some(mask),
            true,
        )
    }

    fn paste_impl(
        &mut self,
        source: PasteSource,
        placement: PastePlacement,
        mask: Option<&Image>,
        compact_solid_source: bool,
    ) -> Result<(), PilError> {
        let destination_mode = self.mode_identity()?;
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
        let mask_alpha = paste_mask_uses_alpha(destination_mode, mask)?;
        let (source_width, source_height) = if compact_solid_source {
            (1, 1)
        } else {
            (width, height)
        };
        let source_image = resolve_paste_source(
            source,
            destination_mode,
            source_width,
            source_height,
            mask.is_some(),
        )?;

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
        let dest_mode = self.mode_identity()?;
        let mode_support = crate::compute::registry::support::OperationId::AlphaComposite
            .support()
            .for_mode(dest_mode);
        mode_support.validate_mode()?;
        let source_mode = source.mode_identity()?;
        let mode_facts = crate::compute::registry::support::StrategyInputs {
            width: 0,
            height: 0,
            byte_len: None,
            palette: crate::compute::registry::support::PaletteKnowledge::Unknown,
            request: crate::compute::registry::support::RequestFacts::Unspecified,
        }
        .with_same_mode_secondary_image(
            crate::compute::registry::support::SecondaryInputFacts {
                mode: source_mode,
                width: 0,
                height: 0,
                byte_len: None,
            },
        );
        mode_support.assess_inputs(mode_facts)?;
        let (w1, h1) = self.size()?;
        let (w2, h2) = source.size()?;
        let facts = crate::compute::registry::support::StrategyInputs {
            width: w1,
            height: h1,
            byte_len: None,
            palette: crate::compute::registry::support::PaletteKnowledge::Unknown,
            request: crate::compute::registry::support::RequestFacts::Unspecified,
        }
        .with_same_mode_secondary_image(
            crate::compute::registry::support::SecondaryInputFacts {
                mode: source_mode,
                width: w2,
                height: h2,
                byte_len: None,
            },
        );
        if mode_support.assess_inputs(facts)?.is_some() {
            return Err(PilError::InternalError(
                "AlphaComposite input contract remained unresolved before queuing".into(),
            ));
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
            let degenerate = raw_width.is_none_or(|value| value == 0)
                || raw_height.is_none_or(|value| value == 0);
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

fn paste_mask_uses_alpha(destination_mode: Mode, mask: Option<&Image>) -> Result<bool, PilError> {
    let Some(mask_image) = mask else {
        return Ok(false);
    };
    OperationId::Paste
        .support()
        .for_mode(destination_mode)
        .paste_mask_uses_alpha(mask_image.mode_identity()?)
}

fn resolve_paste_source(
    source: PasteSource,
    destination_mode: Mode,
    width: u32,
    height: u32,
    has_mask: bool,
) -> Result<Image, PilError> {
    match source {
        PasteSource::Image(image) => resolve_image_source(image, destination_mode, has_mask),
        solid => {
            let color = solid.solid_color(destination_mode)?;
            resolve_solid_source(color, destination_mode, width, height)
        }
    }
}

fn resolve_image_source(
    image: Box<Image>,
    destination_mode: Mode,
    has_mask: bool,
) -> Result<Image, PilError> {
    let source_mode = image.mode_identity()?;
    // Pillow skips conversion for LA, RGBA, and RGBa pasted into RGB because
    // those modes already use the destination's four-byte physical pixel. Its
    // low-level Paste copies or blends these lanes and leaves RGBa premultiplied.
    if source_mode == destination_mode
        || (destination_mode == Mode::ImageRgb8
            && matches!(
                source_mode,
                Mode::ImageLumaA8 | Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8
            ))
    {
        Ok(*image)
    } else if destination_mode == Mode::ImageRgba8
        && source_mode == Mode::ImageRgb8
        && !has_mask
    {
        // Keep RGB source bytes native; the destination writer supplies opaque alpha.
        Ok(*image)
    } else if destination_mode == Mode::ImagePaletteAlpha8
        && source_mode == Mode::ImagePalette8
    {
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
    color: ResolvedPasteColor,
    destination_mode: Mode,
    width: u32,
    height: u32,
) -> Result<Image, PilError> {
    match color {
        ResolvedPasteColor::Bytes(color) if destination_mode == Mode::ImagePalette8 => {
            let dims = CheckedDims::new(width, height, 1)?;
            let mut indices = dims.alloc_buffer();
            indices.fill(color.0);
            Image::frombytes(crate::raster::Mode::ImagePalette8, (width, height), &indices)
        }
        ResolvedPasteColor::Bytes(color) if destination_mode == Mode::ImagePaletteAlpha8 => {
            // PA uses the LA physical layout while retaining its palette tag.
            Image::new(width, height, Mode::ImagePaletteAlpha8, color)
        }
        ResolvedPasteColor::Integer(value) => {
            let bytes = value.to_le_bytes();
            Image::new(width, height, crate::raster::Mode::ImageI32, (bytes[0], bytes[1], bytes[2], bytes[3]))
        }
        ResolvedPasteColor::Float(value) => {
            let bytes = value.to_le_bytes();
            Image::new(width, height, crate::raster::Mode::ImageF32, (bytes[0], bytes[1], bytes[2], bytes[3]))
        }
        ResolvedPasteColor::Luma16(sample) => {
            let pixels = crate::raster::ImageBuffer::from_pixel(
                width,
                height,
                crate::raster::Luma([sample]),
            );
            Image::from_dynamic_with_mode(
                crate::raster::DynamicImage::ImageI16(pixels),
                Some(destination_mode),
            )
        }
        ResolvedPasteColor::Bytes(color) => Image::new(width, height, destination_mode, color),
    }
}

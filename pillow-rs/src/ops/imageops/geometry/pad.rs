use super::{native_geometry_color_input, pad_containment_dimensions, parse_imageops_filter};
use crate::compute::registry::support::{InputContract, ModeSupport, OperationId, PadFillRule};
use crate::error::PilError;
use crate::image::Image;
use crate::ops::imageops::{
    resolve_imageops_color, resolve_pad_palette_fill, CenteringInput, ImageOpsColor,
};
use crate::ops::resize::{parse_resample, ResampleInput};
use crate::pipeline::{NativeColorInput, PipelineOp};

/// Contain geometry evaluated before Image.resize parses the filter.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct PadGeometryPreflight {
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    containment_dimensions: Option<(u32, u32)>,
    containment_axes: Option<(bool, bool)>,
}

/// Validated mode-independent inputs for the ImageOps.pad orchestration.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct PadPreflight {
    geometry: PadGeometryPreflight,
    filter: crate::pipeline::ResampleFilter,
    filter_was_none: bool,
    filter_code: Option<i64>,
}

impl PadPreflight {
    /// Returns the original Pillow code after validation, preserving `None`.
    pub fn normalized_filter_code(&self) -> Option<i64> {
        self.filter_code
    }

    /// Whether Pad will construct and fill a canvas after `contain`.
    pub fn needs_padding(&self) -> bool {
        self.geometry.containment_axes != Some((false, false))
    }

    /// Returns the contain axes on which Pillow will place padding.
    pub fn padded_axes(&self) -> (bool, bool) {
        self.geometry.containment_axes.unwrap_or((true, true))
    }

    /// Returns each output-axis gap left after contain sizing.
    pub fn padding_gaps(&self) -> (u32, u32) {
        let (content_width, content_height) = self.contained_size();
        let geometry = self.geometry;
        (
            geometry.width.saturating_sub(content_width),
            geometry.height.saturating_sub(content_height),
        )
    }

    /// Returns the exact dimensions produced by Pillow's contain step.
    pub fn contained_size(&self) -> (u32, u32) {
        self.geometry
            .containment_dimensions
            .unwrap_or((self.geometry.source_width, self.geometry.source_height))
    }

    /// Validates `Image.new`'s fill before Pillow indexes centering.
    pub fn validate_fill(&self, image: &Image, color: &ImageOpsColor) -> Result<(), PilError> {
        if !self.needs_padding() {
            return Ok(());
        }
        let _ = prepare_pad_fill(image, *self, color.clone())?;
        Ok(())
    }
}

/// Fill resolution completed before ImageOps.pad reads Python centering.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct PadFillPreflight {
    mode: crate::raster::Mode,
    color: Option<(u8, u8, u8, u8)>,
    native_color: Option<NativeColorInput>,
    output_palette: Option<(Vec<u8>, Option<Vec<u8>>)>,
    color_was_none: bool,
}

/// Resolves native and palette fills before a host centering callback runs.
#[doc(hidden)]
pub fn pad_fill_preflight(
    image: &Image,
    preflight: PadPreflight,
    color: ImageOpsColor,
) -> Result<PadFillPreflight, PilError> {
    prepare_pad_fill(image, preflight, color)
}

fn prepare_pad_fill(
    image: &Image,
    preflight: PadPreflight,
    color: ImageOpsColor,
) -> Result<PadFillPreflight, PilError> {
    let mode = image.mode_identity()?;
    if !preflight.needs_padding() {
        return Ok(PadFillPreflight {
            mode,
            color: None,
            native_color: None,
            output_palette: None,
            color_was_none: matches!(color, ImageOpsColor::None),
        });
    }
    let fill_rule = pad_fill_rule(mode)?;
    let native_color = match fill_rule {
        PadFillRule::NativeU16 | PadFillRule::NativeF32 => {
            native_geometry_color_input(&color)
        }
        PadFillRule::ModeEncodedBytes | PadFillRule::PaletteIndexOrColor => None,
    };
    let palette = resolve_pad_palette_fill(image, fill_rule, &color)?;
    let (resolved_color, output_palette) = match palette {
        Some((index, palette, alpha)) => (
            Some((index, index, index, u8::MAX)),
            Some((palette, alpha)),
        ),
        None => (resolve_imageops_color(color.clone(), mode)?, None),
    };
    Ok(PadFillPreflight {
        mode,
        color: resolved_color,
        native_color,
        output_palette,
        color_was_none: matches!(color, ImageOpsColor::None),
    })
}

/// Resolve `ImageOps.pad` centering only after `contain` has determined that
/// padding is needed.
///
/// Pillow indexes only the coordinate for the axis that receives padding. A
/// one-element sequence is therefore valid when width is padded, and extra
/// sequence values are ignored. This is deliberately separate from
/// `ImageOps.fit`, which unpacks both coordinates before doing any work.
fn resolve_pad_centering(
    input: CenteringInput,
    width_padded: bool,
    height_padded: bool,
) -> Result<(f64, f64), PilError> {
    match input {
        CenteringInput::Default => Ok((0.5, 0.5)),
        CenteringInput::Scalar(_) => Err(PilError::TypeError(
            "'float' object is not subscriptable".into(),
        )),
        CenteringInput::Invalid => Err(PilError::TypeError(
            "'NoneType' object is not subscriptable".into(),
        )),
        CenteringInput::PadPosition(_, _) => Err(PilError::InternalError(
            "resolved Pad position reached centering normalization".into(),
        )),
        CenteringInput::Values(values) => {
            let x = if width_padded {
                values
                    .first()
                    .copied()
                    .ok_or_else(|| PilError::IndexError("tuple index out of range".into()))?
            } else {
                0.5
            };
            let y = if height_padded {
                values
                    .get(1)
                    .copied()
                    .ok_or_else(|| PilError::IndexError("tuple index out of range".into()))?
            } else {
                0.5
            };
            Ok((x, y))
        }
    }
}

/// Computes `ImageOps.contain` geometry before `Image.resize` parses filter.
pub fn pad_geometry_preflight(
    image: &Image,
    w: u32,
    h: u32,
) -> Result<PadGeometryPreflight, PilError> {
    let (source_width, source_height) = image.size()?;
    let dimensions = pad_containment_dimensions(source_width, source_height, w, h);
    if source_height == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let containment_axes = dimensions.map(|(new_w, new_h)| (new_w != w, new_h != h));
    Ok(PadGeometryPreflight {
        width: w,
        height: h,
        source_width,
        source_height,
        containment_dimensions: dimensions,
        containment_axes,
    })
}

fn validate_pad_resize_dimensions(geometry: PadGeometryPreflight) -> Result<(), PilError> {
    let PadGeometryPreflight {
        width,
        height,
        source_width,
        source_height,
        containment_dimensions,
        ..
    } = geometry;
    if width == 0 && !(source_width == 0 && source_height == height) {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    if let Some((new_w, new_h)) = containment_dimensions {
        let zero_axis = new_w == 0 || new_h == 0;
        let zero_width_source_resize = source_width == 0 && new_h == source_height;
        if zero_axis && !zero_width_source_resize {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    }
    Ok(())
}

/// Parses the filter and validates the dimensions passed to Image.resize.
pub fn pad_filter_preflight(
    geometry: PadGeometryPreflight,
    filter: Option<ResampleInput>,
) -> Result<PadPreflight, PilError> {
    let filter_was_none = filter.is_none();
    let filter_code = match &filter {
        Some(ResampleInput::Code(code)) => Some(*code),
        Some(ResampleInput::Name(_)) | None => None,
    };
    let filter = parse_imageops_filter(filter)?;
    validate_pad_resize_dimensions(geometry)?;
    Ok(PadPreflight {
        geometry,
        filter,
        filter_was_none,
        filter_code,
    })
}

/// Validates contain geometry, then the filter and dimensions for Resize.
pub fn pad_preflight_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
) -> Result<PadPreflight, PilError> {
    let geometry = pad_geometry_preflight(image, w, h)?;
    pad_filter_preflight(geometry, filter)
}

/// Executes Pad using a previously validated filter and containment plan.
pub fn pad_with_preflight(
    image: &Image,
    preflight: PadPreflight,
    color: ImageOpsColor,
    centering: CenteringInput,
) -> Result<(Image, bool), PilError> {
    let fill = prepare_pad_fill(image, preflight, color)?;
    pad_with_preflight_and_fill(image, preflight, fill, centering)
}

/// Executes Pad after its fill has been resolved before host centering hooks.
#[doc(hidden)]
pub fn pad_with_preflight_and_fill(
    image: &Image,
    preflight: PadPreflight,
    fill: PadFillPreflight,
    centering: CenteringInput,
) -> Result<(Image, bool), PilError> {
    if let Some((contained, preflight)) = resize_after_pad_preflight_mutation(image, preflight)? {
        return pad_with_resolved_preflight(&contained, preflight, fill, centering);
    }
    pad_with_resolved_preflight(image, preflight, fill, centering)
}

/// Keeps Pillow's pre-coercion contain dimensions if a filter callback changed
/// the wrapped source before `Image.resize` receives it.
fn resize_after_pad_preflight_mutation(
    image: &Image,
    preflight: PadPreflight,
) -> Result<Option<(Image, PadPreflight)>, PilError> {
    let geometry = preflight.geometry;
    let planned_source_size = (geometry.source_width, geometry.source_height);
    if image.size()? == planned_source_size {
        return Ok(None);
    }

    let (width, height) = geometry.containment_dimensions.ok_or_else(|| {
        PilError::InternalError("Pad preflight omitted valid contain dimensions".into())
    })?;
    let mode = image.mode_identity()?;
    let filter = crate::compute::pool_cpu::ops::resize::ops::effective_filter(
        mode,
        preflight.filter,
    );
    let contained = Image::push_op(
        image,
        PipelineOp::Resize {
            w: width,
            h: height,
            filter,
        },
    );
    let preflight = PadPreflight {
        geometry: PadGeometryPreflight {
            source_width: width,
            source_height: height,
            ..geometry
        },
        ..preflight
    };
    Ok(Some((contained, preflight)))
}

fn pad_with_resolved_preflight(
    image: &Image,
    preflight: PadPreflight,
    fill: PadFillPreflight,
    centering: CenteringInput,
) -> Result<(Image, bool), PilError> {
    if image.mode_identity()? != fill.mode {
        return Err(PilError::InternalError(
            "Pad fill was prepared for a different image mode".into(),
        ));
    }
    let PadPreflight {
        geometry: planned_geometry,
        filter,
        filter_was_none,
        ..
    } = preflight;
    let PadGeometryPreflight {
        width: w,
        height: h,
        ..
    } = planned_geometry;
    // TODO(pillow-rs#3ptb): verify callback-driven source pixel/palette writes
    // and `getdata` identity in CPU parity.
    let containment_axes = planned_geometry.containment_axes;

    // Pillow's `pad` calls `contain` before touching either optional boundary
    // argument. If the resized image already has the requested dimensions,
    // malformed color/centering values are intentionally ignored.
    if containment_axes == Some((false, false)) {
        if (planned_geometry.source_width, planned_geometry.source_height) == (w, h) {
            // ImageOps.pad still returns an independent copy when the input
            // already has the requested size. Match Image.copy semantics
            // (including clearing source format metadata) without scheduling
            // a no-op Pad pass on CPU, SIMD, or GPU.
            return Ok((image.copy(), true));
        }
        return Ok((
            Image::push_op(
                image,
                PipelineOp::Pad {
                    w,
                    h,
                    filter,
                    color: None,
                    centering: (0.5, 0.5),
                },
            ),
            true,
        ));
    }

    let (width_padded, height_padded) = containment_axes.unwrap_or((true, true));
    let color = fill.color;
    // Image.new validates the fill before ImageOps.pad indexes centering.
    let (centering, position) = match centering {
        CenteringInput::PadPosition(x, y) => {
            let content_size = preflight.contained_size();
            validate_pad_paste_position((x, y), content_size)?;
            ((0.5, 0.5), Some((x, y)))
        }
        centering => (
            resolve_pad_centering(centering, width_padded, height_padded)?,
            None,
        ),
    };
    if position.is_none()
        && filter_was_none
        && fill.color_was_none
        && centering == (0.5, 0.5)
    {
        return pad(image, w, h, None, color, centering).map(|mut result| {
            result.suppress_pipeline_source_info();
            (result, false)
        });
    }
    let op = if let Some(position) = position {
        PipelineOp::PadPositioned {
            w,
            h,
            filter,
            color,
            native_color: fill.native_color,
            position,
        }
    } else {
        match fill.native_color {
            Some(input) => PipelineOp::PadTyped {
                w,
                h,
                filter,
                color,
                input,
                centering,
            },
            None => PipelineOp::Pad {
                w,
                h,
                filter,
                color,
                centering,
            },
        }
    };
    let mut output = Image::push_op(image, op);
    output.suppress_pipeline_source_info();
    if let Some((palette, alpha)) = fill.output_palette {
        output.set_pipeline_output_palette(Some(palette), alpha);
    }
    Ok((output, false))
}

fn validate_pad_paste_position(
    (x, y): (i32, i32),
    (width, height): (u32, u32),
) -> Result<(), PilError> {
    let right = i64::from(x) + i64::from(width);
    let lower = i64::from(y) + i64::from(height);
    if i32::try_from(right).is_err() || i32::try_from(lower).is_err() {
        return Err(PilError::OverflowError(
            "signed integer is greater than maximum".into(),
        ));
    }
    Ok(())
}

fn pad_fill_rule(mode: crate::raster::Mode) -> Result<PadFillRule, PilError> {
    let inputs = match OperationId::Pad.support().for_mode(mode) {
        ModeSupport::Supported { inputs, .. } => inputs,
        ModeSupport::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
    };
    inputs
        .iter()
        .find_map(|input| match input {
            InputContract::PadFill(rule) => Some(*rule),
            _ => None,
        })
        .ok_or_else(|| {
            PilError::InternalError("Pad support omitted its exact-mode fill rule".into())
        })
}

/// Resizes and pads an image to exactly `(w, h)`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn pad(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<&str>,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
) -> Result<Image, PilError> {
    let geometry = pad_geometry_preflight(image, w, h)?;
    let filter = parse_resample(filter)?;
    if w > 0 && h > 0 && image.size()? == (w, h) {
        // `ImageOps.pad` returns an independent copy when no resize or border
        // is needed. Skip a queued Pad pass so every backend avoids the
        // identity placement work, including callers below the Python facade.
        return Ok(image.copy());
    }
    let mut output = Image::push_op(
        image,
        PipelineOp::Pad {
            w,
            h,
            filter,
            color,
            centering,
        },
    );
    if geometry.containment_axes != Some((false, false)) {
        output.suppress_pipeline_source_info();
    }
    Ok(output)
}

/// `ImageOps.pad` with filter and centering validation owned by core.
pub fn pad_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
    color: ImageOpsColor,
    centering: CenteringInput,
) -> Result<Image, PilError> {
    pad_with_input_and_containment(image, w, h, filter, color, centering)
        .map(|(result, _is_contained)| result)
}

/// `ImageOps.pad` plus whether the result is its metadata-preserving contain
/// result or a newly created padded canvas.
pub fn pad_with_input_and_containment(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
    color: ImageOpsColor,
    centering: CenteringInput,
) -> Result<(Image, bool), PilError> {
    let preflight = pad_preflight_with_input(image, w, h, filter)?;
    pad_with_preflight(image, preflight, color, centering)
}

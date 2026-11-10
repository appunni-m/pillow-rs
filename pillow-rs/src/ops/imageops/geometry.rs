use super::*;
use crate::ops::resize::RESAMPLE_GUIDANCE;

mod callbacks;
mod fit_callbacks;
mod scale_callbacks;
pub use callbacks::{
    ImageOpsAspectArithmetic, ImageOpsAspectCallbacks, ImageOpsAspectComparison,
    ImageOpsAspectOperation, ImageOpsAspectRouteCallbacks, ImageOpsAspectSize,
    ImageOpsBorderCallbacks, ImageOpsCropCallbacks, ImageOpsCropRouteCallbacks,
    ImageOpsExpandCallbacks, ImageOpsExpandGeometryCallbacks, ImageOpsNativeSizeCallbacks,
    ImageOpsNativeSizeProperty, ImageOpsPadAxis, ImageOpsPadCallbacks,
    aspect_size_with_route_callbacks, crop_box_with_callbacks, crop_box_with_route_callbacks,
    native_size_pair_with_callbacks,
};
pub use fit_callbacks::{
    ImageOpsFitCallbacks, ImageOpsFitConstant, ImageOpsFitRouteCallbacks, fit_box_with_callbacks,
    fit_box_with_route_callbacks,
};
pub use scale_callbacks::{
    ImageOpsScaleCallbacks, ImageOpsScaleRouteCallbacks, scale_size_with_callbacks,
    scale_with_route_callbacks,
};

mod expand;
mod pad;
mod pad_route;
pub use expand::{
    PreparedExpandFill, expand, expand_canvas_with_prepared_fill, expand_with_callbacks,
    expand_with_input, expand_with_margins, expand_with_margins_and_input,
    finish_expand_fill_palette_from_host, prepare_expand_fill_for_mode,
    validate_expand_output_size_for_mode, validate_prepared_expand_output_size,
};
pub use pad::{
    PadFillPreflight, PadGeometryPreflight, PadPreflight, pad, pad_fallback_with_callbacks,
    pad_fill_preflight, pad_filter_preflight, pad_geometry_preflight, pad_preflight_with_input,
    pad_with_input, pad_with_input_and_containment, pad_with_preflight,
    pad_with_preflight_and_fill,
};
pub use pad_route::{ImageOpsPadRouteCallbacks, pad_with_route_callbacks};

type AspectDimensions = ((u32, u32), (u32, u32));

fn resolve_centering(input: CenteringInput) -> Result<(f64, f64), PilError> {
    match input {
        CenteringInput::Default => Ok((0.5, 0.5)),
        CenteringInput::Scalar(_) => Err(PilError::TypeError(
            "cannot unpack non-iterable float object".into(),
        )),
        CenteringInput::Values(values) if values.len() == 2 => Ok((
            normalize_fit_centering(values[0]),
            normalize_fit_centering(values[1]),
        )),
        CenteringInput::Values(values) if values.len() < 2 => Err(PilError::ValueError(format!(
            "not enough values to unpack (expected 2, got {})",
            values.len()
        ))),
        CenteringInput::Values(_) => Err(PilError::ValueError(
            "too many values to unpack (expected 2)".into(),
        )),
        CenteringInput::Invalid => Err(PilError::TypeError(
            "cannot unpack non-iterable NoneType object".into(),
        )),
        CenteringInput::PadPosition(_, _) => Err(PilError::TypeError(
            "cannot unpack non-iterable pad position object".into(),
        )),
    }
}

/// Pillow replaces each out-of-range `ImageOps.fit` centering coordinate with
/// `0.5` before computing its crop box. This also handles NaN and infinities,
/// for which Rust's floating-point `clamp` would otherwise preserve a value
/// that can make boxed coefficient spans invalid.
pub(crate) fn normalize_fit_centering(value: f64) -> f64 {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        value
    } else {
        0.5
    }
}

fn native_geometry_color_input(input: &ImageOpsColor) -> Option<crate::pipeline::NativeColorInput> {
    match input {
        ImageOpsColor::Scalar(value) => Some(crate::pipeline::NativeColorInput::scalar(*value)),
        ImageOpsColor::Components(values) => crate::pipeline::NativeColorInput::components(values),
        ImageOpsColor::TupleComponents(values) => {
            crate::pipeline::NativeColorInput::components(values)
        }
        ImageOpsColor::FloatingScalar(value) => {
            Some(crate::pipeline::NativeColorInput::floating_scalar(*value))
        }
        ImageOpsColor::FloatingComponents(values) => {
            crate::pipeline::NativeColorInput::floating_components(values)
        }
        ImageOpsColor::TupleComponentsWithFallback { fallback, .. } => {
            return native_geometry_color_input(fallback);
        }
        _ => None,
    }
}

/// Python's `round()` for the positive dimensions used by `ImageOps.contain`.
pub(crate) fn round_positive_ties_even(value: f64) -> u32 {
    let floor = value.floor();
    let fraction = value - floor;
    let rounded = if fraction < 0.5 {
        floor
    } else if fraction > 0.5 || (floor as u64) % 2 == 1 {
        floor + 1.0
    } else {
        floor
    };
    rounded.max(0.0).min(f64::from(u32::MAX)) as u32
}

/// Return the exact dimensions Pillow computes before its empty-image return
/// or `Image.resize` call.
fn pad_containment_dimensions(
    source_w: u32,
    source_h: u32,
    target_w: u32,
    target_h: u32,
) -> (u32, u32) {
    if source_h == 0 {
        return (target_w, 0);
    }
    if target_h == 0 {
        return (0, 0);
    }
    let source_ratio = f64::from(source_w) / f64::from(source_h);
    let destination_ratio = f64::from(target_w) / f64::from(target_h);
    if source_ratio == destination_ratio {
        (target_w, target_h)
    } else if source_ratio > destination_ratio {
        (
            target_w,
            round_positive_ties_even(
                f64::from(source_h) / f64::from(source_w) * f64::from(target_w),
            ),
        )
    } else {
        (
            round_positive_ties_even(
                f64::from(source_w) / f64::from(source_h) * f64::from(target_h),
            ),
            target_h,
        )
    }
}

/// Computes the contain result for a captured source and target size.
///
/// This is shared by the public ImageOps facade and prepared backend
/// strategies so aspect ratios and ties-to-even rounding have one owner.
pub(crate) fn contain_dimensions_from_sizes(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(u32, u32), PilError> {
    Ok(pad_containment_dimensions(
        source.0, source.1, target.0, target.1,
    ))
}

/// Pillow's `ImageOps.contain` returns `Image.new` instead of resizing when
/// either computed output dimension is zero.
pub(crate) const fn contain_returns_empty_image(dimensions: (u32, u32)) -> bool {
    dimensions.0 == 0 || dimensions.1 == 0
}

/// Return the dimensions that `ImageOps.contain` computes for its result.
fn contain_dimensions(image: &Image, w: u32, h: u32) -> Result<AspectDimensions, PilError> {
    let source = image.size()?;
    let dimensions = contain_dimensions_from_sizes(source, (w, h))?;
    Ok((source, dimensions))
}

/// Captures the output dimensions that `ImageOps.contain` computes before
/// `Image.resize` parses the filter.
#[doc(hidden)]
pub fn contain_geometry_preflight(image: &Image, w: u32, h: u32) -> Result<(u32, u32), PilError> {
    contain_dimensions(image, w, h).map(|(_, dimensions)| dimensions)
}

/// Validate the dimensions produced by `ImageOps.contain` or `ImageOps.cover`
/// before queuing the resize. Pillow's empty-width source is the one valid
/// zero-axis result: when its height is unchanged, the resize request equals
/// the source and `Image.resize` returns a copy without rejecting width zero.
pub(crate) fn validate_aspect_resize_dimensions(
    (source_w, source_h): (u32, u32),
    (new_w, new_h): (u32, u32),
) -> Result<(), PilError> {
    let empty_width_copy = source_w == 0 && source_h != 0 && new_w == 0 && new_h == source_h;
    if (new_w == 0 || new_h == 0) && !empty_width_copy {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(())
}

/// Return the dimensions that `ImageOps.cover` passes to `Image.resize`.
///
/// Unlike `contain`, a zero-width source can only reach the resize call when
/// the destination ratio is also zero. If the destination has positive width,
/// Pillow divides by the source width while calculating the covering height
/// and raises `ZeroDivisionError` before filter validation.
pub(crate) fn cover_dimensions_from_sizes(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(u32, u32), PilError> {
    let (w, h) = target;
    if source.1 == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let image_ratio = f64::from(source.0) / f64::from(source.1);
    let destination_ratio = f64::from(w) / f64::from(h);
    let dimensions = if image_ratio == destination_ratio {
        (w, h)
    } else if image_ratio < destination_ratio {
        if source.0 == 0 {
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        (
            w,
            round_positive_ties_even(f64::from(source.1) / f64::from(source.0) * f64::from(w)),
        )
    } else {
        (
            round_positive_ties_even(f64::from(source.0) / f64::from(source.1) * f64::from(h)),
            h,
        )
    };
    Ok(dimensions)
}

fn cover_dimensions(image: &Image, w: u32, h: u32) -> Result<AspectDimensions, PilError> {
    let source = image.size()?;
    let dimensions = cover_dimensions_from_sizes(source, (w, h))?;
    Ok((source, dimensions))
}

/// Captures the output dimensions that `ImageOps.cover` computes before
/// `Image.resize` parses the filter.
#[doc(hidden)]
pub fn cover_geometry_preflight(image: &Image, w: u32, h: u32) -> Result<(u32, u32), PilError> {
    cover_dimensions(image, w, h).map(|(_, dimensions)| dimensions)
}

fn parse_imageops_filter(
    input: Option<ResampleInput>,
) -> Result<crate::pipeline::ResampleFilter, PilError> {
    match input {
        // Pillow's ImageOps ``method=`` parameter does not accept a string;
        // the parity adapter only materializes enum names for ``resample=``.
        // Keep this distinction visible instead of making the four method
        // error cases accidentally succeed.
        Some(ResampleInput::Name(name)) => Err(PilError::ValueError(format!(
            "Unknown resampling filter ({name}). {RESAMPLE_GUIDANCE}"
        ))),
        input => parse_resample_input(input),
    }
}

/// Parses the strict filter value accepted by Pillow ImageOps wrappers.
#[doc(hidden)]
pub fn parse_imageops_filter_input(
    input: Option<ResampleInput>,
) -> Result<crate::pipeline::ResampleFilter, PilError> {
    parse_imageops_filter(input)
}

/// Validates the resampling argument accepted by `ImageOps.deform`.
///
/// Normalize the target facade's symbolic names and validate numeric values;
/// the current mesh backend uses its established nearest-neighbor sampling.
pub fn validate_deform_resample(input: Option<ResampleInput>) -> Result<(), PilError> {
    parse_resample_input(input).map(|_| ())
}
/// Resizes an image to fit within `(w, h)` while preserving aspect ratio.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn contain(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let (source, dimensions) = contain_dimensions(image, w, h)?;
    if contain_returns_empty_image(dimensions) {
        return Image::new_empty_with_mode(dimensions.0, dimensions.1, image.mode_identity()?);
    }
    let filter = parse_resample(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Contain { w, h, filter }))
}

/// `ImageOps.contain` with a host-neutral public filter value.
pub fn contain_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
) -> Result<Image, PilError> {
    if filter.is_none() {
        return contain(image, w, h, None);
    }
    let (source, dimensions) = contain_dimensions(image, w, h)?;
    if contain_returns_empty_image(dimensions) {
        return Image::new_empty_with_mode(dimensions.0, dimensions.1, image.mode_identity()?);
    }
    let filter = parse_imageops_filter(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Contain { w, h, filter }))
}

/// Resizes an image to cover `(w, h)` while preserving its aspect ratio.
///
/// The returned dimensions may exceed `(w, h)`; overflow is not cropped.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn cover(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let (source, dimensions) = cover_dimensions(image, w, h)?;
    let filter = parse_resample(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Cover { w, h, filter }))
}

/// `ImageOps.cover` with a host-neutral public filter value.
pub fn cover_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
) -> Result<Image, PilError> {
    if filter.is_none() {
        return cover(image, w, h, None);
    }
    let (source, dimensions) = cover_dimensions(image, w, h)?;
    let filter = parse_imageops_filter(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Cover { w, h, filter }))
}

/// Resizes and crops an image to exactly fit `(w, h)`.
///
/// `bleed` and `centering` follow Pillow `ImageOps.fit` semantics.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn fit(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<&str>,
    bleed: f64,
    centering: (f64, f64),
) -> Result<Image, PilError> {
    let bleed = normalize_fit_bleed(bleed);
    let centering = (
        normalize_fit_centering(centering.0),
        normalize_fit_centering(centering.1),
    );
    // Pillow computes both aspect ratios inside ImageOps.fit before calling
    // Image.resize (and therefore before Image.resize parses its filter).
    // Keep zero-height and zero-target errors at this eager boundary instead
    // of letting a deferred backend clamp them to one pixel.
    validate_fit_geometry(image, w, h, bleed)?;
    let filter = parse_resample(filter)?;
    validate_fit_resize_dimensions(image, w, h, bleed)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        },
    ))
}

/// `ImageOps.fit` with filter and centering validation owned by core.
pub fn fit_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
    bleed: f64,
    centering: CenteringInput,
) -> Result<Image, PilError> {
    let filter_was_none = filter.is_none();
    let centering = resolve_fit_centering(centering)?;
    let bleed = normalize_fit_bleed(bleed);
    // ImageOps.fit performs its centering unpack and crop-ratio divisions
    // before delegating to Image.resize. In particular, a zero source height
    // or target height must win over an invalid resampling value, while a
    // zero-width source can still produce a valid black positive result.
    validate_fit_geometry(image, w, h, bleed)?;
    let filter = parse_imageops_filter(filter)?;
    validate_fit_resize_dimensions(image, w, h, bleed)?;
    if filter_was_none && centering == (0.5, 0.5) {
        return fit(image, w, h, None, bleed, centering);
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        },
    ))
}

/// Validates `ImageOps.fit` centering and aspect geometry before host filter
/// coercion, matching the order in which Pillow delegates to `Image.resize`.
#[doc(hidden)]
pub fn fit_geometry_preflight(
    image: &Image,
    w: u32,
    h: u32,
    bleed: f64,
    centering: CenteringInput,
) -> Result<(f64, f64, f64, f64), PilError> {
    let centering = resolve_fit_centering(centering)?;
    let bleed = normalize_fit_bleed(bleed);
    validate_fit_geometry(image, w, h, bleed)?;
    Ok(fit_box_coordinates(image.size()?, (w, h), bleed, centering))
}

/// Computes Pillow `ImageOps.fit`'s crop box after its early geometry checks.
///
/// Kept outside the pixel path so the Python facade can retain this geometry
/// while converting the filter and then resize the wrapper's current image.
pub(crate) fn fit_box_coordinates(
    (source_width, source_height): (u32, u32),
    (width, height): (u32, u32),
    bleed: f64,
    centering: (f64, f64),
) -> (f64, f64, f64, f64) {
    let bleed_width = bleed * f64::from(source_width);
    let bleed_height = bleed * f64::from(source_height);
    let live_width = f64::from(source_width) - 2.0 * bleed_width;
    let live_height = f64::from(source_height) - 2.0 * bleed_height;
    let live_ratio = live_width / live_height;
    let target_ratio = f64::from(width) / f64::from(height);
    let (crop_width, crop_height) = if live_ratio == target_ratio {
        (live_width, live_height)
    } else if live_ratio >= target_ratio {
        (target_ratio * live_height, live_height)
    } else {
        (live_width, live_width / target_ratio)
    };
    let crop_left = bleed_width + (live_width - crop_width) * centering.0;
    let crop_top = bleed_height + (live_height - crop_height) * centering.1;
    (
        crop_left,
        crop_top,
        crop_left + crop_width,
        crop_top + crop_height,
    )
}

fn resolve_fit_centering(centering: CenteringInput) -> Result<(f64, f64), PilError> {
    // Pillow treats an explicit `(0.5, 0.5)` pair like the omitted default.
    // Keep the original filter-presence check separate so it still controls
    // the no-filter Fit fast path below.
    let centering = match centering {
        CenteringInput::Values(values)
            if values.len() == 2 && values[0] == 0.5 && values[1] == 0.5 =>
        {
            CenteringInput::Default
        }
        centering => centering,
    };
    resolve_centering(centering)
}

// Pillow's ImageOps.fit normalizes invalid bleed values before calculating the
// crop box. Keep this at the public core boundary so CPU, SIMD, and GPU receive
// the same deferred operation value instead of each reimplementing the rule.
pub(crate) fn normalize_fit_bleed(bleed: f64) -> f64 {
    if (0.0..0.5).contains(&bleed) {
        bleed
    } else {
        0.0
    }
}

/// Validate the geometry and error ordering performed by `ImageOps.fit` before
/// its lazy boxed resize is queued. Pillow evaluates the source/live aspect
/// ratio and output aspect ratio in Python before parsing the resize filter.
fn validate_fit_geometry(
    image: &Image,
    target_width: u32,
    target_height: u32,
    bleed: f64,
) -> Result<(), PilError> {
    let (source_width, source_height) = image.size()?;
    if source_height == 0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
    }
    let bleed_height = bleed * f64::from(source_height);
    let live_height = f64::from(source_height) - 2.0 * bleed_height;
    if live_height == 0.0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
    }
    // This is the first division in Pillow's fit implementation.
    let live_width = f64::from(source_width) - 2.0 * bleed * f64::from(source_width);
    let _live_ratio = live_width / live_height;
    // The output ratio is evaluated next, before Image.resize parses its
    // filter or validates a zero-width target.
    if target_height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let _output_ratio = f64::from(target_width) / f64::from(target_height);

    Ok(())
}

/// Validate the dimensions that Pillow's `Image.resize` checks after its
/// filter has been parsed. Keeping this separate from [`validate_fit_geometry`]
/// preserves the native precedence of an invalid filter over a zero-width
/// resize, while source/target height divisions still win earlier.
fn validate_fit_resize_dimensions(
    image: &Image,
    target_width: u32,
    target_height: u32,
    bleed: f64,
) -> Result<(), PilError> {
    let (source_width, source_height) = image.size()?;
    if target_width == 0 {
        // Image.resize can return an empty copy only when the source and
        // requested dimensions remain identical and no bleed changes the
        // boxed source height.
        let empty_width_copy = source_width == 0 && target_height == source_height && bleed == 0.0;
        if !empty_width_copy {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    } else if source_width == 0 && target_height < source_height {
        // A zero-width boxed source is accepted by Pillow only when the
        // positive destination has at least as many rows as the source.
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(())
}

/// Compute Pillow's ties-to-even target dimensions before public `resize`.
#[doc(hidden)]
pub fn scale_geometry_preflight(image: &Image, factor: f64) -> Result<(f64, f64), PilError> {
    if factor <= 0.0 {
        return Err(PilError::ValueError(
            "the factor must be greater than 0".to_owned(),
        ));
    }
    let (width, height) = image.size()?;
    scale_dimensions_preflight((width, height), factor)
}

/// Compute Pillow's ties-to-even Scale dimensions from a captured image size.
/// The prepared CPU path uses the same validation and rounding as the eager
/// facade before it selects a mode or architecture strategy.
pub(crate) fn scale_dimensions_preflight(
    (width, height): (u32, u32),
    factor: f64,
) -> Result<(f64, f64), PilError> {
    if factor.is_nan() {
        return Err(PilError::ValueError(
            "cannot convert float NaN to integer".to_owned(),
        ));
    }
    if factor <= 0.0 {
        return Err(PilError::ValueError(
            "the factor must be greater than 0".to_owned(),
        ));
    }
    if factor == 1.0 {
        return Ok((f64::from(width), f64::from(height)));
    }
    Ok((
        round_scaled_dimension(width, factor)?,
        round_scaled_dimension(height, factor)?,
    ))
}

fn round_scaled_dimension(dimension: u32, factor: f64) -> Result<f64, PilError> {
    let value = f64::from(dimension) * factor;
    if value.is_nan() {
        return Err(PilError::ValueError(
            "cannot convert float NaN to integer".to_owned(),
        ));
    }
    if value.is_infinite() {
        return Err(PilError::OverflowError(
            "cannot convert float infinity to integer".to_owned(),
        ));
    }
    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    };
    Ok(rounded)
}

pub(crate) fn scale_dimension_to_u32(value: f64) -> Result<u32, PilError> {
    if value > f64::from(u32::MAX) {
        return Err(PilError::OverflowError(
            "signed integer is greater than maximum".into(),
        ));
    }
    Ok(value as u32)
}

/// `ImageOps.scale` with the integer/enum resampling value exposed by Pillow.
pub fn scale_with_input(
    image: &Image,
    factor: f64,
    filter: Option<ResampleInput>,
) -> Result<Image, PilError> {
    // Pillow returns ``image.copy()`` before it inspects ``resample`` when
    // the factor is exactly one. Keep this fast path ahead of filter parsing,
    // including for empty images and invalid filter values.
    if factor == 1.0 {
        return Ok(image.copy());
    }
    if factor.is_nan() {
        return Err(PilError::ValueError(
            "cannot convert float NaN to integer".to_owned(),
        ));
    }
    if factor <= 0.0 {
        return Err(PilError::ValueError(
            "the factor must be greater than 0".to_owned(),
        ));
    }
    let (rounded_width, rounded_height) = scale_geometry_preflight(image, factor)?;
    let rounded_width = scale_dimension_to_u32(rounded_width)?;
    let rounded_height = scale_dimension_to_u32(rounded_height)?;

    // The input language records Image.Resampling enum members by name. The
    // Python source side materializes that name as an IntEnum; normalize the
    // equivalent target-facade representation here for this `resample=`
    // parameter only. ImageOps method parameters intentionally use the
    // stricter parser above.
    let filter = parse_resample_input(filter)?;

    // Pillow's resize accepts the all-zero image/result pair. Other rounded
    // zero dimensions still fail at the resize boundary; do not let the
    // backend's minimum-size clamp turn them into a one-pixel image.
    if rounded_width == 0 || rounded_height == 0 {
        let (width, height) = image.size()?;
        if width == 0 && height == 0 {
            return Ok(image.copy());
        }
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }

    Ok(Image::push_op(image, PipelineOp::Scale { factor, filter }))
}

/// Crops the same border width from every image edge.
///
/// # Errors
///
/// Returns Pillow's width-first or height-second crop-coordinate error when
/// the requested border is strictly larger than half of the corresponding
/// dimension.  Pillow validates the derived box in `ImageOps.crop` before it
/// returns a lazy image; doing the same here prevents an invalid request from
/// queueing a `CropBorder` operation and then failing only at materialization.
pub fn crop(image: &Image, border: u32) -> Result<Image, PilError> {
    crop_box_geometry_preflight(image, border)?;
    Ok(Image::push_op(image, PipelineOp::CropBorder { border }))
}

/// Return the same-border crop box used by `ImageOps.crop`.
///
/// This lets a host wrapper preserve public `Image.crop()` dispatch while
/// leaving ordinary integer geometry and reversed-coordinate errors in core.
#[doc(hidden)]
pub fn crop_box_geometry_preflight(
    image: &Image,
    border: u32,
) -> Result<(u32, u32, u32, u32), PilError> {
    let (width, height) = image.size()?;
    // Avoid `2 * border`, which could wrap for an oversized u32. Equality is
    // valid and produces an empty image; only a strictly larger border fails.
    if border > width / 2 {
        return Err(PilError::ValueError(
            "Coordinate 'right' is less than 'left'".into(),
        ));
    }
    if border > height / 2 {
        return Err(PilError::ValueError(
            "Coordinate 'lower' is less than 'upper'".into(),
        ));
    }
    Ok((border, border, width - border, height - border))
}

//! Pillow `ImageOps`-style module functions.
//!
//! Functions take [`crate::Image`] handles and return lazy result images where the
//! operation can be represented in the compute pipeline.

use crate::error::PilError;
use crate::image::Image;
use crate::ops::resize::{ResampleInput, parse_resample, parse_resample_input};
use crate::pipeline::PipelineOp;
use std::sync::Arc;

/// Host-neutral centering input for `ImageOps.fit` and `ImageOps.pad`.
#[derive(Debug, Clone)]
pub enum CenteringInput {
    /// Use Pillow's default `(0.5, 0.5)`.
    Default,
    /// A scalar supplied where a pair was expected.
    Scalar(f64),
    /// A sequence supplied by the caller.
    Values(Vec<f64>),
    /// A value that could not be represented as a numeric sequence.
    Invalid,
}

/// Host-neutral mask input for ImageOps functions.
#[derive(Debug, Clone)]
pub enum ImageOpsMask {
    /// No mask was supplied.
    None,
    /// A mask image extracted by a binding.
    Image(Image),
    /// A non-image value was supplied, preserving its host type name for the
    /// same attribute error Pillow raises when it calls ``mask.load()``.
    Invalid(String),
}

/// Host-neutral color input for `ImageOps.pad`.
#[derive(Debug, Clone)]
pub enum ImageOpsColor {
    /// No explicit color was supplied; use the operation default.
    None,
    /// A named or CSS-style color extracted from the host object.
    Name(String),
    /// A scalar color value extracted from the host object.
    Scalar(i64),
    /// A color component sequence extracted from the host object.
    Components(Vec<i64>),
    /// A value that was not a supported color representation.
    Invalid,
}

pub fn validate_imageops_mask(image: &Image, mask: ImageOpsMask) -> Result<(), PilError> {
    match mask {
        ImageOpsMask::None => Ok(()),
        ImageOpsMask::Invalid(type_name) => Err(PilError::AttributeError(format!(
            "'{type_name}' object has no attribute 'load'"
        ))),
        ImageOpsMask::Image(mask) => crate::ops::analysis::validate_transparency_mask(image, &mask),
    }
}

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
    }
}

/// Pillow replaces each out-of-range `ImageOps.fit` centering coordinate with
/// `0.5` before computing its crop box. This also handles NaN and infinities,
/// for which Rust's floating-point `clamp` would otherwise preserve a value
/// that can make boxed coefficient spans invalid.
fn normalize_fit_centering(value: f64) -> f64 {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        value
    } else {
        0.5
    }
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

/// Python's `round()` for the positive dimensions used by `ImageOps.contain`.
fn round_positive_ties_even(value: f64) -> u32 {
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

/// Return the exact contain dimensions Pillow computes before `Image.resize`.
/// A rounded axis of zero is intentionally preserved: Pillow passes that
/// result to `resize`, which raises `ValueError("height and width must be > 0")`
/// rather than silently clamping the axis to one.
fn pad_containment_dimensions(
    source_w: u32,
    source_h: u32,
    target_w: u32,
    target_h: u32,
) -> Option<(u32, u32)> {
    if source_h == 0 || target_h == 0 {
        return None;
    }
    let source_ratio = f64::from(source_w) / f64::from(source_h);
    let destination_ratio = f64::from(target_w) / f64::from(target_h);
    if (source_ratio - destination_ratio).abs() < 1e-10 {
        Some((target_w, target_h))
    } else if source_ratio > destination_ratio {
        Some((
            target_w,
            round_positive_ties_even(
                f64::from(source_h) / f64::from(source_w) * f64::from(target_w),
            ),
        ))
    } else {
        Some((
            round_positive_ties_even(
                f64::from(source_w) / f64::from(source_h) * f64::from(target_h),
            ),
            target_h,
        ))
    }
}

/// Return the dimensions that `ImageOps.contain` passes to `Image.resize`.
///
/// Pillow evaluates both aspect-ratio divisions in `ImageOps.contain` before
/// it calls `Image.resize`, so zero source/target heights must be reported as
/// `ZeroDivisionError` at this public boundary.  Rounded-zero output axes are
/// deliberately returned for the later resize validation step: Pillow parses
/// the resampling filter before `Image.resize` rejects those dimensions.
fn contain_dimensions(image: &Image, w: u32, h: u32) -> Result<((u32, u32), (u32, u32)), PilError> {
    let source = image.size()?;
    if source.1 == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let dimensions = pad_containment_dimensions(source.0, source.1, w, h)
        .expect("non-zero contain heights were validated above");
    Ok((source, dimensions))
}

/// Validate the dimensions produced by `ImageOps.contain` or `ImageOps.cover`
/// before queuing the resize. Pillow's empty-width source is the one valid
/// zero-axis result: when its height is unchanged, the resize request equals
/// the source and `Image.resize` returns a copy without rejecting width zero.
fn validate_aspect_resize_dimensions(
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
fn cover_dimensions(image: &Image, w: u32, h: u32) -> Result<((u32, u32), (u32, u32)), PilError> {
    let source = image.size()?;
    if source.1 == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let image_ratio = f64::from(source.0) / f64::from(source.1);
    let destination_ratio = f64::from(w) / f64::from(h);
    let dimensions = if (image_ratio - destination_ratio).abs() < 1e-10 {
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
    Ok((source, dimensions))
}

/// Return which axes `ImageOps.pad` will fill after its `contain` step.
/// `None` preserves deferred error handling for zero-sized inputs, where
/// Pillow evaluates the aspect-ratio division before it can inspect color or
/// centering.
fn pad_containment_axes(image: &Image, w: u32, h: u32) -> Result<Option<(bool, bool)>, PilError> {
    let (iw, ih) = image.size()?;
    Ok(pad_containment_dimensions(iw, ih, w, h).map(|(new_w, new_h)| (new_w != w, new_h != h)))
}

pub(crate) fn resolve_imageops_color(
    input: ImageOpsColor,
    mode: &str,
) -> Result<Option<(u8, u8, u8, u8)>, PilError> {
    fn clamp(value: i64) -> u8 {
        value.clamp(0, i64::from(u8::MAX)) as u8
    }

    fn is_luma_mode(mode: &str) -> bool {
        matches!(
            mode,
            "1" | "L" | "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N"
        )
    }

    fn is_alpha_mode(mode: &str) -> bool {
        matches!(mode, "LA" | "RGBA" | "PA")
    }

    fn invalid_color(mode: &str) -> PilError {
        if mode == "F" {
            PilError::TypeError("must be real number, not tuple".into())
        } else if is_luma_mode(mode) {
            PilError::TypeError("color must be int or single-element tuple".into())
        } else if mode == "LA" {
            PilError::TypeError("color must be int, or tuple of one or two elements".into())
        } else {
            PilError::TypeError("color must be int, or tuple of one, three or four elements".into())
        }
    }

    fn scalar(value: i64, mode: &str) -> (u8, u8, u8, u8) {
        if mode == "F" {
            // Pillow's Image.new/ImageOps.pad keep F samples in their native
            // four-byte scalar representation. The SIMD pad adapter treats
            // these bytes as the final pixel, so repeated grayscale bytes
            // would turn a valid scalar fill into a bogus RGBA sample.
            let [a, b, c, d] = (value as f32).to_le_bytes();
            return (a, b, c, d);
        }
        if mode == "I" {
            // Pillow stores an I-mode fill as one signed little-endian int32
            // sample. ImageOps.pad carries the sample through the four-byte
            // RGBA-compatible pipeline storage, so repeating the low byte in
            // all channels would produce a different integer at materialize.
            let [a, b, c, d] = (value as i32).to_le_bytes();
            return (a, b, c, d);
        }
        let value = clamp(value);
        if is_luma_mode(mode) {
            return (value, value, value, u8::MAX);
        }
        if mode == "LA" {
            return (value, value, value, 0);
        }
        if mode == "P" {
            return (value, 0, 0, u8::MAX);
        }
        (value, 0, 0, if is_alpha_mode(mode) { 0 } else { u8::MAX })
    }

    fn color_value(value: crate::color::ColorValue, mode: &str) -> (u8, u8, u8, u8) {
        match value {
            crate::color::ColorValue::Gray(value) => scalar(i64::from(value), mode),
            crate::color::ColorValue::GrayAlpha(value, alpha) => (
                clamp(i64::from(value)),
                clamp(i64::from(value)),
                clamp(i64::from(value)),
                clamp(i64::from(alpha)),
            ),
            crate::color::ColorValue::Rgb(r, g, b) => (
                clamp(i64::from(r)),
                clamp(i64::from(g)),
                clamp(i64::from(b)),
                u8::MAX,
            ),
            crate::color::ColorValue::Rgba(r, g, b, a) => (
                clamp(i64::from(r)),
                clamp(i64::from(g)),
                clamp(i64::from(b)),
                clamp(i64::from(a)),
            ),
            crate::color::ColorValue::Hsv(h, s, v) => (
                clamp(i64::from(h)),
                clamp(i64::from(s)),
                clamp(i64::from(v)),
                u8::MAX,
            ),
        }
    }

    match input {
        ImageOpsColor::None => Ok(None),
        ImageOpsColor::Invalid => Err(PilError::TypeError("color must be int or tuple".into())),
        ImageOpsColor::Name(name) => {
            let (r, g, b, a) = crate::color::parse_color_str(&name)?;
            if mode == "P" {
                // Pillow's Image.new("P", ..., tuple_or_name) creates a
                // temporary palette entry, then ImageOps.pad pastes into the
                // source palette without copying that entry. The fill stays
                // at palette index zero; only scalar colors are raw indices.
                return Ok(Some((0, 0, 0, u8::MAX)));
            }
            let value = crate::color::getcolor(
                i32::from(r),
                i32::from(g),
                i32::from(b),
                i32::from(a),
                mode,
            )?;
            Ok(Some(color_value(value, mode)))
        }
        ImageOpsColor::Scalar(value) => Ok(Some(scalar(value, mode))),
        ImageOpsColor::Components(values) => match values.as_slice() {
            [value] => Ok(Some(scalar(*value, mode))),
            [_, _, _] if mode == "P" => Ok(Some((0, 0, 0, u8::MAX))),
            [_, _, _, alpha] if mode == "P" && *alpha == i64::from(u8::MAX) => {
                Ok(Some((0, 0, 0, u8::MAX)))
            }
            [_, _, _, _] if mode == "P" => Err(PilError::ValueError(
                "cannot add non-opaque RGBA color to RGB palette".into(),
            )),
            [value, alpha] if matches!(mode, "LA" | "PA") => Ok(Some((
                clamp(*value),
                clamp(*value),
                clamp(*value),
                clamp(*alpha),
            ))),
            [r, g, b] | [r, g, b, _] if !is_luma_mode(mode) && mode != "LA" => Ok(Some((
                clamp(*r),
                clamp(*g),
                clamp(*b),
                if mode == "RGBA" && values.len() == 4 {
                    clamp(values[3])
                } else {
                    u8::MAX
                },
            ))),
            _ => Err(invalid_color(mode)),
        },
    }
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
            "Unknown resampling filter ({name}). Use Image.Resampling.NEAREST (0), \
             Image.Resampling.LANCZOS (1), Image.Resampling.BILINEAR (2), \
             Image.Resampling.BICUBIC (3), Image.Resampling.BOX (4) or \
             Image.Resampling.HAMMING (5)"
        ))),
        input => parse_resample_input(input),
    }
}

/// Validates the resampling argument accepted by `ImageOps.deform`.
///
/// Normalize the target facade's symbolic names and validate numeric values;
/// the current mesh backend uses its established nearest-neighbor sampling.
pub fn validate_deform_resample(input: Option<ResampleInput>) -> Result<(), PilError> {
    parse_resample_input(input).map(|_| ())
}

fn validate_autocontrast_mode(image: &Image) -> Result<(), PilError> {
    let mode = image.mode()?;
    // Pillow 12.2.0 `ImageOps._lut` accepts only "L" and "RGB"; "P" raises
    // NotImplementedError and every other mode raises the OSError below.
    if mode == "P" {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    if mode != "L" && mode != "RGB" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(())
}

/// Normalizes image contrast by clipping darkest and lightest values.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
pub fn autocontrast(image: &Image, cutoff: f64) -> Result<Image, PilError> {
    validate_autocontrast_mode(image)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Autocontrast { cutoff, mask: None },
    ))
}

/// Normalizes contrast after validating an optional Pillow mask.
pub fn autocontrast_with_mask(
    image: &Image,
    cutoff: f64,
    mask: ImageOpsMask,
) -> Result<Image, PilError> {
    match mask {
        ImageOpsMask::None => autocontrast(image, cutoff),
        ImageOpsMask::Invalid(type_name) => {
            validate_imageops_mask(image, ImageOpsMask::Invalid(type_name))
                .and_then(|_| autocontrast(image, cutoff))
        }
        ImageOpsMask::Image(mask) => {
            validate_imageops_mask(image, ImageOpsMask::Image(mask.clone()))?;
            validate_autocontrast_mode(image)?;
            Ok(Image::push_op(
                image,
                PipelineOp::Autocontrast {
                    cutoff,
                    mask: Some(Arc::new(mask)),
                },
            ))
        }
    }
}

/// Equalizes the image histogram.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
pub fn equalize(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;
    // Pillow 12.2.0 converts "P" to "RGB" before building the LUT and then
    // `_lut` accepts only "L" and "RGB"; all other modes raise the OSError.
    if mode != "L" && mode != "RGB" && mode != "P" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Equalize))
}

/// Equalizes an image after validating an optional Pillow mask.
pub fn equalize_with_mask(image: &Image, mask: ImageOpsMask) -> Result<Image, PilError> {
    if matches!(&mask, ImageOpsMask::None) {
        return equalize(image);
    }
    validate_imageops_mask(image, mask)?;
    equalize(image)
}

/// Inverts all pixel values.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
pub fn invert(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;
    // Pillow's ImageOps._lut path rejects CMYK before creating the result.
    // Keep this separate from ImageChops.invert, which has a different mode
    // contract and is implemented by the chops module.
    if mode == "CMYK" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    if matches!(mode.as_str(), "LA" | "PA" | "RGBA") {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Invert))
}

/// Inverts through the `ImageOps.invert` compatibility path.
///
/// Unlike `ImageChops.invert`, Pillow raises for `P` mode here.
///
/// # Errors
///
/// Returns [`PilError::NotImplementedError`] for `P` mode, or errors from
/// [`invert`].
pub fn invert_ops(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "P" {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    invert(image)
}

/// Flips an image vertically.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn flip(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Flip))
}

/// Mirrors an image horizontally.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn mirror(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Mirror))
}

/// Reduces the number of stored high bits per color channel.
///
/// `bits` is clamped to `1..=8`.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
pub fn posterize(image: &Image, bits: u8) -> Result<Image, PilError> {
    let mode = image.mode()?;
    // ImageOps.posterize delegates to `_lut`, whose public contract is the
    // same as autocontrast: P is a named unsupported path and all other modes
    // outside L/RGB raise OSError before a pipeline is created.
    if mode == "P" {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    if mode != "L" && mode != "RGB" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Posterize {
            bits: bits.clamp(1, 8),
        },
    ))
}

/// Inverts pixel values at or above `threshold`.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
pub fn solarize(image: &Image, threshold: u8) -> Result<Image, PilError> {
    let mode = image.mode()?;
    // Pillow 12.2.0 exposes P as an explicit unsupported ImageOps._lut path;
    // this must be raised at the call, before deferred pipeline execution.
    if mode == "P" {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    if matches!(mode.as_str(), "LA" | "RGBA" | "CMYK") {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Solarize { threshold }))
}

/// Converts an image to grayscale using Pillow-compatible BT.601 luma.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn grayscale(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Grayscale))
}

/// Validates that an image can be colorized.
///
/// # Errors
///
/// Returns [`PilError::AssertionError`] when the source mode is not `"L"`, or
/// another [`PilError`] when mode detection fails.
pub fn validate_colorize_mode(image: &Image) -> Result<(), PilError> {
    let mode = image.mode()?;
    if mode != "L" {
        // PIL raises AssertionError for non-L modes before resolving colors.
        return Err(PilError::AssertionError(String::new()));
    }
    Ok(())
}

/// Colorizes an `L` image by mapping black and white endpoints.
///
/// # Errors
///
/// Returns [`PilError::AssertionError`] when the source mode is not `"L"`, or
/// another [`PilError`] when mode detection fails.
pub fn colorize(
    image: &Image,
    black: (u8, u8, u8),
    white: (u8, u8, u8),
    mid: Option<(u8, u8, u8)>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
) -> Result<Image, PilError> {
    validate_colorize_mode(image)?;
    if let Some(_) = mid {
        // PIL: assert 0 <= blackpoint <= midpoint <= whitepoint <= 255
        if !(blackpoint <= midpoint && midpoint <= whitepoint) {
            return Err(PilError::AssertionError(String::new()));
        }
    } else if blackpoint > whitepoint {
        // PIL: assert 0 <= blackpoint <= whitepoint <= 255
        return Err(PilError::AssertionError(String::new()));
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Colorize {
            black,
            white,
            mid,
            blackpoint,
            midpoint,
            whitepoint,
        },
    ))
}

/// Adds a border around an image.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn expand(image: &Image, border: u32, fill: (u8, u8, u8, u8)) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Expand { border, fill }))
}

/// Resizes an image to fit within `(w, h)` while preserving aspect ratio.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn contain(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let (source, dimensions) = contain_dimensions(image, w, h)?;
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
    let filter = parse_imageops_filter(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Contain { w, h, filter }))
}

/// Resizes an image to cover `(w, h)`, cropping overflow.
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
    // Pillow treats an explicit `(0.5, 0.5)` pair exactly like the omitted
    // default, but it still preserves an explicitly supplied resampling
    // method. Normalize the centering in core so that the default path cannot
    // discard that method before building the pipeline operation.
    let centering = match centering {
        CenteringInput::Values(values)
            if values.len() == 2 && values[0] == 0.5 && values[1] == 0.5 =>
        {
            CenteringInput::Default
        }
        centering => centering,
    };
    let centering = resolve_centering(centering)?;
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

// Pillow's ImageOps.fit normalizes invalid bleed values before calculating the
// crop box. Keep this at the public core boundary so CPU, SIMD, and GPU receive
// the same deferred operation value instead of each reimplementing the rule.
fn normalize_fit_bleed(bleed: f64) -> f64 {
    if 0.0 <= bleed && bleed < 0.5 {
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
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Pad {
            w,
            h,
            filter,
            color,
            centering,
        },
    ))
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
    let filter_was_none = filter.is_none();
    let color_was_none = matches!(&color, ImageOpsColor::None);
    let filter = parse_imageops_filter(filter)?;

    // Pillow's `pad` calls `contain` before touching either optional boundary
    // argument. If the resized image already has the requested dimensions,
    // malformed color/centering values are intentionally ignored.
    let containment_axes = pad_containment_axes(image, w, h)?;
    // Pillow evaluates ``contain`` before constructing the padded canvas. A
    // zero-height source or destination therefore raises division by zero at
    // the public call boundary. A zero-width destination normally reaches
    // ``Image.new`` and raises its non-positive-dimension ValueError, except
    // when an empty-width source keeps the same height: that contain result
    // remains empty and can be padded. Keep these checks after
    // ``pad_containment_axes`` so the aspect-ratio guard remains exercised by
    // valid public zero-dimension inputs.
    let (_, source_height) = image.size()?;
    if source_height == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let (source_width, _) = image.size()?;
    if w == 0 && !(source_width == 0 && source_height == h) {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    if let Some((new_w, new_h)) = pad_containment_dimensions(source_width, source_height, w, h) {
        let zero_axis = new_w == 0 || new_h == 0;
        let zero_width_source_resize = source_width == 0 && new_h == source_height;
        if zero_axis && !zero_width_source_resize {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    }
    if containment_axes == Some((false, false)) {
        return Ok(Image::push_op(
            image,
            PipelineOp::Pad {
                w,
                h,
                filter,
                color: None,
                centering: (0.5, 0.5),
            },
        ));
    }

    let (width_padded, height_padded) = containment_axes.unwrap_or((true, true));
    let centering = resolve_pad_centering(centering, width_padded, height_padded)?;
    let color = resolve_imageops_color(color, &image.mode()?)?;
    if filter_was_none && color_was_none && centering == (0.5, 0.5) {
        return pad(image, w, h, None, color, centering);
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Pad {
            w,
            h,
            filter,
            color,
            centering,
        },
    ))
}

/// `ImageOps.scale` with the integer/enum resampling value exposed by Pillow.
pub fn scale_with_input(
    image: &Image,
    factor: f64,
    filter: Option<ResampleInput>,
) -> Result<Image, PilError> {
    // Pillow's ``factor <= 0`` check intentionally does not catch NaN.  The
    // later Python ``round`` then reports the conversion error for NaN, while
    // positive infinity reports its distinct overflow error. Preserve those
    // public distinctions before constructing the lazy resize.
    if factor.is_nan() {
        return Err(PilError::ValueError(
            "cannot convert float NaN to integer".to_owned(),
        ));
    }
    // Pillow returns ``image.copy()`` before it inspects ``resample`` when
    // the factor is exactly one. Keep this fast path ahead of filter parsing,
    // including for empty images and invalid filter values.
    if factor == 1.0 {
        return Ok(image.copy());
    }
    if factor <= 0.0 {
        return Err(PilError::ValueError(
            "the factor must be greater than 0".to_owned(),
        ));
    }

    // ImageOps.scale computes dimensions with Python's ties-to-even round and
    // immediately delegates to Image.resize.  Resize rejects a rounded zero;
    // do that check here so the lazy target has the same public call boundary
    // as Pillow instead of allowing its backend's minimum-size clamp to turn
    // the request into a 1x1 image.
    let (width, height) = image.size()?;
    let rounded_dimension = |dimension: u32| {
        let value = f64::from(dimension) * factor;
        // ``round(inf)`` and ``round(nan)`` fail before Pillow delegates to
        // Image.resize. In particular, ``inf * 0`` is NaN, so an empty image
        // receives the NaN conversion error rather than the infinity one.
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
        if rounded > f64::from(u32::MAX) {
            Err(PilError::OverflowError(
                "signed integer is greater than maximum".into(),
            ))
        } else {
            Ok(rounded as u32)
        }
    };
    let rounded_width = rounded_dimension(width)?;
    let rounded_height = rounded_dimension(height)?;

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
    let (width, height) = image.size()?;
    // Use division instead of `2 * border` so an oversized u32 border cannot
    // wrap before the comparison.  Equality is valid and produces Pillow's
    // empty image; only a strictly larger border is rejected.
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
    Ok(Image::push_op(image, PipelineOp::CropBorder { border }))
}

/// Applies the EXIF orientation transform used by `ImageOps.exif_transpose`.
///
/// The returned image is `Some` for a transformed image, or for a copy when
/// `in_place` is false and no valid orientation is present. In-place
/// replacement is performed by the binding after this function returns.
pub fn exif_transpose(image: &Image, in_place: bool) -> Result<Option<Image>, PilError> {
    let raw_exif = image.getexif();
    // Pillow maps EXIF's eight orientation values onto these named transpose
    // operations; the rotation directions are intentionally asymmetric.
    let method = match exif_get_orientation(&raw_exif).unwrap_or(1) {
        2 => Some("FLIP_LEFT_RIGHT"),
        3 => Some("ROTATE_180"),
        4 => Some("FLIP_TOP_BOTTOM"),
        5 => Some("TRANSPOSE"),
        6 => Some("ROTATE_270"),
        7 => Some("TRANSVERSE"),
        8 => Some("ROTATE_90"),
        _ => None,
    };

    match method {
        Some(method) => {
            let exif = exif_remove_orientation(&raw_exif);
            image
                .transpose(method)?
                .with_exif_metadata(Some(exif))
                .map(Some)
        }
        None if in_place => Ok(None),
        None => Ok(Some(image.copy())),
    }
}

/// Extract Orientation tag (0x0112) from raw EXIF bytes. Returns None if not found.
///
/// Scans TIFF IFD0 entries looking for tag 0x0112. Handles both TIFF and
/// Exif-JPEG (starts with "Exif\0\0") formats. Supports little-endian (II)
/// and big-endian (MM) byte orders.
pub fn exif_get_orientation(raw: &[u8]) -> Option<u32> {
    if raw.is_empty() || raw.len() < 8 {
        return None;
    }

    // Skip EXIF header if present (Exif-JPEG format)
    let data = if raw.starts_with(b"Exif\x00\x00") {
        &raw[6..]
    } else {
        raw
    };

    if data.len() < 8 {
        return None;
    }

    // Determine byte order
    let le = match &data[..2] {
        b"II" => true,  // Little-endian
        b"MM" => false, // Big-endian
        _ => return None,
    };

    // Check TIFF magic number (42)
    let magic = if le {
        u16::from_le_bytes([data[2], data[3]])
    } else {
        u16::from_be_bytes([data[2], data[3]])
    };
    if magic != 42 {
        return None;
    }

    // Get IFD0 offset
    let ifd_offset = if le {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]])
    } else {
        u32::from_be_bytes([data[4], data[5], data[6], data[7]])
    } as usize;

    if ifd_offset + 2 > data.len() {
        return None;
    }

    // Number of IFD entries
    let num_entries = if le {
        u16::from_le_bytes([data[ifd_offset], data[ifd_offset + 1]])
    } else {
        u16::from_be_bytes([data[ifd_offset], data[ifd_offset + 1]])
    } as usize;

    // Scan IFD entries for Orientation tag (0x0112)
    for i in 0..num_entries {
        let entry_start = ifd_offset + 2 + i * 12;
        if entry_start + 12 > data.len() {
            break;
        }
        let tag = if le {
            u16::from_le_bytes([data[entry_start], data[entry_start + 1]])
        } else {
            u16::from_be_bytes([data[entry_start], data[entry_start + 1]])
        };
        if tag == 0x0112 {
            // Orientation value is at entry_start + 8, SHORT type (2 bytes)
            let value = if le {
                u16::from_le_bytes([data[entry_start + 8], data[entry_start + 9]])
            } else {
                u16::from_be_bytes([data[entry_start + 8], data[entry_start + 9]])
            };
            if (1..=8).contains(&value) {
                return Some(value as u32);
            }
            return None;
        }
    }

    None
}

/// Remove IFD0's Orientation entry after the pixels have been transposed.
///
/// Pillow's `ImageOps.exif_transpose` serializes the retained Exif mapping
/// again, which removes tag `0x0112` while retaining unrelated IFD0 entries.
/// The fixture surface currently uses inline SHORT values; preserve the
/// remainder of the TIFF payload byte-for-byte so opaque metadata remains
/// untouched.
pub fn exif_remove_orientation(raw: &[u8]) -> Vec<u8> {
    let prefix_len = if raw.starts_with(b"Exif\x00\x00") {
        6
    } else {
        0
    };
    let data = raw.get(prefix_len..).unwrap_or_default();
    if data.len() < 8 {
        return raw.to_vec();
    }
    let le = match &data[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return raw.to_vec(),
    };
    let read_u16 = |bytes: &[u8]| {
        if le {
            u16::from_le_bytes([bytes[0], bytes[1]])
        } else {
            u16::from_be_bytes([bytes[0], bytes[1]])
        }
    };
    let read_u32 = |bytes: &[u8]| {
        if le {
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        } else {
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        }
    };
    if read_u16(&data[2..4]) != 42 {
        return raw.to_vec();
    }
    let Some(ifd_offset) = usize::try_from(read_u32(&data[4..8])).ok() else {
        return raw.to_vec();
    };
    let Some(entry_count_end) = ifd_offset.checked_add(2) else {
        return raw.to_vec();
    };
    let Some(entry_count_bytes) = data.get(ifd_offset..entry_count_end) else {
        return raw.to_vec();
    };
    let entry_count = usize::from(read_u16(entry_count_bytes));
    let Some(entries_len) = entry_count.checked_mul(12) else {
        return raw.to_vec();
    };
    let Some(entries_end) = entry_count_end.checked_add(entries_len) else {
        return raw.to_vec();
    };
    let retain_entries = |entries: &[u8]| {
        entries
            .chunks_exact(12)
            .filter(|entry| read_u16(&entry[..2]) != 0x0112)
            .map(|entry| {
                let mut entry = entry.to_vec();
                // Pillow's Exif serializer emits ImageWidth's integer value
                // as a LONG even when the source IFD used an inline SHORT.
                if read_u16(&entry[..2]) == 0x0100
                    && read_u16(&entry[2..4]) == 3
                    && read_u32(&entry[4..8]) == 1
                {
                    let value = u32::from(read_u16(&entry[8..10]));
                    let type_bytes = if le {
                        4u16.to_le_bytes()
                    } else {
                        4u16.to_be_bytes()
                    };
                    entry[2..4].copy_from_slice(&type_bytes);
                    let value_bytes = if le {
                        value.to_le_bytes()
                    } else {
                        value.to_be_bytes()
                    };
                    entry[8..12].copy_from_slice(&value_bytes);
                }
                entry
            })
            .collect::<Vec<_>>()
    };
    let Some(entries) = data.get(entry_count_end..entries_end) else {
        // Pillow rebuilds an Exif mapping when a valid Orientation entry is
        // followed by an incomplete advertised entry.  Preserve every
        // complete non-Orientation record and emit the serializer's zero next
        // IFD pointer instead of retaining the malformed raw tail.
        let available_entries = data.len().saturating_sub(entry_count_end) / 12;
        let partial_end = entry_count_end.saturating_add(available_entries * 12);
        let Some(partial_entries) = data.get(entry_count_end..partial_end) else {
            return raw.to_vec();
        };
        let retained = retain_entries(partial_entries);
        let mut output = Vec::with_capacity(
            prefix_len
                .saturating_add(ifd_offset)
                .saturating_add(2)
                .saturating_add(retained.len().saturating_mul(12))
                .saturating_add(4),
        );
        output.extend_from_slice(&raw[..prefix_len]);
        output.extend_from_slice(&data[..ifd_offset]);
        let retained_count = retained.len() as u16;
        if le {
            output.extend_from_slice(&retained_count.to_le_bytes());
        } else {
            output.extend_from_slice(&retained_count.to_be_bytes());
        }
        for entry in retained {
            output.extend_from_slice(&entry);
        }
        output.extend_from_slice(&[0u8; 4]);
        return output;
    };
    let retained = retain_entries(entries);
    if retained.len() == entry_count {
        return raw.to_vec();
    }

    let mut output = Vec::with_capacity(raw.len().saturating_sub(12));
    output.extend_from_slice(&raw[..prefix_len]);
    output.extend_from_slice(&data[..ifd_offset]);
    let retained_count = retained.len() as u16;
    if le {
        output.extend_from_slice(&retained_count.to_le_bytes());
    } else {
        output.extend_from_slice(&retained_count.to_be_bytes());
    }
    for entry in retained {
        output.extend_from_slice(&entry);
    }
    let tail = &data[entries_end..];
    output.extend_from_slice(tail);
    if tail.len() < 4 {
        output.extend(std::iter::repeat(0).take(4 - tail.len()));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        CenteringInput, ImageOpsColor, contain_with_input, cover_with_input, fit, fit_with_input,
        pad_with_input, scale_with_input,
    };
    use crate::error::PilError;
    use crate::image::Image;
    use crate::ops::resize::ResampleInput;
    use crate::raster::GenericImageView;

    fn empty_image(size: (u32, u32)) -> Image {
        Image::new(size.0, size.1, "L", (0, 0, 0, 0)).expect("empty image dimensions are valid")
    }

    #[test]
    fn scale_factor_one_copies_empty_shapes_before_filter_validation() {
        for size in [(0, 0), (0, 1), (1, 0), (2, 3)] {
            let image = empty_image(size);
            let result = scale_with_input(&image, 1.0, Some(ResampleInput::Code(99)))
                .expect("factor one must ignore the resample value");
            assert_eq!(result.size().expect("copy dimensions are known"), size);
        }
    }

    #[test]
    fn scale_preserves_all_zero_resize_result() {
        let image = empty_image((0, 0));
        let result = scale_with_input(&image, 0.5, None).expect("empty resize remains empty");
        assert_eq!(result.size().expect("copy dimensions are known"), (0, 0));
    }

    #[test]
    fn scale_infinity_observes_first_zero_dimension() {
        let both_zero = scale_with_input(&empty_image((0, 0)), f64::INFINITY, None)
            .expect_err("inf times zero must fail as NaN");
        assert!(matches!(
            both_zero,
            PilError::ValueError(message) if message == "cannot convert float NaN to integer"
        ));

        let zero_width = scale_with_input(&empty_image((0, 1)), f64::INFINITY, None)
            .expect_err("inf times zero width must fail as NaN");
        assert!(matches!(
            zero_width,
            PilError::ValueError(message) if message == "cannot convert float NaN to integer"
        ));

        let zero_height = scale_with_input(&empty_image((1, 0)), f64::INFINITY, None)
            .expect_err("nonzero width must observe infinity first");
        assert!(matches!(
            zero_height,
            PilError::OverflowError(message) if message == "cannot convert float infinity to integer"
        ));
    }

    #[test]
    fn scale_rejects_rounded_zero_for_nonempty_source() {
        let image = empty_image((2, 3));
        let error = scale_with_input(&image, 0.1, None)
            .expect_err("a nonempty source cannot resize to a zero dimension");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));
    }

    #[test]
    fn pad_rejects_rounded_zero_contain_dimensions() {
        for size in [(2, 100), (100, 2)] {
            let image = empty_image(size);
            let error = pad_with_input(
                &image,
                1,
                1,
                None,
                ImageOpsColor::None,
                CenteringInput::Default,
            )
            .expect_err("contain must reject a rounded zero resize axis");
            assert!(matches!(
                error,
                PilError::ValueError(message) if message == "height and width must be > 0"
            ));
        }
    }

    #[test]
    fn pad_preserves_pillow_zero_width_source_rules() {
        let source = empty_image((0, 2));
        let padded = pad_with_input(
            &source,
            2,
            2,
            None,
            ImageOpsColor::None,
            CenteringInput::Default,
        )
        .expect("matching source height keeps an empty-width contain result")
        .materialize()
        .expect("empty-width padded image materializes");
        assert_eq!(padded.dimensions(), (2, 2));
        assert_eq!(padded.as_bytes(), &[0, 0, 0, 0]);

        let empty = pad_with_input(
            &source,
            0,
            2,
            None,
            ImageOpsColor::None,
            CenteringInput::Default,
        )
        .expect("zero-width target with matching source height is valid")
        .materialize()
        .expect("zero-width target materializes");
        assert_eq!(empty.dimensions(), (0, 2));
        assert!(empty.as_bytes().is_empty());

        let error = pad_with_input(
            &source,
            1,
            1,
            None,
            ImageOpsColor::None,
            CenteringInput::Default,
        )
        .expect_err("changed contain height must fail for an empty-width source");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));
    }

    #[test]
    fn contain_preserves_zero_dimension_and_filter_error_ordering() {
        let nonempty = empty_image((2, 100));
        let error = contain_with_input(&nonempty, 1, 1, None)
            .expect_err("rounded-zero contain output must fail");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));

        let invalid_filter = contain_with_input(&nonempty, 1, 1, Some(ResampleInput::Code(99)))
            .expect_err("resize filter validation precedes rounded-zero validation");
        assert!(
            matches!(invalid_filter, PilError::ValueError(message) if message.starts_with(
                "Unknown resampling filter (99)."
            ))
        );

        let zero_height =
            contain_with_input(&empty_image((1, 0)), 1, 1, Some(ResampleInput::Code(99)))
                .expect_err("zero source height must raise before filter validation");
        assert!(matches!(
            zero_height,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));
    }

    #[test]
    fn contain_preserves_empty_width_source_rules() {
        let source = empty_image((0, 2));
        for target in [(0, 2), (1, 2), (2, 2)] {
            let result = contain_with_input(&source, target.0, target.1, None)
                .expect("unchanged contain height keeps an empty-width copy")
                .materialize()
                .expect("empty-width contain materializes");
            assert_eq!(result.dimensions(), (0, 2));
            assert!(result.as_bytes().is_empty());
        }

        let error = contain_with_input(&source, 1, 1, None)
            .expect_err("changed contain height must reject an empty-width result");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));
    }

    #[test]
    fn cover_preserves_empty_width_source_rules() {
        let source = empty_image((0, 2));
        let result = cover_with_input(&source, 0, 2, None)
            .expect("same-size empty-width cover returns a copy")
            .materialize()
            .expect("empty-width cover materializes");
        assert_eq!(result.dimensions(), (0, 2));
        assert!(result.as_bytes().is_empty());

        let error = cover_with_input(&source, 0, 1, None)
            .expect_err("changed empty-width cover size must fail resize validation");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));

        let division = cover_with_input(&source, 1, 2, None)
            .expect_err("positive destination width divides by zero source width");
        assert!(matches!(
            division,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));

        let source_height =
            cover_with_input(&empty_image((1, 0)), 1, 1, Some(ResampleInput::Code(99)))
                .expect_err("zero source height precedes filter validation");
        assert!(matches!(
            source_height,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));
    }

    #[test]
    fn fit_preserves_zero_dimension_source_rules() {
        let source = empty_image((0, 2));
        let empty = fit_with_input(&source, 0, 2, None, 0.0, CenteringInput::Default)
            .expect("same-size empty-width fit is a valid copy")
            .materialize()
            .expect("empty-width fit materializes");
        assert_eq!(empty.dimensions(), (0, 2));
        assert!(empty.as_bytes().is_empty());

        let black = fit_with_input(&source, 2, 2, None, 0.0, CenteringInput::Default)
            .expect("a positive target at the source height is valid")
            .materialize()
            .expect("zero-width source fit materializes");
        assert_eq!(black.dimensions(), (2, 2));
        assert_eq!(black.as_bytes(), &[0, 0, 0, 0]);

        for target in [(0, 1), (1, 1), (1, 0), (0, 3)] {
            let error = fit_with_input(
                &source,
                target.0,
                target.1,
                None,
                0.0,
                CenteringInput::Default,
            )
            .expect_err("fit must preserve Pillow's zero-dimension error");
            match (target, error) {
                ((1, 0), PilError::ZeroDivisionError(message)) => {
                    assert_eq!(message, "division by zero")
                }
                (_, PilError::ValueError(message)) => {
                    assert_eq!(message, "height and width must be > 0")
                }
                (target, error) => panic!("unexpected fit result for {target:?}: {error:?}"),
            }
        }

        let invalid_filter = fit_with_input(
            &source,
            2,
            2,
            Some(ResampleInput::Code(99)),
            0.0,
            CenteringInput::Default,
        )
        .expect_err("filter validation follows fit geometry");
        assert!(matches!(
            invalid_filter,
            PilError::ValueError(message) if message.starts_with("Unknown resampling filter (99).")
        ));

        let invalid_filter_after_zero_width = fit_with_input(
            &source,
            0,
            3,
            Some(ResampleInput::Code(99)),
            0.0,
            CenteringInput::Default,
        )
        .expect_err("Image.resize parses the filter before rejecting a zero width");
        assert!(matches!(
            invalid_filter_after_zero_width,
            PilError::ValueError(message) if message.starts_with("Unknown resampling filter (99).")
        ));

        let normalized_centering = fit_with_input(
            &source,
            2,
            2,
            None,
            0.0,
            CenteringInput::Values(vec![-1.0, 2.0]),
        )
        .expect("out-of-range centering falls back to the midpoint")
        .materialize()
        .expect("normalized fit materializes");
        assert_eq!(normalized_centering.dimensions(), (2, 2));
        assert_eq!(normalized_centering.as_bytes(), &[0, 0, 0, 0]);
    }

    #[test]
    fn fit_preserves_zero_height_and_filter_error_ordering() {
        let zero_height = fit_with_input(
            &empty_image((1, 0)),
            1,
            1,
            Some(ResampleInput::Code(99)),
            0.0,
            CenteringInput::Default,
        )
        .expect_err("source height division precedes filter parsing");
        assert!(matches!(
            zero_height,
            PilError::ZeroDivisionError(message) if message == "float division by zero"
        ));

        let zero_target_height = fit(
            &empty_image((2, 3)),
            1,
            0,
            Some("not-a-filter"),
            0.0,
            (0.5, 0.5),
        )
        .expect_err("target height division precedes filter parsing");
        assert!(matches!(
            zero_target_height,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));
    }

    #[test]
    fn fit_integer_f_crop_precedes_filter_tails_for_special_values() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&f32::INFINITY.to_le_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_le_bytes());
        let source = Image::frombytes("F", (1, 2), &bytes).expect("F source");
        let fitted = fit(&source, 1, 1, Some("LANCZOS"), 0.0, (1.0, 1.0))
            .expect("Fit operation")
            .materialize()
            .expect("Fit materializes");
        assert_eq!(fitted.as_bytes(), &(-2.25f32).to_le_bytes());
    }
}

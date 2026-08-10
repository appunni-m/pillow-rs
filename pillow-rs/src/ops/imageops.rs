//! Pillow `ImageOps`-style module functions.
//!
//! Functions take [`crate::Image`] handles and return lazy result images where the
//! operation can be represented in the compute pipeline.

use crate::error::PilError;
use crate::image::Image;
use crate::ops::resize::{ResampleInput, parse_resample, parse_resample_input};
use crate::pipeline::PipelineOp;

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

fn resolve_centering(input: CenteringInput, pad: bool) -> Result<(f64, f64), PilError> {
    match input {
        CenteringInput::Default => Ok((0.5, 0.5)),
        CenteringInput::Scalar(_) if pad => Ok((0.5, 0.5)),
        CenteringInput::Scalar(_) => Err(PilError::TypeError(
            "cannot unpack non-iterable float object".into(),
        )),
        CenteringInput::Values(values) if values.len() == 2 => Ok((values[0], values[1])),
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

/// Normalizes image contrast by clipping darkest and lightest values.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
pub fn autocontrast(image: &Image, cutoff: f64) -> Result<Image, PilError> {
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
    Ok(Image::push_op(image, PipelineOp::Autocontrast { cutoff }))
}

/// Normalizes contrast after validating an optional Pillow mask.
pub fn autocontrast_with_mask(
    image: &Image,
    cutoff: f64,
    mask: ImageOpsMask,
) -> Result<Image, PilError> {
    if matches!(&mask, ImageOpsMask::None) {
        return autocontrast(image, cutoff);
    }
    validate_imageops_mask(image, mask)?;
    autocontrast(image, cutoff)
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
    if mode == "LA" || mode == "RGBA" {
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
    if mode == "LA" || mode == "RGBA" {
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
    let filter = parse_resample(filter)?;
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
    let filter = parse_imageops_filter(filter)?;
    Ok(Image::push_op(image, PipelineOp::Contain { w, h, filter }))
}

/// Resizes an image to cover `(w, h)`, cropping overflow.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn cover(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    validate_cover_source(image)?;
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
    let filter = parse_imageops_filter(filter)?;
    validate_cover_source(image)?;
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
    let filter = parse_resample(filter)?;
    validate_fit_source(image)?;
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
    let filter = parse_imageops_filter(filter)?;
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
    let centering = resolve_centering(centering, false)?;
    if filter_was_none && centering == (0.5, 0.5) {
        return fit(image, w, h, None, bleed, centering);
    }
    validate_fit_source(image)?;
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

// Pillow evaluates ImageOps.cover/fit's source aspect ratio when the public
// call is made. Keep those division errors in core instead of allowing a
// deferred SIMD/CPU pipeline to turn an invalid empty input into an image.
fn validate_cover_source(image: &Image) -> Result<(), PilError> {
    let (width, height) = image.size()?;
    if width == 0 || height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    Ok(())
}

fn validate_fit_source(image: &Image) -> Result<(), PilError> {
    let (_, height) = image.size()?;
    if height == 0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
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
    let centering = resolve_centering(centering, true)?;
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

/// Scales image dimensions by `factor`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn scale(image: &Image, factor: f64, filter: Option<&str>) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(image, PipelineOp::Scale { factor, filter }))
}

/// Crops the same border width from every image edge.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports invalid
/// crop geometry or later materialization failures.
pub fn crop(image: &Image, border: u32) -> Result<Image, PilError> {
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
    let Some(entries) = data.get(entry_count_end..entries_end) else {
        return raw.to_vec();
    };
    let retained: Vec<Vec<u8>> = entries
        .chunks_exact(12)
        .filter(|entry| read_u16(&entry[..2]) != 0x0112)
        .map(|entry| {
            let mut entry = entry.to_vec();
            // Pillow's Exif serializer emits ImageWidth's integer value as a
            // LONG even when the source IFD used an inline SHORT.
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
        .collect();
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
    use super::{exif_get_orientation, exif_remove_orientation};

    #[test]
    fn test_exif_get_orientation_empty() {
        assert_eq!(exif_get_orientation(&[]), None);
        assert_eq!(exif_get_orientation(b"abc"), None);
    }

    #[test]
    fn test_exif_get_orientation_no_exif() {
        // Valid TIFF header but no orientation tag
        // II (little-endian), magic=42, IFD0 at offset 8, 0 entries
        let raw = b"II\x2a\x00\x08\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(exif_get_orientation(raw), None);
    }

    #[test]
    fn test_exif_get_orientation_with_tag_le() {
        // Little-endian TIFF with orientation=6 (ROTATE_270)
        // II, magic=42 (2a 00), IFD0 offset=8, 1 entry
        // Entry: tag=0112, type=SHORT(03), count=1, value=6
        let mut raw = Vec::new();
        raw.extend_from_slice(b"II\x2a\x00"); // Byte order + magic
        raw.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset
        raw.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
        raw.extend_from_slice(&0x0112u16.to_le_bytes()); // tag = Orientation
        raw.extend_from_slice(&3u16.to_le_bytes()); // type = SHORT
        raw.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        raw.extend_from_slice(&6u16.to_le_bytes()); // value = 6
        raw.extend_from_slice(&[0u8; 2]); // padding

        assert_eq!(exif_get_orientation(&raw), Some(6));
    }

    #[test]
    fn test_exif_get_orientation_with_tag_be() {
        // Big-endian TIFF with orientation=3 (ROTATE_180)
        let mut raw = Vec::new();
        raw.extend_from_slice(b"MM\x00\x2a"); // Byte order + magic
        raw.extend_from_slice(&8u32.to_be_bytes()); // IFD0 offset
        raw.extend_from_slice(&1u16.to_be_bytes()); // 1 entry
        raw.extend_from_slice(&0x0112u16.to_be_bytes()); // tag
        raw.extend_from_slice(&3u16.to_be_bytes()); // type
        raw.extend_from_slice(&1u32.to_be_bytes()); // count
        raw.extend_from_slice(&3u16.to_be_bytes()); // value = 3
        raw.extend_from_slice(&[0u8; 2]); // padding

        assert_eq!(exif_get_orientation(&raw), Some(3));
    }

    #[test]
    fn test_exif_get_orientation_exif_jpeg() {
        // Exif-JPEG format: starts with "Exif\0\0", then TIFF
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II\x2a\x00");
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&8u16.to_le_bytes()); // value = 8
        tiff.extend_from_slice(&[0u8; 2]);

        let mut raw = Vec::new();
        raw.extend_from_slice(b"Exif\x00\x00");
        raw.extend_from_slice(&tiff);

        assert_eq!(exif_get_orientation(&raw), Some(8));
    }

    #[test]
    fn test_exif_without_orientation_retains_other_ifd_entries() {
        let mut raw = b"Exif\x00\x00II\x2a\x00".to_vec();
        raw.extend_from_slice(&8u32.to_le_bytes());
        raw.extend_from_slice(&2u16.to_le_bytes());
        raw.extend_from_slice(&0x0100u16.to_le_bytes());
        raw.extend_from_slice(&3u16.to_le_bytes());
        raw.extend_from_slice(&1u32.to_le_bytes());
        raw.extend_from_slice(&2u16.to_le_bytes());
        raw.extend_from_slice(&[0u8; 2]);
        raw.extend_from_slice(&0x0112u16.to_le_bytes());
        raw.extend_from_slice(&3u16.to_le_bytes());
        raw.extend_from_slice(&1u32.to_le_bytes());
        raw.extend_from_slice(&6u16.to_le_bytes());
        raw.extend_from_slice(&[0u8; 2]);
        raw.extend_from_slice(&0u32.to_le_bytes());

        let expected = b"Exif\x00\x00II\x2a\x00\x08\x00\x00\x00\x01\x00\x00\x01\x04\x00\x01\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(exif_remove_orientation(&raw), expected);
    }

    #[test]
    fn test_exif_without_orientation_retains_big_endian_width_value() {
        let raw = b"Exif\x00\x00MM\x00\x2a\x00\x00\x00\x08\x00\x02\x01\x00\x00\x03\x00\x00\x00\x01\x00\x02\x00\x00\x01\x12\x00\x03\x00\x00\x00\x01\x00\x03\x00\x00\x00\x00\x00\x00";
        let expected = b"Exif\x00\x00MM\x00\x2a\x00\x00\x00\x08\x00\x01\x01\x00\x00\x04\x00\x00\x00\x01\x00\x00\x00\x02\x00\x00\x00\x00";
        assert_eq!(exif_remove_orientation(raw), expected);
    }
}

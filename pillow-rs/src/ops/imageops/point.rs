use super::*;
use crate::raster::Mode;

fn mode_identity(image: &Image) -> Result<Mode, PilError> {
    match image.known_mode_identity() {
        Some(mode) => Ok(mode),
        // TODO(cpu-migration): make every lazy pipeline expose its exact Mode
        // before operation admission so validation never has to materialize.
        None => Ok(image.materialized_shared()?.mode()),
    }
}

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::ImageLumaA16 => "LA",
        Mode::ImageRgb16 | Mode::ImageRgb32F => "RGB",
        Mode::ImageRgba16 | Mode::ImageRgba32F => "RGBA",
        _ => mode.pillow_name().unwrap_or("unknown"),
    }
}

fn validate_autocontrast_mode(image: &Image) -> Result<(), PilError> {
    let mode = mode_identity(image)?;
    // Pillow 12.2.0 `ImageOps._lut` accepts only "L" and "RGB"; "P" raises
    // NotImplementedError and every other mode raises the OSError below.
    if mode == Mode::ImagePalette8 {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    if !matches!(mode, Mode::ImageLuma8 | Mode::ImageRgb8) {
        return Err(PilError::OsError(format!(
            "not supported for mode {}",
            mode_label(mode)
        )));
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
                    mask: Some(Arc::new(*mask)),
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
    let mode = mode_identity(image)?;
    // Pillow 12.2.0 converts "P" to "RGB" before building the LUT and then
    // `_lut` accepts only "L" and "RGB"; all other modes raise the OSError.
    if !matches!(mode, Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImagePalette8) {
        return Err(PilError::OsError(format!(
            "not supported for mode {}",
            mode_label(mode)
        )));
    }
    Ok(Image::push_op(image, PipelineOp::Equalize))
}

/// Equalizes an image after validating an optional Pillow mask.
pub fn equalize_with_mask(image: &Image, mask: ImageOpsMask) -> Result<Image, PilError> {
    match mask {
        ImageOpsMask::None => equalize(image),
        ImageOpsMask::Invalid(type_name) => Err(PilError::AttributeError(format!(
            "'{type_name}' object has no attribute 'load'"
        ))),
        ImageOpsMask::Image(mask) => {
            validate_imageops_mask(image, ImageOpsMask::Image(mask.clone()))?;
            // Retain equalize's mode validation after mask validation, matching
            // Pillow's histogram-before-LUT error order.
            let mode = mode_identity(image)?;
            if !matches!(mode, Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImagePalette8) {
                return Err(PilError::OsError(format!(
                    "not supported for mode {}",
                    mode_label(mode)
                )));
            }
            Ok(Image::push_op(
                image,
                PipelineOp::EqualizeMasked {
                    mask: Arc::new(*mask),
                },
            ))
        }
    }
}

fn invert_for_mode(image: &Image, mode: Mode) -> Result<Image, PilError> {
    // Pillow's ImageOps._lut path rejects CMYK before creating the result.
    // Keep this separate from ImageChops.invert, which has a different mode
    // contract and is implemented by the chops module.
    if mode == Mode::ImageCmyk8 {
        return Err(PilError::OsError("not supported for mode CMYK".into()));
    }
    if matches!(mode, Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 | Mode::ImageRgba8) {
        return Err(PilError::OsError(format!(
            "not supported for mode {}",
            mode_label(mode)
        )));
    }
    Ok(Image::push_op(image, PipelineOp::Invert))
}

/// Inverts all pixel values.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
pub fn invert(image: &Image) -> Result<Image, PilError> {
    invert_for_mode(image, mode_identity(image)?)
}

/// Inverts through the `ImageOps.invert` compatibility path.
///
/// Unlike `ImageChops.invert`, Pillow raises for `P` mode here.
///
/// # Errors
///
/// Returns [`PilError::NotImplementedError`] for `P` mode, or errors from
/// the underlying inversion operation.
pub fn invert_ops(image: &Image) -> Result<Image, PilError> {
    let mode = mode_identity(image)?;
    if mode == Mode::ImagePalette8 {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    invert_for_mode(image, mode)
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
    let mode = mode_identity(image)?;
    // ImageOps.posterize delegates to `_lut`, whose public contract is the
    // same as autocontrast: P is a named unsupported path and all other modes
    // outside L/RGB raise OSError before a pipeline is created.
    if mode == Mode::ImagePalette8 {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    if !matches!(mode, Mode::ImageLuma8 | Mode::ImageRgb8) {
        return Err(PilError::OsError(format!(
            "not supported for mode {}",
            mode_label(mode)
        )));
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
/// Returns [`PilError::OsError`] for modes other than L and RGB,
/// or another [`PilError`] when mode detection fails.
pub fn solarize(image: &Image, threshold: u8) -> Result<Image, PilError> {
    solarize_with_input(image, SolarizeInput::Threshold(f64::from(threshold)))
}

/// Solarizes with numeric or host-defined threshold comparisons.
///
/// A sample is unchanged exactly when `sample < threshold`. Fractional
/// thresholds therefore round upward; thresholds above 255 preserve all bytes,
/// and NaN inverts them all. Custom comparisons need not be monotonic.
///
/// # Errors
///
/// Returns [`PilError::NotImplementedError`] for P, [`PilError::OsError`] for
/// other modes outside L/RGB, or an error when mode detection fails. Bindings
/// must evaluate custom comparisons before calling this function so their
/// exceptions and mutations precede image validation, as they do in Pillow.
pub fn solarize_with_input(image: &Image, input: SolarizeInput) -> Result<Image, PilError> {
    let mode = mode_identity(image)?;
    // Pillow 12.2.0 exposes P as an explicit unsupported ImageOps._lut path;
    // this must be raised at the call, before deferred pipeline execution.
    if mode == Mode::ImagePalette8 {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    if !matches!(mode, Mode::ImageLuma8 | Mode::ImageRgb8) {
        return Err(PilError::OsError(format!(
            "not supported for mode {}",
            mode_label(mode)
        )));
    }
    let lut = match input {
        SolarizeInput::Threshold(threshold) if threshold <= 255.0 || threshold.is_nan() => {
            return Ok(Image::push_op(
                image,
                PipelineOp::Solarize {
                    threshold: threshold.ceil() as u8,
                },
            ));
        }
        SolarizeInput::Threshold(_) => (0..=255u8).collect::<Vec<_>>(),
        SolarizeInput::Comparisons(below) => (0..=255u8)
            .map(|value| {
                if below[usize::from(value)] {
                    value
                } else {
                    255 - value
                }
            })
            .collect(),
    };
    // TODO(cpu-migration): preserve this high-threshold/custom-comparison
    // Solarize route when Eval receives its prepared CPU owner; these requests
    // deliberately become a LUT operation rather than PipelineOp::Solarize.
    crate::ops::module_fns::eval_replicated(
        image,
        &lut,
        if mode == Mode::ImageLuma8 { 1 } else { 3 },
    )
}

/// Builds the 256 host-defined threshold comparisons in byte sample order.
///
/// The host callback remains in its owning runtime so custom comparison
/// behavior and its error type are preserved. Core controls the sample order.
#[doc(hidden)]
pub fn solarize_comparison_table<E, F>(mut compare: F) -> Result<Box<[bool; 256]>, E>
where
    F: FnMut(usize) -> Result<bool, E>,
{
    let mut below = Box::new([false; 256]);
    for (sample, result) in below.iter_mut().enumerate() {
        *result = compare(sample)?;
    }
    Ok(below)
}

/// Converts an image to grayscale using Pillow-compatible BT.601 luma.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] for Pillow's unsupported `La` to `L`
/// conversion. Other materialization failures may be reported lazily.
pub fn grayscale(image: &Image) -> Result<Image, PilError> {
    if mode_identity(image)? == Mode::ImageLaPremultiplied8 {
        return Err(PilError::ValueError(
            "conversion from La to L not supported".into(),
        ));
    }
    if let Some(result) = image.try_grayscale_eager_rgb()? {
        return Ok(result);
    }
    Ok(Image::push_op(image, PipelineOp::Grayscale))
}

/// Validates that an image can be colorized.
///
/// # Errors
///
/// Returns [`PilError::AssertionError`] when the source mode is not `"L"`, or
/// another [`PilError`] when mode detection fails.
pub fn validate_colorize_mode(image: &Image) -> Result<(), PilError> {
    let mode = mode_identity(image)?;
    if mode != Mode::ImageLuma8 {
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
    if mid.is_some() {
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

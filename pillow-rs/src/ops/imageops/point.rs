use super::*;
use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::pipeline::AutocontrastOptions;
use crate::raster::Mode;

fn mode_identity(image: &Image) -> Result<Mode, PilError> {
    image.mode_identity()
}

fn validate_autocontrast_mode_after_histogram(
    image: &Image,
    options: &AutocontrastOptions,
    mask: Option<&Image>,
) -> Result<(), PilError> {
    let mode = mode_identity(image)?;
    let support = OperationId::Autocontrast.support().for_mode(mode);
    if matches!(support, ModeSupport::Rejected { .. }) {
        // Pillow computes the source histogram and LUT parameters before `_lut`
        // rejects an unsupported output mode. Keep accepted operations lazy;
        // only their rejection path needs this eager work to preserve fault order.
        let histogram = if options.preserve_tone {
            image
                .convert(Mode::ImageLuma8, None, None, None, None)?
                .histogram_with_mask(mask)?
        } else {
            image.histogram_with_mask(mask)?
        };
        autocontrast_lut_from_histogram(
            &histogram,
            options.cutoff,
            options.ignore.clone(),
        )?;
    }
    support.validate_mode()
}

/// Normalizes image contrast by clipping darkest and lightest values.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
pub fn autocontrast(image: &Image, cutoff: f64) -> Result<Image, PilError> {
    autocontrast_with_options(image, AutocontrastOptions::symmetric(cutoff))
}

/// Normalizes image contrast using independent cutoffs, ignored bins, and tone-preserving luma statistics.
pub fn autocontrast_with_options(
    image: &Image,
    options: AutocontrastOptions,
) -> Result<Image, PilError> {
    validate_autocontrast_mode_after_histogram(image, &options, None)?;
    Ok(Image::push_op(image, PipelineOp::Autocontrast { options, mask: None }))
}

/// Normalizes contrast after validating an optional Pillow mask.
pub fn autocontrast_with_mask(
    image: &Image,
    cutoff: f64,
    mask: ImageOpsMask,
) -> Result<Image, PilError> {
    autocontrast_with_options_and_mask(
        image,
        AutocontrastOptions::symmetric(cutoff),
        mask,
    )
}

/// Normalizes image contrast using typed options after validating an optional Pillow mask.
pub fn autocontrast_with_options_and_mask(
    image: &Image,
    options: AutocontrastOptions,
    mask: ImageOpsMask,
) -> Result<Image, PilError> {
    match mask {
        ImageOpsMask::None => autocontrast_with_options(image, options),
        ImageOpsMask::Invalid(type_name) => {
            validate_imageops_mask(image, ImageOpsMask::Invalid(type_name))
                .and_then(|_| autocontrast_with_options(image, options))
        }
        ImageOpsMask::Image(mask) => {
            validate_imageops_mask(image, ImageOpsMask::Image(mask.clone()))?;
            validate_autocontrast_mode_after_histogram(image, &options, Some(mask.as_ref()))?;
            Ok(Image::push_op(
                image,
                PipelineOp::Autocontrast {
                    options,
                    mask: Some(Arc::new(*mask)),
                },
            ))
        }
    }
}

/// Builds the Rust autocontrast LUT from a histogram computed by a host
/// wrapper before it coerces the operation's cutoff and ignored bins.
#[doc(hidden)]
pub fn autocontrast_lut_from_histogram(
    histogram: &[u32],
    cutoff: (f64, f64),
    ignore: Option<Vec<i64>>,
) -> Result<Vec<u8>, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::autocontrast_lut_from_histogram(
        histogram,
        &AutocontrastOptions {
            cutoff,
            ignore,
            preserve_tone: false,
        },
    )
}

/// Builds one channel's LUT from cutoff counts evaluated with host arithmetic.
#[doc(hidden)]
pub fn autocontrast_lut_from_histogram_and_counts(
    histogram: &[u32],
    cutoff_counts: (i128, i128),
) -> Result<Vec<u8>, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::autocontrast_lut_from_cutoff_counts(
        histogram,
        cutoff_counts,
    )
}

/// Equalizes the image histogram.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
// TODO(pillow-rs#3ptb): Verify this lazy Rust API's mode/architecture route,
// output, and fallback receipts in the frozen CPU matrix. Python ImageOps
// composes the eager Histogram and Point paths for Pillow callback order.
pub fn equalize(image: &Image) -> Result<Image, PilError> {
    let mode = mode_identity(image)?;
    OperationId::Equalize.support().for_mode(mode).validate_mode()?;
    Ok(Image::push_op(image, PipelineOp::Equalize))
}

/// Builds the Equalize lookup table from counts collected by a host wrapper.
///
/// The host can collect a histogram eagerly to preserve callback ordering;
/// table arithmetic and mode admission remain in the Rust operation layer.
#[doc(hidden)]
pub fn equalize_lut_from_histogram(
    image: &Image,
    histogram: &[u32],
) -> Result<Vec<u8>, PilError> {
    let lut = crate::compute::pool_cpu::ops::imageops::helpers::histogram::
        equalize_lut_from_histogram(histogram)
        .ok_or_else(|| PilError::ValueError("invalid Equalize histogram length".into()))?;
    let mode = mode_identity(image)?;
    OperationId::Equalize.support().for_mode(mode).validate_mode()?;
    Ok(lut)
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
            OperationId::Equalize.support().for_mode(mode).validate_mode()?;
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
    // The public ImageOps guard differs from the exact-mode prepared Invert
    // child. Keep both contracts visible while preserving lazy child errors.
    crate::compute::pool_cpu::ops::imageops::ops::validate_imageops_invert_mode(mode)?;
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
/// Returns [`PilError::OsError`] for modes outside L/RGB/P, or another
/// [`PilError`] when mode detection fails. P is a core-only native-index path.
pub fn posterize(image: &Image, bits: u8) -> Result<Image, PilError> {
    let mode = mode_identity(image)?;
    OperationId::Posterize.support().for_mode(mode).validate_mode()?;
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
/// Returns [`PilError::OsError`] for modes outside L/RGB/P, or another
/// [`PilError`] when mode detection fails. P is a core-only native-index path.
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
/// Returns [`PilError::OsError`] for modes outside L/RGB/P, or an error when
/// mode detection fails. P is a core-only native-index path. Bindings must
/// evaluate custom comparisons before calling this function so their
/// exceptions and mutations precede image validation, as they do in Pillow.
pub fn solarize_with_input(image: &Image, input: SolarizeInput) -> Result<Image, PilError> {
    let mode = mode_identity(image)?;
    OperationId::Solarize.support().for_mode(mode).validate_mode()?;
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
    // High-threshold and custom-comparison cases deliberately become a
    // mode-aware Eval LUT operation rather than PipelineOp::Solarize.
    crate::ops::module_fns::eval_replicated(
        image,
        &lut,
        if matches!(mode, Mode::ImageLuma8 | Mode::ImagePalette8) {
            1
        } else {
            3
        },
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
    OperationId::Colorize.support().for_mode(mode).validate_mode()
}

/// Builds the three channel tables used by the Python ImageOps Colorize
/// composition after the host has evaluated Pillow's dynamic range inputs.
#[doc(hidden)]
pub fn colorize_lut_from_range_lengths(
    black: (u8, u8, u8),
    white: (u8, u8, u8),
    mid: Option<(u8, u8, u8)>,
    lengths: (usize, usize, usize, usize),
) -> Result<Vec<u8>, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::colorize::
        lookup_table_from_range_lengths(black, white, mid, lengths)
}

/// Colorizes an `L` image by mapping black and white endpoints.
///
/// # Errors
///
/// Returns [`PilError::AssertionError`] when the source mode is not `"L"`, or
/// another [`PilError`] when mode detection fails.
// TODO(pillow-rs#3ptb): Verify this typed lazy API's native output and
// mode/architecture receipts separately from Python wrapper callback order.
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

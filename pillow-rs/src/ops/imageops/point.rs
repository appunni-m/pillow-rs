use super::palette_lut::{ImageOpsPaletteLutCallbacks, imageops_lut_with_palette_callbacks};
use super::*;
use crate::compute::registry::support::OperationId;
use crate::raster::Mode;

/// Host operations used by ImageOps wrappers that apply a lookup table.
#[doc(hidden)]
pub trait ImageOpsLutCallbacks {
    /// Host lookup table passed to the public wrapper's `Image.point` method.
    type Image;
    type Lut;
    /// Value returned by the wrapper's public `Image.point` method.
    type Output;
    /// Host or core error, translated at the binding boundary.
    type Error: From<PilError>;

    /// Evaluate the shared `_lut` helper's public `image.mode == "P"` branch.
    fn image_mode_is_palette(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Evaluate `image.mode in ("L", "RGB")` using host comparison rules.
    fn image_mode_is_luma_or_rgb(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Evaluate the `_lut` helper's public `image.mode == "RGB"` branch.
    fn image_mode_is_rgb(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Return the wrapper's unsupported-mode error using the current mode value.
    fn unsupported_mode_error(&mut self, image: &Self::Image) -> Result<Self::Error, Self::Error>;
    /// Return the current LUT length using the host's `len` behavior.
    fn lut_length(&mut self, lut: &Self::Lut) -> Result<usize, Self::Error>;
    /// Return `lut + lut + lut` using the host's sequence behavior.
    fn repeat_lut(&mut self, lut: Self::Lut) -> Result<Self::Lut, Self::Error>;
    /// Call the public wrapper's `point(lut)` method.
    fn point(&mut self, image: &Self::Image, lut: Self::Lut) -> Result<Self::Output, Self::Error>;
}

/// Host operations used by Pillow's ImageOps Invert wrapper.
#[doc(hidden)]
pub trait ImageOpsInvertCallbacks: ImageOpsPaletteLutCallbacks {
    /// Build `list(range(255, -1, -1))` using the host namespace.
    fn invert_lut(&mut self) -> Result<Self::Lut, Self::Error>;
    /// Evaluate the public `image.mode == "1"` branch.
    fn image_mode_is_one(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
}

/// Host sequence primitives used while `pillow-rs` builds an ImageOps LUT.
#[doc(hidden)]
pub trait ImageOpsLutBuildCallbacks: ImageOpsPaletteLutCallbacks {
    /// A value yielded by the host's `range(256)` iterator.
    type Value;
    /// Iterator returned by the host's `range(256)` expression.
    type Iterator;
    /// Mutable host list being filled by the Rust-owned operation loop.
    type Builder;

    /// Construct the operation's empty list at its source-defined point.
    fn new_lut_builder(&mut self) -> Result<Self::Builder, Self::Error>;
    /// Evaluate `range(256)` and acquire its iterator.
    fn lut_samples(&mut self) -> Result<Self::Iterator, Self::Error>;
    /// Fetch the next value from the current iterator.
    fn next_lut_sample(
        &mut self,
        samples: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error>;
    /// Append one computed output to the list.
    fn append_lut_value(
        &mut self,
        builder: &mut Self::Builder,
        value: Self::Value,
    ) -> Result<(), Self::Error>;
    /// Finish the list and pass it to the shared ImageOps mode route.
    fn finish_lut(&mut self, builder: Self::Builder) -> Result<Self::Lut, Self::Error>;
}

/// Host protocols used by the Rust-owned ImageOps Posterize LUT loop.
#[doc(hidden)]
pub trait ImageOpsPosterizeCallbacks: ImageOpsLutBuildCallbacks {
    /// The caller's bits value, retaining Python's numeric protocols.
    type Bits;

    /// Evaluate the left-side expression `8 - bits`.
    fn eight_minus_bits(&mut self, bits: &Self::Bits) -> Result<Self::Value, Self::Error>;
    /// Evaluate `2 ** exponent` with the host's power protocol.
    fn two_to_power(&mut self, exponent: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Evaluate `power - 1` with the host's subtraction protocol.
    fn subtract_one(&mut self, power: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Evaluate Python's unary bitwise inversion.
    fn invert_mask(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Evaluate each list-comprehension item `sample & mask`.
    fn and_sample_with_mask(
        &mut self,
        sample: &Self::Value,
        mask: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
}

/// Host protocols used by the Rust-owned ImageOps Solarize LUT loop.
#[doc(hidden)]
pub trait ImageOpsSolarizeCallbacks: ImageOpsLutBuildCallbacks {
    /// The caller's threshold, retaining Python's comparison protocol.
    type Threshold;

    /// Evaluate `sample < threshold` with host comparison and truthiness.
    fn sample_is_below_threshold(
        &mut self,
        sample: &Self::Value,
        threshold: &Self::Threshold,
    ) -> Result<bool, Self::Error>;
    /// Evaluate `255 - sample` with the host's subtraction protocol.
    fn invert_sample(&mut self, sample: &Self::Value) -> Result<Self::Value, Self::Error>;
}

/// Applies Pillow's shared `_lut` checks when `palette=False`.
#[doc(hidden)]
// The Python ImageOps facade scopes this callback route and forwards the
// captured context through the public Image.point wrapper, preserving subclass
// dispatch without adding backend state to this host-neutral callback trait.
// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub fn imageops_lut_with_callbacks<C: ImageOpsLutCallbacks>(
    callbacks: &mut C,
    image: &C::Image,
    lut: C::Lut,
    operation: OperationId,
) -> Result<C::Output, C::Error> {
    if callbacks.image_mode_is_palette(image)? {
        return Err(PilError::NotImplementedError("mode P support coming soon".into()).into());
    }
    imageops_lut_non_palette(callbacks, image, lut, operation)
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub(super) fn imageops_lut_non_palette<C: ImageOpsLutCallbacks>(
    callbacks: &mut C,
    image: &C::Image,
    lut: C::Lut,
    operation: OperationId,
) -> Result<C::Output, C::Error> {
    if !callbacks.image_mode_is_luma_or_rgb(image)? {
        return Err(callbacks.unsupported_mode_error(image)?);
    }
    let rgb = callbacks.image_mode_is_rgb(image)?;
    let mode = if rgb {
        Mode::ImageRgb8
    } else {
        Mode::ImageLuma8
    };
    operation.support().for_mode(mode).validate_mode()?;
    let lut = if rgb && callbacks.lut_length(&lut)? == 256 {
        callbacks.repeat_lut(lut)?
    } else {
        lut
    };
    callbacks.point(image, lut)
}

/// Runs ImageOps Invert through its public mode, point, and palette paths.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub fn invert_with_callbacks<C: ImageOpsInvertCallbacks>(
    callbacks: &mut C,
    image: C::Image,
) -> Result<C::Output, C::Error> {
    let lut = callbacks.invert_lut()?;
    if callbacks.image_mode_is_one(&image)? {
        OperationId::ImageOpsInvert
            .support()
            .for_mode(Mode::One)
            .validate_mode()?;
        return callbacks.point(&image, lut);
    }
    imageops_lut_with_palette_callbacks(callbacks, &image, lut, OperationId::ImageOpsInvert)
}

/// Runs ImageOps Posterize's LUT construction and shared wrapper mode route.
/// TODO(pillow-rs#3ptb): freeze parity for a replaced `range` that mutates or
/// raises, proving it runs before result-list allocation and P-mode admission.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub fn posterize_with_callbacks<C: ImageOpsPosterizeCallbacks>(
    callbacks: &mut C,
    image: C::Image,
    bits: C::Bits,
) -> Result<C::Output, C::Error> {
    let exponent = callbacks.eight_minus_bits(&bits)?;
    let power = callbacks.two_to_power(&exponent)?;
    let lower_mask = callbacks.subtract_one(&power)?;
    let mask = callbacks.invert_mask(&lower_mask)?;
    let mut samples = callbacks.lut_samples()?;
    // The list-comprehension iterable is evaluated before its result list is
    // allocated, so a replaced `range` keeps its source-order side effects.
    let mut builder = callbacks.new_lut_builder()?;
    while let Some(sample) = callbacks.next_lut_sample(&mut samples)? {
        let value = callbacks.and_sample_with_mask(&sample, &mask)?;
        callbacks.append_lut_value(&mut builder, value)?;
    }
    let lut = callbacks.finish_lut(builder)?;
    imageops_lut_with_palette_callbacks(callbacks, &image, lut, OperationId::ImageOpsPosterize)
}

/// Runs ImageOps Solarize's LUT construction and shared wrapper mode route.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub fn solarize_with_callbacks<C: ImageOpsSolarizeCallbacks>(
    callbacks: &mut C,
    image: C::Image,
    threshold: C::Threshold,
) -> Result<C::Output, C::Error> {
    let mut builder = callbacks.new_lut_builder()?;
    let mut samples = callbacks.lut_samples()?;
    while let Some(sample) = callbacks.next_lut_sample(&mut samples)? {
        let value = if callbacks.sample_is_below_threshold(&sample, &threshold)? {
            sample
        } else {
            callbacks.invert_sample(&sample)?
        };
        callbacks.append_lut_value(&mut builder, value)?;
    }
    let lut = callbacks.finish_lut(builder)?;
    imageops_lut_with_palette_callbacks(callbacks, &image, lut, OperationId::ImageOpsSolarize)
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
// VERIFY: source=reviewed@2026-11-15; parity=pending; benchmark=pending
pub fn invert_ops(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode_identity()?;
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
// VERIFY: source=reviewed@2026-11-15; parity=pending; benchmark=pending
pub fn posterize(image: &Image, bits: u8) -> Result<Image, PilError> {
    let mode = image.mode_identity()?;
    OperationId::Posterize
        .support()
        .for_mode(mode)
        .validate_mode()?;
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
// VERIFY: source=reviewed@2026-11-15; parity=pending; benchmark=pending
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
// VERIFY: source=reviewed@2026-11-15; parity=pending; benchmark=pending
pub fn solarize_with_input(image: &Image, input: SolarizeInput) -> Result<Image, PilError> {
    let mode = image.mode_identity()?;
    OperationId::Solarize
        .support()
        .for_mode(mode)
        .validate_mode()?;
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
    if image.mode_identity()? == Mode::ImageLaPremultiplied8 {
        return Err(PilError::ValueError(
            "conversion from La to L not supported".into(),
        ));
    }
    if let Some(result) = image.try_grayscale_eager_rgb()? {
        return Ok(result);
    }
    Ok(Image::push_op(image, PipelineOp::Grayscale))
}

use super::*;
use crate::compute::registry::support::{
    ModeSupport, OperationId, SupportScope,
};
use crate::pipeline::AutocontrastOptions;
use crate::raster::Mode;

/// Host operations needed after ImageOps Autocontrast has built its LUT.
#[doc(hidden)]
pub trait ImageOpsAutocontrastResultCallbacks {
    /// Host value carrying the completed lookup table.
    type Lut;
    /// Result returned by the public host `Image.point` method.
    type Output;
    /// Host or core error, translated at the binding boundary.
    type Error: From<PilError>;

    /// Return the Pillow-compatible OSError for a mode rejected by ImageOps.
    fn unsupported_mode_error(&mut self) -> Result<Self::Error, Self::Error>;
    /// Get the lookup table length with the host's normal `len` behavior.
    fn lut_length(&mut self, lut: &Self::Lut) -> Result<usize, Self::Error>;
    /// Repeat the table using the host's sequence multiplication behavior.
    fn repeat_lut(&mut self, lut: Self::Lut) -> Result<Self::Lut, Self::Error>;
    /// Apply the completed table through the public host wrapper method.
    fn point(&mut self, lut: Self::Lut) -> Result<Self::Output, Self::Error>;
}

/// Host operations used by the complete ImageOps Autocontrast wrapper flow.
#[doc(hidden)]
pub trait ImageOpsAutocontrastSourceCallbacks {
    /// Public host image wrapper, including subclasses.
    type Image;
    /// Host value used for the histogram, cutoff, ignore, and LUT.
    type Value: Clone;
    /// Host mask passed to the public histogram method.
    type Mask;
    /// Host value whose truthiness selects tone-preserving conversion.
    type PreserveTone;
    /// Host error, translated at the binding boundary.
    type Error: From<PilError>;

    /// Call the public image's `convert("L")` method.
    fn convert_to_luma(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error>;
    /// Call the selected image's public `histogram(mask)` method.
    fn histogram(
        &mut self,
        image: &Self::Image,
        mask: &Self::Mask,
    ) -> Result<Self::Value, Self::Error>;
    /// Evaluate the public wrapper's truthiness protocol for `preserve_tone`.
    fn preserve_tone_is_truthy(
        &mut self,
        value: &Self::PreserveTone,
    ) -> Result<bool, Self::Error>;
    /// Read the current exact raster mode after histogram and LUT callbacks.
    fn mode(&mut self, image: &Self::Image) -> Result<Mode, Self::Error>;
}

/// Host operations for ImageOps Autocontrast histogram traversal.
#[doc(hidden)]
pub trait ImageOpsAutocontrastCallbacks {
    /// Host value used for the histogram, channel, cutoff, and LUT.
    type Value: Clone;
    /// Iterator returned by the host's `range` or ignored-bin iterable.
    type Iterator;
    /// Host error, translated at the binding boundary.
    type Error;

    /// Create the output LUT before evaluating the channel range.
    fn new_lut(&mut self) -> Result<Self::Value, Self::Error>;
    /// Evaluate `range(0, len(histogram), 256)` in host lookup order.
    fn channel_starts(
        &mut self,
        histogram: &Self::Value,
    ) -> Result<Self::Iterator, Self::Error>;
    /// Fetch the next channel start without narrowing its host value.
    fn next_channel_start(
        &mut self,
        starts: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error>;
    /// Evaluate `histogram[start:start + 256]` with host operators.
    fn histogram_channel(
        &mut self,
        histogram: &Self::Value,
        start: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Test `isinstance(ignore, int)` through the host boundary.
    fn ignore_is_integer(&mut self, ignore: &Self::Value) -> Result<bool, Self::Error>;
    /// Create a fresh iterator for one channel's ignored-bin traversal.
    fn ignore_indices(
        &mut self,
        ignore: &Self::Value,
    ) -> Result<Self::Iterator, Self::Error>;
    /// Fetch the next ignored index from the current channel's iterator.
    fn next_ignore_index(
        &mut self,
        indices: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error>;
    /// Assign zero to one bin using the host's item-assignment protocol.
    fn zero_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Read one histogram bin through the host's item-access protocol.
    fn histogram_bin(
        &mut self,
        channel: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error>;
    /// Read one cutoff component through the host's item-access protocol.
    fn cutoff_component(
        &mut self,
        cutoff: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply the host's ordinary addition operation.
    fn add(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply the host's multiplication operation.
    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply floor division by the Python integer literal 100.
    fn floor_divide_by_hundred(
        &mut self,
        value: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply the host's `int` conversion.
    fn int_value(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Apply the host's greater-than comparison and truthiness protocol.
    fn greater_than(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<bool, Self::Error>;
    /// Apply ordinary subtraction.
    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply `channel[index] -= value` through the host's in-place protocol.
    fn subtract_from_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: usize,
        value: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Set one bin to the Python integer literal zero.
    fn clear_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: usize,
    ) -> Result<(), Self::Error>;
    /// Test `cutoff <= 0` using host comparison and truthiness.
    fn cutoff_is_nonpositive(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error>;
    /// Test one bin's truthiness through the host protocol.
    fn histogram_bin_is_truthy(
        &mut self,
        channel: &Self::Value,
        index: usize,
    ) -> Result<bool, Self::Error>;
    /// Create the Python integer literal zero.
    fn zero_value(&mut self) -> Result<Self::Value, Self::Error>;
    /// Evaluate cutoff truthiness for the current channel.
    fn cutoff_is_truthy(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error>;
    /// Test `isinstance(cutoff, tuple)` through the host boundary.
    fn cutoff_is_tuple(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error>;
    /// Build the literal `(cutoff, cutoff)` tuple used after the first channel.
    fn duplicate_cutoff(&mut self, cutoff: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Extend the output LUT using the host list operation.
    fn extend_lut(
        &mut self,
        lut: &mut Self::Value,
        channel_lut: &[u8],
    ) -> Result<(), Self::Error>;
}

/// Builds ImageOps Autocontrast's channel LUT using Rust-owned orchestration.
///
/// Callbacks preserve the wrapper's observable order: create the output list,
/// evaluate `range(0, len(histogram), 256)`, slice each channel, apply ignored
/// bins, evaluate and normalize cutoff, build that channel's LUT, then extend
/// the output. The host supplies Python sequence and numeric protocols; this
/// function owns channel iteration and cutoff state across channels.
// TODO(pillow-rs#3ptb): freeze callback fault order and selected
// mode/architecture receipts in the CPU parity matrix.
#[doc(hidden)]
pub fn autocontrast_lut_with_callbacks<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    histogram: &C::Value,
    mut cutoff: C::Value,
    ignore: Option<C::Value>,
) -> Result<C::Value, C::Error> {
    let mut lut = callbacks.new_lut()?;
    let mut starts = callbacks.channel_starts(histogram)?;
    while let Some(start) = callbacks.next_channel_start(&mut starts)? {
        let mut channel = callbacks.histogram_channel(histogram, &start)?;
        if let Some(ignore) = ignore.as_ref() {
            if callbacks.ignore_is_integer(ignore)? {
                callbacks.zero_histogram_bin(&mut channel, ignore)?;
            } else {
                let mut indices = callbacks.ignore_indices(ignore)?;
                while let Some(index) = callbacks.next_ignore_index(&mut indices)? {
                    callbacks.zero_histogram_bin(&mut channel, &index)?;
                }
            }
        }
        let channel_cutoff = if callbacks.cutoff_is_truthy(&cutoff)? {
            if !callbacks.cutoff_is_tuple(&cutoff)? {
                cutoff = callbacks.duplicate_cutoff(&cutoff)?;
            }
            Some(cutoff.clone())
        } else {
            None
        };
        let channel_lut = autocontrast_channel_lut_with_callbacks(
            callbacks,
            &mut channel,
            channel_cutoff.as_ref(),
        )?;
        callbacks.extend_lut(&mut lut, &channel_lut)?;
    }
    Ok(lut)
}

fn autocontrast_channel_lut_with_callbacks<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &mut C::Value,
    cutoff: Option<&C::Value>,
) -> Result<[u8; 256], C::Error> {
    if let Some(cutoff) = cutoff {
        let total = autocontrast_histogram_total(callbacks, channel)?;
        let low = autocontrast_cutoff_count(callbacks, &total, cutoff, 0)?;
        trim_autocontrast_edge(callbacks, channel, low, false)?;
        let high = autocontrast_cutoff_count(callbacks, &total, cutoff, 1)?;
        trim_autocontrast_edge(callbacks, channel, high, true)?;
    }
    let low = autocontrast_edge(callbacks, channel, false)?;
    let high = autocontrast_edge(callbacks, channel, true)?;
    let mut lut = [0; 256];
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::
        fill_autocontrast_lut_from_bounds(&mut lut, low, high);
    Ok(lut)
}

fn autocontrast_histogram_total<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &C::Value,
) -> Result<C::Value, C::Error> {
    let mut total = callbacks.zero_value()?;
    for index in 0..256 {
        let bin = callbacks.histogram_bin(channel, index)?;
        total = callbacks.add(&total, &bin)?;
    }
    Ok(total)
}

fn autocontrast_cutoff_count<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    total: &C::Value,
    cutoff: &C::Value,
    index: usize,
) -> Result<C::Value, C::Error> {
    let cutoff = callbacks.cutoff_component(cutoff, index)?;
    let product = callbacks.multiply(total, &cutoff)?;
    let percentage = callbacks.floor_divide_by_hundred(&product)?;
    callbacks.int_value(&percentage)
}

fn trim_autocontrast_edge<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &mut C::Value,
    mut cutoff: C::Value,
    high: bool,
) -> Result<(), C::Error> {
    let mut index = if high { 255 } else { 0 };
    loop {
        let bin = callbacks.histogram_bin(channel, index)?;
        if callbacks.greater_than(&cutoff, &bin)? {
            let bin = callbacks.histogram_bin(channel, index)?;
            cutoff = callbacks.subtract(&cutoff, &bin)?;
            callbacks.clear_histogram_bin(channel, index)?;
        } else {
            callbacks.subtract_from_histogram_bin(channel, index, &cutoff)?;
            cutoff = callbacks.zero_value()?;
        }
        if callbacks.cutoff_is_nonpositive(&cutoff)? {
            break;
        }
        if (high && index == 0) || (!high && index == 255) {
            break;
        }
        index = if high { index - 1 } else { index + 1 };
    }
    Ok(())
}

fn autocontrast_edge<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &C::Value,
    high: bool,
) -> Result<usize, C::Error> {
    let mut index = if high { 255 } else { 0 };
    loop {
        if callbacks.histogram_bin_is_truthy(channel, index)? {
            return Ok(index);
        }
        if (high && index == 0) || (!high && index == 255) {
            return Ok(index);
        }
        index = if high { index - 1 } else { index + 1 };
    }
}

/// Completes ImageOps Autocontrast after the host has built per-channel LUTs.
///
/// Mode admission uses the current typed image identity. The Python adapter
/// retains only host sequence operations and the public `Image.point` call.
#[doc(hidden)]
pub fn autocontrast_result_from_lut_with_callbacks<C: ImageOpsAutocontrastResultCallbacks>(
    mode: Mode,
    callbacks: &mut C,
    lut: C::Lut,
) -> Result<C::Output, C::Error> {
    if mode == Mode::ImagePalette8 {
        return Err(PilError::NotImplementedError("mode P support coming soon".into()).into());
    }
    match OperationId::Autocontrast.support().for_mode(mode) {
        ModeSupport::Supported {
            scope: SupportScope::Pillow,
            ..
        } => {}
        ModeSupport::Supported {
            scope: SupportScope::CoreOnly { .. },
            ..
        }
        | ModeSupport::Rejected { .. } => {
            return Err(callbacks.unsupported_mode_error()?);
        }
    }
    let lut = if mode == Mode::ImageRgb8 && callbacks.lut_length(&lut)? == 256 {
        callbacks.repeat_lut(lut)?
    } else {
        lut
    };
    callbacks.point(lut)
}

/// Runs the ImageOps Autocontrast wrapper flow around its host protocols.
///
/// Rust selects whether to request the public luma conversion, then sequences
/// histogram collection, LUT traversal, typed mode admission, and `Image.point`.
/// The adapters retain host truthiness, conversion, histogram, sequence, and
/// point calls.
#[doc(hidden)]
pub fn autocontrast_with_callbacks<S, L, R>(
    source_callbacks: &mut S,
    lut_callbacks: &mut L,
    result_callbacks: &mut R,
    image: S::Image,
    mask: S::Mask,
    preserve_tone: S::PreserveTone,
    cutoff: L::Value,
    ignore: Option<L::Value>,
) -> Result<R::Output, S::Error>
where
    S: ImageOpsAutocontrastSourceCallbacks,
    L: ImageOpsAutocontrastCallbacks<Value = S::Value, Error = S::Error>,
    R: ImageOpsAutocontrastResultCallbacks<Lut = S::Value, Error = S::Error>,
{
    let preserve_tone = source_callbacks.preserve_tone_is_truthy(&preserve_tone)?;
    let converted;
    let histogram_image = if preserve_tone {
        converted = source_callbacks.convert_to_luma(&image)?;
        &converted
    } else {
        &image
    };
    let histogram = source_callbacks.histogram(histogram_image, &mask)?;
    let lut = autocontrast_lut_with_callbacks(
        lut_callbacks,
        &histogram,
        cutoff,
        ignore,
    )?;
    let mode = source_callbacks.mode(&image)?;
    autocontrast_result_from_lut_with_callbacks(mode, result_callbacks, lut)
}

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
#[cfg(test)]
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

use super::{ImageOpsMask, validate_imageops_mask};
use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{AutocontrastOptions, PipelineOp};
use crate::raster::Mode;
use std::sync::Arc;

/// Host operations needed after ImageOps Autocontrast has built its LUT.
#[doc(hidden)]
pub trait ImageOpsAutocontrastResultCallbacks {
    /// Host value carrying the completed lookup table.
    type Lut;
    /// Result returned by the public host `Image.point` method.
    type Output;
    /// Host or core error, translated at the binding boundary.
    type Error: From<PilError>;

    /// Test Pillow's `image.mode == "P"` branch after LUT construction.
    fn image_mode_is_palette(&mut self) -> Result<bool, Self::Error>;
    /// Test Pillow's `image.mode in ("L", "RGB")` branch.
    fn image_mode_is_luma_or_rgb(&mut self) -> Result<bool, Self::Error>;
    /// Test Pillow's `image.mode == "RGB"` before reading LUT length.
    fn image_mode_is_rgb(&mut self) -> Result<bool, Self::Error>;
    /// Return Pillow's `NotImplementedError` for mode P.
    fn palette_mode_error(&mut self) -> Result<Self::Error, Self::Error>;
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
    fn preserve_tone_is_truthy(&mut self, value: &Self::PreserveTone) -> Result<bool, Self::Error>;
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

    /// Whether the exact built-in channel and LUT values permit the native
    /// 0..255 loops without bypassing a replaced ImageOps global.
    fn native_lut_path_is_safe(
        &mut self,
        channel: &Self::Value,
        cutoff: Option<&Self::Value>,
    ) -> Result<bool, Self::Error>;

    /// Create the output LUT before evaluating the channel range.
    fn new_lut(&mut self) -> Result<Self::Value, Self::Error>;
    /// Evaluate `range(0, len(histogram), 256)` in host lookup order.
    fn channel_starts(&mut self, histogram: &Self::Value) -> Result<Self::Iterator, Self::Error>;
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
    fn ignore_indices(&mut self, ignore: &Self::Value) -> Result<Self::Iterator, Self::Error>;
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
    /// Evaluate `channel[index]` for an index yielded by a replaced `range`.
    fn histogram_bin_at(
        &mut self,
        channel: &Self::Value,
        index: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    /// Sum one histogram channel using the host sequence's iteration protocol.
    fn sum_histogram_channel(&mut self, channel: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Read one cutoff component through the host's item-access protocol.
    fn cutoff_component(
        &mut self,
        cutoff: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply the host's multiplication operation.
    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Apply floor division by the Python integer literal 100.
    fn floor_divide_by_hundred(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Resolve the module-level `int` callable at the source expression point.
    fn int_function(&mut self) -> Result<Self::Value, Self::Error>;
    /// Call an already-resolved `int` after its argument expression is evaluated.
    fn call_int_function(
        &mut self,
        function: &Self::Value,
        value: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
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
    /// Apply host addition for the float LUT formula.
    fn add_values(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Evaluate `255.0 / value` using host true-division semantics.
    fn divide_float_255_by(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Apply host unary negation.
    fn negate_value(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Test a LUT value's `< 0` branch with host comparison and truthiness.
    fn lut_value_is_negative(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Test a LUT value's `> 255` branch with host comparison and truthiness.
    fn lut_value_exceeds_byte(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Evaluate `range(256)` or `range(255, -1, -1)` through the module global.
    fn lut_scan_indices(&mut self, reverse: bool) -> Result<Self::Iterator, Self::Error>;
    /// Preserve loop-local values and compare `hi <= lo` in host order.
    fn non_identity_lut_bounds(
        &mut self,
        high: Option<&ImageOpsAutocontrastLoopIndex<Self::Value>>,
        low: Option<&ImageOpsAutocontrastLoopIndex<Self::Value>>,
    ) -> Result<Option<(Self::Value, Self::Value)>, Self::Error>;
    /// Append one host value to the output LUT.
    fn append_lut_value(
        &mut self,
        lut: &mut Self::Value,
        value: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Evaluate and extend with `list(range(256))` in host lookup order.
    fn extend_identity_lut(&mut self, lut: &mut Self::Value) -> Result<(), Self::Error>;
    /// Apply `channel[index] -= value` through the host's in-place protocol.
    fn subtract_from_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: usize,
        value: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Evaluate `channel[index] -= value` for a dynamically yielded index.
    fn subtract_from_histogram_bin_at(
        &mut self,
        channel: &mut Self::Value,
        index: &Self::Value,
        value: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Set one bin to the Python integer literal zero.
    fn clear_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: usize,
    ) -> Result<(), Self::Error>;
    /// Set a dynamically indexed histogram bin to zero.
    fn clear_histogram_bin_at(
        &mut self,
        channel: &mut Self::Value,
        index: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Test `cutoff <= 0` using host comparison and truthiness.
    fn cutoff_is_nonpositive(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error>;
    /// Test one bin's truthiness through the host protocol.
    fn histogram_bin_is_truthy(
        &mut self,
        channel: &Self::Value,
        index: usize,
    ) -> Result<bool, Self::Error>;
    /// Test a dynamically indexed histogram bin's truthiness.
    fn histogram_bin_at_is_truthy(
        &mut self,
        channel: &Self::Value,
        index: &Self::Value,
    ) -> Result<bool, Self::Error>;
    /// Create the Python integer literal zero.
    fn zero_value(&mut self) -> Result<Self::Value, Self::Error>;
    /// Create the Python integer literal 255.
    fn byte_maximum_value(&mut self) -> Result<Self::Value, Self::Error>;
    /// Evaluate cutoff truthiness for the current channel.
    fn cutoff_is_truthy(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error>;
    /// Test `isinstance(cutoff, tuple)` through the host boundary.
    fn cutoff_is_tuple(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error>;
    /// Build the literal `(cutoff, cutoff)` tuple used after the first channel.
    fn duplicate_cutoff(&mut self, cutoff: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Extend the output LUT using the host list operation.
    fn extend_lut(&mut self, lut: &mut Self::Value, channel_lut: &[u8]) -> Result<(), Self::Error>;
}

/// A Pillow loop variable captured without allocating native integer values.
#[doc(hidden)]
pub enum ImageOpsAutocontrastLoopIndex<Value> {
    /// Index assigned by the unmodified built-in `range` fast path.
    Native(usize),
    /// Value yielded by a replaced `range` function.
    Host(Value),
}

/// Builds ImageOps Autocontrast's channel LUT using Rust-owned orchestration.
///
/// Callbacks preserve the wrapper's observable order: create the output list,
/// evaluate `range(0, len(histogram), 256)`, slice each channel, apply ignored
/// bins, evaluate and normalize cutoff, then write each channel's LUT into the
/// output. The host supplies Python sequence and numeric protocols; this
/// function owns channel iteration and cutoff state across channels.
// TODO(pillow-rs#3ptb): freeze custom-range/list/index faults, callback order,
// and mode/architecture receipts for the host-protocol fallback.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub fn autocontrast_lut_with_callbacks<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    histogram: &C::Value,
    mut cutoff: C::Value,
    ignore: Option<C::Value>,
) -> Result<C::Value, C::Error> {
    let mut lut = callbacks.new_lut()?;
    let mut starts = callbacks.channel_starts(histogram)?;
    // Pillow's `lo` and `hi` locals persist across range scans and channels.
    let mut low_index: Option<ImageOpsAutocontrastLoopIndex<C::Value>> = None;
    let mut high_index: Option<ImageOpsAutocontrastLoopIndex<C::Value>> = None;
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
        autocontrast_channel_lut_with_callbacks(
            callbacks,
            &mut channel,
            channel_cutoff.as_ref(),
            &mut lut,
            &mut low_index,
            &mut high_index,
        )?;
    }
    Ok(lut)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_channel_lut_with_callbacks<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &mut C::Value,
    cutoff: Option<&C::Value>,
    output: &mut C::Value,
    low_index: &mut Option<ImageOpsAutocontrastLoopIndex<C::Value>>,
    high_index: &mut Option<ImageOpsAutocontrastLoopIndex<C::Value>>,
) -> Result<(), C::Error> {
    if callbacks.native_lut_path_is_safe(channel, cutoff)? {
        let channel_lut =
            autocontrast_channel_native_lut(callbacks, channel, cutoff, low_index, high_index)?;
        return callbacks.extend_lut(output, &channel_lut);
    }

    if let Some(cutoff) = cutoff {
        let total = autocontrast_histogram_total(callbacks, channel)?;
        let low = autocontrast_cutoff_count(callbacks, &total, cutoff, 0)?;
        trim_autocontrast_edge_with_host_range(callbacks, channel, low, false, low_index)?;
        let high = autocontrast_cutoff_count(callbacks, &total, cutoff, 1)?;
        trim_autocontrast_edge_with_host_range(callbacks, channel, high, true, high_index)?;
    }

    autocontrast_edge_with_host_range(callbacks, channel, false, low_index)?;
    autocontrast_edge_with_host_range(callbacks, channel, true, high_index)?;
    let Some((high, low)) =
        callbacks.non_identity_lut_bounds(high_index.as_ref(), low_index.as_ref())?
    else {
        return callbacks.extend_identity_lut(output);
    };
    let span = callbacks.subtract(&high, &low)?;
    let scale = callbacks.divide_float_255_by(&span)?;
    let negative_low = callbacks.negate_value(&low)?;
    let offset = callbacks.multiply(&negative_low, &scale)?;
    append_autocontrast_lut_with_host_range(callbacks, output, &scale, &offset)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_channel_native_lut<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &mut C::Value,
    cutoff: Option<&C::Value>,
    low_index: &mut Option<ImageOpsAutocontrastLoopIndex<C::Value>>,
    high_index: &mut Option<ImageOpsAutocontrastLoopIndex<C::Value>>,
) -> Result<[u8; 256], C::Error> {
    if let Some(cutoff) = cutoff {
        let total = autocontrast_histogram_total(callbacks, channel)?;
        let low = autocontrast_cutoff_count(callbacks, &total, cutoff, 0)?;
        trim_autocontrast_edge(callbacks, channel, low, false, low_index)?;
        let high = autocontrast_cutoff_count(callbacks, &total, cutoff, 1)?;
        trim_autocontrast_edge(callbacks, channel, high, true, high_index)?;
    }
    let low = autocontrast_edge(callbacks, channel, false, low_index)?;
    let high = autocontrast_edge(callbacks, channel, true, high_index)?;
    let mut lut = [0; 256];
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::fill_autocontrast_lut_from_bounds(
        &mut lut, low, high,
    );
    Ok(lut)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_histogram_total<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &C::Value,
) -> Result<C::Value, C::Error> {
    callbacks.sum_histogram_channel(channel)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_cutoff_count<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    total: &C::Value,
    cutoff: &C::Value,
    index: usize,
) -> Result<C::Value, C::Error> {
    let int = callbacks.int_function()?;
    let cutoff = callbacks.cutoff_component(cutoff, index)?;
    let product = callbacks.multiply(total, &cutoff)?;
    let percentage = callbacks.floor_divide_by_hundred(&product)?;
    callbacks.call_int_function(&int, &percentage)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn trim_autocontrast_edge_with_host_range<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &mut C::Value,
    mut cutoff: C::Value,
    high: bool,
    last_index: &mut Option<ImageOpsAutocontrastLoopIndex<C::Value>>,
) -> Result<(), C::Error> {
    let mut indices = callbacks.lut_scan_indices(high)?;
    while let Some(index) = callbacks.next_ignore_index(&mut indices)? {
        *last_index = Some(ImageOpsAutocontrastLoopIndex::Host(index.clone()));
        let bin = callbacks.histogram_bin_at(channel, &index)?;
        if callbacks.greater_than(&cutoff, &bin)? {
            let bin = callbacks.histogram_bin_at(channel, &index)?;
            cutoff = callbacks.subtract(&cutoff, &bin)?;
            callbacks.clear_histogram_bin_at(channel, &index)?;
        } else {
            callbacks.subtract_from_histogram_bin_at(channel, &index, &cutoff)?;
            cutoff = callbacks.zero_value()?;
        }
        if callbacks.cutoff_is_nonpositive(&cutoff)? {
            break;
        }
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_edge_with_host_range<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &C::Value,
    high: bool,
    last_index: &mut Option<ImageOpsAutocontrastLoopIndex<C::Value>>,
) -> Result<(), C::Error> {
    let mut indices = callbacks.lut_scan_indices(high)?;
    while let Some(index) = callbacks.next_ignore_index(&mut indices)? {
        *last_index = Some(ImageOpsAutocontrastLoopIndex::Host(index.clone()));
        if callbacks.histogram_bin_at_is_truthy(channel, &index)? {
            break;
        }
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn append_autocontrast_lut_with_host_range<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    output: &mut C::Value,
    scale: &C::Value,
    offset: &C::Value,
) -> Result<(), C::Error> {
    let mut indices = callbacks.lut_scan_indices(false)?;
    while let Some(index) = callbacks.next_ignore_index(&mut indices)? {
        let int = callbacks.int_function()?;
        let scaled = callbacks.multiply(&index, scale)?;
        let shifted = callbacks.add_values(&scaled, offset)?;
        let mapped = callbacks.call_int_function(&int, &shifted)?;
        let mapped = if callbacks.lut_value_is_negative(&mapped)? {
            callbacks.zero_value()?
        } else if callbacks.lut_value_exceeds_byte(&mapped)? {
            callbacks.byte_maximum_value()?
        } else {
            mapped
        };
        callbacks.append_lut_value(output, &mapped)?;
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn trim_autocontrast_edge<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &mut C::Value,
    mut cutoff: C::Value,
    high: bool,
    last_index: &mut Option<ImageOpsAutocontrastLoopIndex<C::Value>>,
) -> Result<(), C::Error> {
    let mut index = if high { 255 } else { 0 };
    loop {
        *last_index = Some(ImageOpsAutocontrastLoopIndex::Native(index));
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

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_edge<C: ImageOpsAutocontrastCallbacks>(
    callbacks: &mut C,
    channel: &C::Value,
    high: bool,
    last_index: &mut Option<ImageOpsAutocontrastLoopIndex<C::Value>>,
) -> Result<usize, C::Error> {
    let mut index = if high { 255 } else { 0 };
    loop {
        *last_index = Some(ImageOpsAutocontrastLoopIndex::Native(index));
        if callbacks.histogram_bin_is_truthy(channel, index)? {
            return Ok(index);
        }
        if (high && index == 0) || (!high && index == 255) {
            return Ok(index);
        }
        index = if high { index - 1 } else { index + 1 };
    }
}

/// Completes ImageOps Autocontrast after the host has built its LUT.
///
/// Mode admission follows Pillow's public wrapper property after histogram
/// callbacks, while the host retains Python comparison and error behavior.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub fn autocontrast_result_from_lut_with_callbacks<C: ImageOpsAutocontrastResultCallbacks>(
    callbacks: &mut C,
    lut: C::Lut,
) -> Result<C::Output, C::Error> {
    if callbacks.image_mode_is_palette()? {
        return Err(callbacks.palette_mode_error()?);
    }
    if !callbacks.image_mode_is_luma_or_rgb()? {
        return Err(callbacks.unsupported_mode_error()?);
    }
    let mode = if callbacks.image_mode_is_rgb()? {
        Mode::ImageRgb8
    } else {
        Mode::ImageLuma8
    };
    OperationId::Autocontrast
        .support()
        .for_mode(mode)
        .validate_mode()?;
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
/// histogram collection, LUT traversal, wrapper-mode admission, and `Image.point`.
/// The adapters retain host truthiness, conversion, histogram, sequence, and
/// point calls. The Python facade scopes this sequence so its Convert,
/// Histogram, and Point wrappers receive the same captured CPU route.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
    let lut = autocontrast_lut_with_callbacks(lut_callbacks, &histogram, cutoff, ignore)?;
    autocontrast_result_from_lut_with_callbacks(result_callbacks, lut)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn mode_identity(image: &Image) -> Result<Mode, PilError> {
    image.mode_identity()
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
        autocontrast_lut_from_histogram(&histogram, options.cutoff, options.ignore.clone())?;
    }
    support.validate_mode()
}

/// Normalizes image contrast by clipping darkest and lightest values.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub fn autocontrast(image: &Image, cutoff: f64) -> Result<Image, PilError> {
    autocontrast_with_options(image, AutocontrastOptions::symmetric(cutoff))
}

/// Normalizes image contrast using independent cutoffs, ignored bins, and tone-preserving luma statistics.
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub fn autocontrast_with_options(
    image: &Image,
    options: AutocontrastOptions,
) -> Result<Image, PilError> {
    validate_autocontrast_mode_after_histogram(image, &options, None)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Autocontrast {
            options,
            mask: None,
        },
    ))
}

/// Normalizes contrast after validating an optional Pillow mask.
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub fn autocontrast_with_mask(
    image: &Image,
    cutoff: f64,
    mask: ImageOpsMask,
) -> Result<Image, PilError> {
    autocontrast_with_options_and_mask(image, AutocontrastOptions::symmetric(cutoff), mask)
}

/// Normalizes image contrast using typed options after validating an optional Pillow mask.
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
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
// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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

use std::sync::Arc;

use crate::compute::registry::support::{ModeSupport, OperationId, OutputModeRule};
use crate::error::PilError;
use crate::image::Image;
use crate::ops::imageops::{ImageOpsMask, validate_imageops_mask};
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

/// Host operations used by the ImageOps Equalize wrapper strategy.
#[doc(hidden)]
pub trait ImageOpsEqualizeCallbacks {
    /// Host image wrapper, including subclasses and public methods.
    type Image;
    /// Host histogram mask argument.
    type Mask;
    /// Host histogram result.
    type Histogram;
    /// Host values returned while traversing an arbitrary histogram.
    type Value: Clone;
    /// Host sequence used for channel slices and filtered nonzero counts.
    type Sequence;
    /// Host iterator used for histogram and channel traversal.
    type Iterator;
    /// Host or native lookup table passed to `Image.point`.
    type Lut;
    /// Public host `Image.point` result.
    type Output;
    /// Host or core error, translated at the binding boundary.
    type Error: From<PilError>;

    /// Evaluate Pillow's initial `image.mode == "P"` wrapper branch.
    fn image_mode_is_palette(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Evaluate the post-LUT `_lut` `image.mode == "P"` branch.
    fn lut_mode_is_palette(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Evaluate the post-LUT `_lut` membership check against L and RGB.
    fn lut_mode_is_luma_or_rgb(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Evaluate the post-LUT `_lut` `image.mode == "RGB"` branch.
    fn lut_mode_is_rgb(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Return Pillow's `_lut` `NotImplementedError` for mode P.
    fn palette_mode_error(&mut self) -> Result<Self::Error, Self::Error>;
    /// Return Pillow's unsupported-mode error for an unrecognized wrapper mode.
    fn unsupported_mode_error(&mut self, image: &Self::Image) -> Result<Self::Error, Self::Error>;
    /// Convert the palette wrapper to RGB before histogram collection.
    fn convert_to_rgb(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error>;
    /// Call the current wrapper's public histogram method.
    fn histogram(
        &mut self,
        image: &Self::Image,
        mask: &Self::Mask,
    ) -> Result<Self::Histogram, Self::Error>;
    /// Return a safe native fast-path histogram when host protocols are inert.
    fn native_histogram(
        &mut self,
        histogram: &Self::Histogram,
    ) -> Result<Option<Vec<u32>>, Self::Error>;
    /// Create the output LUT before channel range lookup.
    fn new_lut(&mut self) -> Result<Self::Lut, Self::Error>;
    /// Evaluate `range(0, len(histogram), 256)` through the host.
    fn channel_starts(
        &mut self,
        histogram: &Self::Histogram,
    ) -> Result<Self::Iterator, Self::Error>;
    /// Fetch the next channel start from the host iterator.
    fn next_channel_start(
        &mut self,
        starts: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error>;
    /// Evaluate the source slice for one 256-value histogram channel.
    fn histogram_channel(
        &mut self,
        histogram: &Self::Histogram,
        start: &Self::Value,
    ) -> Result<Self::Sequence, Self::Error>;
    /// Iterate one source channel using its host iteration protocol.
    fn channel_values(&mut self, channel: &Self::Sequence) -> Result<Self::Iterator, Self::Error>;
    /// Fetch the next host value from a channel iterator.
    fn next_channel_value(
        &mut self,
        values: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error>;
    /// Evaluate one histogram value's host truthiness.
    fn value_is_truthy(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Create the list populated by Equalize's nonzero-bin comprehension.
    fn new_nonzero_values(&mut self) -> Result<Self::Sequence, Self::Error>;
    /// Append one truthy histogram value to the nonzero-bin list.
    fn append_nonzero_value(
        &mut self,
        values: &mut Self::Sequence,
        value: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Return `len(nonzero_values)` through the host namespace.
    fn nonzero_value_count(&mut self, values: &Self::Sequence) -> Result<Self::Value, Self::Error>;
    /// Evaluate `len(nonzero_values) <= 1` using host comparison semantics.
    fn nonzero_value_count_at_most_one(&mut self, count: &Self::Value)
    -> Result<bool, Self::Error>;
    /// Evaluate the host's `sum(nonzero_values)` call.
    fn sum_nonzero_values(&mut self, values: &Self::Sequence) -> Result<Self::Value, Self::Error>;
    /// Evaluate `nonzero_values[-1]` through the host sequence protocol.
    fn last_nonzero_value(&mut self, values: &Self::Sequence) -> Result<Self::Value, Self::Error>;
    /// Evaluate `left - right` through the host numeric protocol.
    fn subtract_values(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Evaluate `value // divisor` through the host numeric protocol.
    fn floor_divide_constant(
        &mut self,
        value: &Self::Value,
        divisor: i64,
    ) -> Result<Self::Value, Self::Error>;
    /// Evaluate `left // right` through the host numeric protocol.
    fn floor_divide_values(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Create the host iterator for `range(256)`.
    fn channel_offsets(&mut self) -> Result<Self::Iterator, Self::Error>;
    /// Fetch the next offset from `range(256)`.
    fn next_channel_offset(
        &mut self,
        offsets: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error>;
    /// Evaluate `histogram[offset + start]` in the original index order.
    fn histogram_value(
        &mut self,
        histogram: &Self::Histogram,
        index: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Evaluate `left + right` through the host numeric protocol.
    fn add_values(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Append one computed LUT value through the output list operation.
    fn append_lut_value(
        &mut self,
        lut: &mut Self::Lut,
        value: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Extend the output with the result of `list(range(256))`.
    fn extend_identity_lut(&mut self, lut: &mut Self::Lut) -> Result<(), Self::Error>;
    /// Wrap the native Rust LUT for the host `Image.point` adapter.
    fn native_lut(&mut self, lut: Vec<u8>) -> Result<Self::Lut, Self::Error>;
    /// Apply the LUT through the current wrapper's public point method.
    fn point(&mut self, image: &Self::Image, lut: Self::Lut) -> Result<Self::Output, Self::Error>;
}

/// Runs Pillow's ImageOps Equalize wrapper flow through host callbacks.
///
/// The operation support output rule selects the palette-to-RGB conversion
/// before the public histogram callback. After building the LUT, separate
/// wrapper-mode callbacks reproduce Pillow's `_lut` checks before public `point`.
// TODO(pillow-rs#3ptb): freeze wrapper output, mask fault order, mode-property
// changes across `_lut` checks, formatting faults, and terminal selection
// receipts.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
#[doc(hidden)]
pub fn equalize_with_callbacks<C: ImageOpsEqualizeCallbacks>(
    callbacks: &mut C,
    image: C::Image,
    mask: C::Mask,
) -> Result<C::Output, C::Error> {
    let image = if callbacks.image_mode_is_palette(&image)?
        && matches!(
            OperationId::Equalize
                .support()
                .for_mode(Mode::ImagePalette8),
            ModeSupport::Supported {
                output: OutputModeRule::Fixed(Mode::ImageRgb8),
                ..
            }
        ) {
        callbacks.convert_to_rgb(&image)?
    } else {
        image
    };
    let histogram = callbacks.histogram(&image, &mask)?;
    let lut = equalize_lut_with_callbacks(callbacks, &histogram)?;
    if callbacks.lut_mode_is_palette(&image)? {
        return Err(callbacks.palette_mode_error()?);
    }
    if !callbacks.lut_mode_is_luma_or_rgb(&image)? {
        return Err(callbacks.unsupported_mode_error(&image)?);
    }
    let mode = if callbacks.lut_mode_is_rgb(&image)? {
        Mode::ImageRgb8
    } else {
        Mode::ImageLuma8
    };
    OperationId::Equalize
        .support()
        .for_mode(mode)
        .validate_mode()?;
    callbacks.point(&image, lut)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn equalize_lut_with_callbacks<C: ImageOpsEqualizeCallbacks>(
    callbacks: &mut C,
    histogram: &C::Histogram,
) -> Result<C::Lut, C::Error> {
    if let Some(native_histogram) = callbacks.native_histogram(histogram)?
        && let Some(lut) =
            crate::compute::pool_cpu::ops::imageops::helpers::histogram::equalize_lut_from_histogram(
                &native_histogram,
            )
    {
        return callbacks.native_lut(lut);
    }

    let mut lut = callbacks.new_lut()?;
    let mut starts = callbacks.channel_starts(histogram)?;
    while let Some(start) = callbacks.next_channel_start(&mut starts)? {
        let channel = callbacks.histogram_channel(histogram, &start)?;
        let mut channel_values = callbacks.channel_values(&channel)?;
        let mut nonzero_values = callbacks.new_nonzero_values()?;
        while let Some(value) = callbacks.next_channel_value(&mut channel_values)? {
            if callbacks.value_is_truthy(&value)? {
                callbacks.append_nonzero_value(&mut nonzero_values, &value)?;
            }
        }

        let nonzero_count = callbacks.nonzero_value_count(&nonzero_values)?;
        if callbacks.nonzero_value_count_at_most_one(&nonzero_count)? {
            callbacks.extend_identity_lut(&mut lut)?;
            continue;
        }

        let total = callbacks.sum_nonzero_values(&nonzero_values)?;
        let last = callbacks.last_nonzero_value(&nonzero_values)?;
        let remaining = callbacks.subtract_values(&total, &last)?;
        let step = callbacks.floor_divide_constant(&remaining, 255)?;
        if !callbacks.value_is_truthy(&step)? {
            callbacks.extend_identity_lut(&mut lut)?;
            continue;
        }

        let mut cumulative = callbacks.floor_divide_constant(&step, 2)?;
        let mut offsets = callbacks.channel_offsets()?;
        while let Some(offset) = callbacks.next_channel_offset(&mut offsets)? {
            let output = callbacks.floor_divide_values(&cumulative, &step)?;
            callbacks.append_lut_value(&mut lut, &output)?;
            let index = callbacks.add_values(&offset, &start)?;
            let count = callbacks.histogram_value(histogram, &index)?;
            cumulative = callbacks.add_values(&cumulative, &count)?;
        }
    }
    Ok(lut)
}

/// Equalizes the image histogram.
///
/// # Errors
///
/// Returns [`PilError::OsError`] for alpha modes that Pillow does not support,
/// or another [`PilError`] when mode detection fails.
// TODO(pillow-rs#3ptb): Verify this lazy Rust API's mode/architecture route,
// output, and fallback receipts in the frozen CPU matrix.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub fn equalize(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode_identity()?;
    OperationId::Equalize
        .support()
        .for_mode(mode)
        .validate_mode()?;
    Ok(Image::push_op(image, PipelineOp::Equalize))
}

/// Builds the Equalize lookup table from counts collected by a host wrapper.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
#[doc(hidden)]
pub fn equalize_lut_from_histogram(image: &Image, histogram: &[u32]) -> Result<Vec<u8>, PilError> {
    let lut =
        crate::compute::pool_cpu::ops::imageops::helpers::histogram::equalize_lut_from_histogram(
            histogram,
        )
        .ok_or_else(|| PilError::ValueError("invalid Equalize histogram length".into()))?;
    let mode = image.mode_identity()?;
    OperationId::Equalize
        .support()
        .for_mode(mode)
        .validate_mode()?;
    Ok(lut)
}

/// Equalizes an image after validating an optional Pillow mask.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub fn equalize_with_mask(image: &Image, mask: ImageOpsMask) -> Result<Image, PilError> {
    match mask {
        ImageOpsMask::None => equalize(image),
        ImageOpsMask::Invalid(type_name) => Err(PilError::AttributeError(format!(
            "'{type_name}' object has no attribute 'load'"
        ))),
        ImageOpsMask::Image(mask) => {
            validate_imageops_mask(image, ImageOpsMask::Image(mask.clone()))?;
            let mode = image.mode_identity()?;
            OperationId::Equalize
                .support()
                .for_mode(mode)
                .validate_mode()?;
            Ok(Image::push_op(
                image,
                PipelineOp::EqualizeMasked {
                    mask: Arc::new(*mask),
                },
            ))
        }
    }
}

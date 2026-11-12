use crate::compute::registry::support::OperationId;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

/// Host operations used by the ImageOps Colorize wrapper strategy.
#[doc(hidden)]
pub trait ImageOpsColorizeCallbacks: crate::ops::imageops::ImageOpsLutCallbacks {
    /// Python values used by colors, point bounds, and arithmetic.
    type Value;
    /// Python-compatible channel list used to assemble the LUT.
    type Sequence;
    /// Range-like value created for a Colorize segment.
    type Range;
    /// Iterator produced from a segment range.
    type Iterator;
    /// Evaluate Pillow's initial `image.mode == "L"` wrapper comparison.
    fn image_mode_is_luma(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Return `str(image.mode)` after the wrapper rejects a non-`L` mode.
    fn image_mode_name(&mut self, image: &Self::Image) -> Result<String, Self::Error>;
    /// Test `0 <= value` with the host's rich-comparison protocol.
    fn point_is_nonnegative(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Test one ordered point comparison with the host protocol.
    fn point_is_at_most(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<bool, Self::Error>;
    /// Test `value <= 255` with the host protocol.
    fn point_is_at_most_255(&mut self, value: &Self::Value) -> Result<bool, Self::Error>;
    /// Resolve string colors through Pillow's ImageColor wrapper when needed.
    fn resolve_color(&mut self, color: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Construct a host integer literal used by Colorize's expressions.
    fn integer(&mut self, value: i64) -> Result<Self::Value, Self::Error>;
    /// Create one of Colorize's local channel lists.
    fn new_channel(&mut self) -> Result<Self::Sequence, Self::Error>;
    /// Append one host value to a channel list.
    fn append(
        &mut self,
        channel: &mut Self::Sequence,
        value: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Call the wrapper's `range(stop)` for one Colorize segment.
    fn range(&mut self, stop: &Self::Value) -> Result<Self::Range, Self::Error>;
    /// Iterate the range object with its host protocol.
    fn range_values(&mut self, range: &Self::Range) -> Result<Self::Iterator, Self::Error>;
    /// Return the next host value from a range iterator.
    fn next_range_value(
        &mut self,
        values: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error>;
    /// Evaluate `len(range)` at each point where Pillow evaluates it.
    fn range_length(&mut self, range: &Self::Range) -> Result<Self::Value, Self::Error>;
    /// Index a color sequence using the original Python order.
    fn component(&mut self, color: &Self::Value, index: usize) -> Result<Self::Value, Self::Error>;
    /// Evaluate one host subtraction.
    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Evaluate one host multiplication.
    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Evaluate Python floor division.
    fn floor_divide(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Evaluate one host addition.
    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Concatenate `red + green + blue` in source order.
    fn combine_channels(
        &mut self,
        red: Self::Sequence,
        green: Self::Sequence,
        blue: Self::Sequence,
    ) -> Result<Self::Lut, Self::Error>;
    /// Convert the source through public `Image.convert("RGB")`.
    fn convert_to_rgb(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error>;
}

/// Runs ImageOps Colorize with wrapper mode admission and ordered callbacks.
// TODO(pillow-rs#3ptb): freeze wrapper mode-comparison, `isinstance`/`str`
// lookup and truthiness, ImageColor resolver, color-index, arithmetic,
// Convert/Point faults, subclass Convert mode overrides, and final
// mode/output/selection receipts.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
#[doc(hidden)]
pub fn colorize_with_callbacks<C: ImageOpsColorizeCallbacks>(
    callbacks: &mut C,
    image: C::Image,
    black: C::Value,
    white: C::Value,
    mid: Option<C::Value>,
    blackpoint: C::Value,
    midpoint: C::Value,
    whitepoint: C::Value,
) -> Result<C::Output, C::Error> {
    if !callbacks.image_mode_is_luma(&image)? {
        let mode = callbacks.image_mode_name(&image)?;
        return Err(PilError::ValueError(format!("mode must be L, not {mode}")).into());
    }
    OperationId::ImageOpsColorize
        .support()
        .for_mode(Mode::ImageLuma8)
        .validate_mode()?;
    let points_valid = if mid.is_none() {
        callbacks.point_is_nonnegative(&blackpoint)?
            && callbacks.point_is_at_most(&blackpoint, &whitepoint)?
            && callbacks.point_is_at_most_255(&whitepoint)?
    } else {
        callbacks.point_is_nonnegative(&blackpoint)?
            && callbacks.point_is_at_most(&blackpoint, &midpoint)?
            && callbacks.point_is_at_most(&midpoint, &whitepoint)?
            && callbacks.point_is_at_most_255(&whitepoint)?
    };
    if !points_valid {
        let message = if mid.is_some() {
            "midpoint must be between or equal to blackpoint and whitepoint"
        } else {
            "blackpoint and whitepoint must each be between or equal to 0 and 255, with blackpoint less than or equal to whitepoint"
        };
        return Err(PilError::ValueError(message.into()).into());
    }

    let lut = colorize_lut_with_callbacks(
        callbacks,
        &black,
        &white,
        mid.as_ref(),
        &blackpoint,
        &midpoint,
        &whitepoint,
    )?;
    let image = callbacks.convert_to_rgb(&image)?;
    crate::ops::imageops::imageops_lut_with_callbacks(
        callbacks,
        &image,
        lut,
        OperationId::ImageOpsColorizeLut,
    )
}

fn colorize_lut_with_callbacks<C: ImageOpsColorizeCallbacks>(
    callbacks: &mut C,
    black: &C::Value,
    white: &C::Value,
    mid: Option<&C::Value>,
    blackpoint: &C::Value,
    midpoint: &C::Value,
    whitepoint: &C::Value,
) -> Result<C::Lut, C::Error> {
    let black = callbacks.resolve_color(black)?;
    let white = callbacks.resolve_color(white)?;
    let mid = mid
        .map(|color| callbacks.resolve_color(color))
        .transpose()?;
    let mut red = callbacks.new_channel()?;
    let mut green = callbacks.new_channel()?;
    let mut blue = callbacks.new_channel()?;

    let mut prefix = callbacks.range(blackpoint)?;
    append_endpoint(
        callbacks,
        &black,
        &mut prefix,
        &mut red,
        &mut green,
        &mut blue,
    )?;

    if let Some(mid) = mid.as_ref() {
        let first_stop = callbacks.subtract(midpoint, blackpoint)?;
        let first = callbacks.range(&first_stop)?;
        let second_stop = callbacks.subtract(whitepoint, midpoint)?;
        let second = callbacks.range(&second_stop)?;
        append_segment(
            callbacks, &black, mid, &first, &mut red, &mut green, &mut blue,
        )?;
        append_segment(
            callbacks, mid, &white, &second, &mut red, &mut green, &mut blue,
        )?;
    } else {
        let stop = callbacks.subtract(whitepoint, blackpoint)?;
        let range = callbacks.range(&stop)?;
        append_segment(
            callbacks, &black, &white, &range, &mut red, &mut green, &mut blue,
        )?;
    }

    let full_scale = callbacks.integer(256)?;
    let suffix_stop = callbacks.subtract(&full_scale, whitepoint)?;
    let mut suffix = callbacks.range(&suffix_stop)?;
    append_endpoint(
        callbacks,
        &white,
        &mut suffix,
        &mut red,
        &mut green,
        &mut blue,
    )?;
    callbacks.combine_channels(red, green, blue)
}

fn append_endpoint<C: ImageOpsColorizeCallbacks>(
    callbacks: &mut C,
    color: &C::Value,
    range: &mut C::Range,
    red: &mut C::Sequence,
    green: &mut C::Sequence,
    blue: &mut C::Sequence,
) -> Result<(), C::Error> {
    let mut values = callbacks.range_values(range)?;
    while callbacks.next_range_value(&mut values)?.is_some() {
        for (index, channel) in [&mut *red, &mut *green, &mut *blue].into_iter().enumerate() {
            let value = callbacks.component(color, index)?;
            callbacks.append(channel, &value)?;
        }
    }
    Ok(())
}

fn append_segment<C: ImageOpsColorizeCallbacks>(
    callbacks: &mut C,
    start: &C::Value,
    end: &C::Value,
    range: &C::Range,
    red: &mut C::Sequence,
    green: &mut C::Sequence,
    blue: &mut C::Sequence,
) -> Result<(), C::Error> {
    let mut values = callbacks.range_values(range)?;
    while let Some(offset) = callbacks.next_range_value(&mut values)? {
        for (index, channel) in [&mut *red, &mut *green, &mut *blue].into_iter().enumerate() {
            let value = interpolated_component(callbacks, start, end, &offset, range, index)?;
            callbacks.append(channel, &value)?;
        }
    }
    Ok(())
}

fn interpolated_component<C: ImageOpsColorizeCallbacks>(
    callbacks: &mut C,
    start: &C::Value,
    end: &C::Value,
    offset: &C::Value,
    range: &C::Range,
    index: usize,
) -> Result<C::Value, C::Error> {
    let start_value = callbacks.component(start, index)?;
    let end_value = callbacks.component(end, index)?;
    let start_for_difference = callbacks.component(start, index)?;
    let difference = callbacks.subtract(&end_value, &start_for_difference)?;
    let scaled = callbacks.multiply(offset, &difference)?;
    let length = callbacks.range_length(range)?;
    let increment = callbacks.floor_divide(&scaled, &length)?;
    callbacks.add(&start_value, &increment)
}

/// Validates that an image can be colorized.
pub fn validate_colorize_mode(image: &Image) -> Result<(), PilError> {
    validate_colorize_mode_for_mode(image.mode_identity()?)
}

fn validate_colorize_mode_for_mode(mode: Mode) -> Result<(), PilError> {
    let support = OperationId::Colorize.support().for_mode(mode);
    if support.scope().is_some() {
        return Ok(());
    }
    let mode_name = mode
        .pillow_name()
        .or_else(|| mode.rust_name())
        .unwrap_or("unknown");
    Err(PilError::ValueError(format!(
        "mode must be L, not {mode_name}"
    )))
}

/// Colorizes an `L` image by mapping black and white endpoints.
// TODO(pillow-rs#3ptb): Verify native output and mode/architecture receipts
// separately from the Python wrapper callback route.
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
        if !(blackpoint <= midpoint && midpoint <= whitepoint) {
            return Err(PilError::ValueError(
                "midpoint must be between or equal to blackpoint and whitepoint".into(),
            ));
        }
    } else if blackpoint > whitepoint {
        return Err(PilError::ValueError(
            "blackpoint and whitepoint must each be between or equal to 0 and 255, with blackpoint less than or equal to whitepoint".into(),
        ));
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

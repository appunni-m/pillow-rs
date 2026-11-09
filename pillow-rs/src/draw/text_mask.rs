use crate::error::PilError;
use crate::raster::Mode;

/// Host values needed to place a custom font mask.
#[doc(hidden)]
#[derive(Clone)]
pub struct DrawTextMaskPosition<Value> {
    /// Integer destination x coordinate before applying a font offset.
    pub x: Value,
    /// Integer destination y coordinate before applying a font offset.
    pub y: Value,
    /// Fractional start passed to the font mask method.
    pub start: Value,
}

/// Host operations used by the Rust-owned custom-font mask strategy.
#[doc(hidden)]
pub trait DrawTextMaskCallbacks {
    /// Host value carrier for coordinates, masks, fill values, and image wrappers.
    type Value: Clone;
    /// Host error carrier; core errors are translated at the binding boundary.
    type Error: From<PilError>;

    /// Convert the requested position and retain the fractional font start.
    fn position(
        &mut self,
        line: &Self::Value,
    ) -> Result<DrawTextMaskPosition<Self::Value>, Self::Error>;
    /// Resolve the selected font-mask mode, preserving host values when needed.
    fn font_mask_mode(&mut self, default: Mode) -> Result<Self::Value, Self::Error>;
    /// Apply Pillow's `mode == "RGBA"` branch to the selected host value.
    fn mode_is_rgba(&mut self, mode: &Self::Value) -> Result<bool, Self::Error>;
    /// Call `font.getmask2` with the requested Pillow arguments.
    fn getmask2(
        &mut self,
        line: &Self::Value,
        mode: &Self::Value,
        start: &Self::Value,
    ) -> Result<(Self::Value, Self::Value), Self::Error>;
    /// Call `font.getmask` with its full legacy argument list.
    fn getmask(
        &mut self,
        line: &Self::Value,
        mode: &Self::Value,
        start: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Call the final `font.getmask(text)` fallback after a TypeError.
    fn getmask_text_only(&mut self, line: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Identify AttributeError from `getmask2`, the only error that selects getmask.
    fn is_attribute_error(&self, error: &Self::Error) -> bool;
    /// Identify TypeError from the full `getmask` call, which selects text-only fallback.
    fn is_type_error(&self, error: &Self::Error) -> bool;
    /// Add one returned mask offset to the matching destination coordinate.
    fn add_offset(
        &mut self,
        position: &mut DrawTextMaskPosition<Self::Value>,
        offset: &Self::Value,
        index: usize,
    ) -> Result<(), Self::Error>;
    /// Read `mask.size` with Pillow's default-None behavior.
    fn mask_size(&mut self, mask: &Self::Value) -> Result<Option<Self::Value>, Self::Error>;
    /// Compare one mask-size component with zero and convert the result to bool.
    fn mask_dimension_is_zero(
        &mut self,
        size: &Self::Value,
        index: usize,
    ) -> Result<bool, Self::Error>;
    /// Read `mask._image` with Pillow's default-None behavior.
    fn embedded_image(&mut self, mask: &Self::Value) -> Result<Option<Self::Value>, Self::Error>;
    /// Test whether the mask is already an Image instance.
    fn is_image(&mut self, mask: &Self::Value) -> Result<bool, Self::Error>;
    /// Read `mask.mode`, defaulting to the selected font-mask mode.
    fn mask_mode(
        &mut self,
        mask: &Self::Value,
        default: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Call `mask.tobytes()`.
    fn mask_bytes(&mut self, mask: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Resolve the host's `Image.frombytes` method before reading mask bytes.
    fn image_from_bytes_method(&mut self) -> Result<Self::Value, Self::Error>;
    /// Construct an Image through the host's `Image.frombytes` API.
    fn image_from_bytes(
        &mut self,
        method: &Self::Value,
        mode: &Self::Value,
        size: &Self::Value,
        bytes: &Self::Value,
    ) -> Result<Self::Value, Self::Error>;
    /// Call `mask_image.getchannel("A")`.
    fn alpha_channel(&mut self, mask_image: &Self::Value) -> Result<Self::Value, Self::Error>;
    /// Call `mask_image.putalpha(alpha)`.
    fn set_alpha(&mut self, mask_image: &Self::Value, alpha: u8) -> Result<(), Self::Error>;
    /// Draw an RGBA mask through the destination's text-color-mask path.
    fn draw_rgba_mask(
        &mut self,
        position: &DrawTextMaskPosition<Self::Value>,
        mask_image: &Self::Value,
        coverage: &Self::Value,
    ) -> Result<(), Self::Error>;
    /// Draw a coverage mask through ImageDraw.bitmap.
    fn draw_bitmap(
        &mut self,
        position: &DrawTextMaskPosition<Self::Value>,
        mask_image: &Self::Value,
        fill: &Self::Value,
    ) -> Result<(), Self::Error>;
}

/// Retrieve, adapt, and composite a custom font mask using ordered callbacks.
#[doc(hidden)]
pub fn draw_custom_font_mask_with_callbacks<C: DrawTextMaskCallbacks>(
    callbacks: &mut C,
    line: &C::Value,
    fill: &C::Value,
    ink: i64,
    mode: Mode,
) -> Result<(), C::Error> {
    let mode = callbacks.font_mask_mode(mode)?;
    let mut position = callbacks.position(line)?;
    let mask = retrieve_custom_mask(callbacks, line, &mode, &mut position)?;
    if mask_is_empty(callbacks, &mask)? {
        return Ok(());
    }
    let mask_image = custom_mask_image(callbacks, &mask, &mode)?;
    if callbacks.mode_is_rgba(&mode)? {
        let coverage = callbacks.alpha_channel(&mask_image)?;
        callbacks.set_alpha(&mask_image, ((ink >> 24) & 0xff) as u8)?;
        callbacks.draw_rgba_mask(&position, &mask_image, &coverage)
    } else {
        callbacks.draw_bitmap(&position, &mask_image, fill)
    }
}

fn retrieve_custom_mask<C: DrawTextMaskCallbacks>(
    callbacks: &mut C,
    line: &C::Value,
    mode: &C::Value,
    position: &mut DrawTextMaskPosition<C::Value>,
) -> Result<C::Value, C::Error> {
    let mask_result: Result<C::Value, C::Error> = (|| {
        let (mask, offset) = callbacks.getmask2(line, mode, &position.start)?;
        callbacks.add_offset(position, &offset, 0)?;
        callbacks.add_offset(position, &offset, 1)?;
        Ok(mask)
    })();
    match mask_result {
        Ok(mask) => Ok(mask),
        Err(error) if callbacks.is_attribute_error(&error) => {
            match callbacks.getmask(line, mode, &position.start) {
                Ok(mask) => Ok(mask),
                Err(error) if callbacks.is_type_error(&error) => callbacks.getmask_text_only(line),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

fn mask_is_empty<C: DrawTextMaskCallbacks>(
    callbacks: &mut C,
    mask: &C::Value,
) -> Result<bool, C::Error> {
    let Some(size) = callbacks.mask_size(mask)? else {
        return Ok(false);
    };
    if callbacks.mask_dimension_is_zero(&size, 0)? {
        return Ok(true);
    }
    callbacks.mask_dimension_is_zero(&size, 1)
}

fn custom_mask_image<C: DrawTextMaskCallbacks>(
    callbacks: &mut C,
    mask: &C::Value,
    default_mode: &C::Value,
) -> Result<C::Value, C::Error> {
    if let Some(image) = callbacks.embedded_image(mask)? {
        return Ok(image);
    }
    if callbacks.is_image(mask)? {
        return Ok(mask.clone());
    }
    let Some(size) = callbacks.mask_size(mask)? else {
        return Err(PilError::TypeError("font mask has no image size".into()).into());
    };
    let mode = callbacks.mask_mode(mask, default_mode)?;
    let from_bytes = callbacks.image_from_bytes_method()?;
    let bytes = callbacks.mask_bytes(mask)?;
    callbacks.image_from_bytes(&from_bytes, &mode, &size, &bytes)
}

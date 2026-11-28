use crate::compute::registry::support::OperationId;
use crate::raster::Mode;

use super::point::ImageOpsLutCallbacks;

/// Host operations used by Pillow's palette-aware `_lut(..., palette=True)` path.
#[doc(hidden)]
pub trait ImageOpsPaletteLutCallbacks: ImageOpsLutCallbacks {
    /// Public palette mode object used by Pillow's channel expression.
    type PaletteMode;
    /// Value returned by the public `getpalette(None)` method.
    type PaletteValues;
    /// Iterator over the palette values.
    type PaletteIterator;
    /// One palette value supplied to the LUT.
    type PaletteEntry;
    /// Mutable sequence filled with the transformed palette values.
    type PaletteBuilder;

    /// Call the public image's `copy()` method.
    fn copy_palette_image(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error>;
    /// Evaluate `image.palette is None` on the copied image.
    fn palette_is_none(&mut self, image: &Self::Image) -> Result<bool, Self::Error>;
    /// Return Pillow's bare assertion error for a missing palette.
    fn palette_assertion_error(&mut self) -> Self::Error;
    /// Read `image.palette.mode` from the copied image.
    fn palette_mode(&mut self, image: &Self::Image) -> Result<Self::PaletteMode, Self::Error>;
    /// Call `image.getpalette(None)` on the copied image.
    fn palette_values(&mut self, image: &Self::Image) -> Result<Self::PaletteValues, Self::Error>;
    /// Test whether `getpalette(None)` returned `None`.
    fn palette_values_are_none(
        &mut self,
        values: &Self::PaletteValues,
    ) -> Result<bool, Self::Error>;
    /// Acquire the iterator used by Pillow's palette list comprehension.
    fn palette_values_iter(
        &mut self,
        values: Self::PaletteValues,
    ) -> Result<Self::PaletteIterator, Self::Error>;
    /// Fetch the next palette value.
    fn next_palette_entry(
        &mut self,
        values: &mut Self::PaletteIterator,
    ) -> Result<Option<Self::PaletteEntry>, Self::Error>;
    /// Evaluate `mode[index % len(mode)] == "A"` with host protocols.
    fn palette_entry_is_alpha(
        &mut self,
        mode: &Self::PaletteMode,
        index: usize,
    ) -> Result<bool, Self::Error>;
    /// Evaluate `lut[entry]` with the host's sequence behavior.
    fn palette_lut_entry(
        &mut self,
        lut: &Self::Lut,
        entry: &Self::PaletteEntry,
    ) -> Result<Self::PaletteEntry, Self::Error>;
    /// Allocate the result list after the palette iterator is acquired.
    fn new_palette_builder(&mut self) -> Result<Self::PaletteBuilder, Self::Error>;
    /// Append one copied or transformed channel to the result list.
    fn append_palette_entry(
        &mut self,
        builder: &mut Self::PaletteBuilder,
        entry: Self::PaletteEntry,
    ) -> Result<(), Self::Error>;
    /// Call `image.putpalette(values, mode)` and return the copied image.
    fn put_palette(
        &mut self,
        image: Self::Image,
        values: Self::PaletteBuilder,
        mode: Self::PaletteMode,
    ) -> Result<Self::Output, Self::Error>;
}

/// Applies Pillow's `_lut(..., palette=True)` route and its public palette APIs.
#[doc(hidden)]
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub fn imageops_lut_with_palette_callbacks<C: ImageOpsPaletteLutCallbacks>(
    callbacks: &mut C,
    image: &C::Image,
    lut: C::Lut,
    operation: OperationId,
) -> Result<C::Output, C::Error> {
    if callbacks.image_mode_is_palette(image)? {
        operation
            .support()
            .for_mode(Mode::ImagePalette8)
            .validate_mode()?;
        return imageops_palette_lut(callbacks, image, &lut);
    }
    super::point::imageops_lut_non_palette(callbacks, image, lut, operation)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn imageops_palette_lut<C: ImageOpsPaletteLutCallbacks>(
    callbacks: &mut C,
    image: &C::Image,
    lut: &C::Lut,
) -> Result<C::Output, C::Error> {
    let copied = callbacks.copy_palette_image(image)?;
    // Pillow's `_lut` reads `im.palette` once for this assertion.
    if callbacks.palette_is_none(&copied)? {
        return Err(callbacks.palette_assertion_error());
    }
    let mode = callbacks.palette_mode(&copied)?;
    let values = callbacks.palette_values(&copied)?;
    if callbacks.palette_values_are_none(&values)? {
        return Err(callbacks.palette_assertion_error());
    }
    let mut entries = callbacks.palette_values_iter(values)?;
    let mut output = callbacks.new_palette_builder()?;
    let mut index = 0;
    while let Some(entry) = callbacks.next_palette_entry(&mut entries)? {
        let output_entry = if callbacks.palette_entry_is_alpha(&mode, index)? {
            entry
        } else {
            callbacks.palette_lut_entry(lut, &entry)?
        };
        callbacks.append_palette_entry(&mut output, output_entry)?;
        index += 1;
    }
    callbacks.put_palette(copied, output, mode)
}

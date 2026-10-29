//! Palette colors are injected before the archived expansion and luma stages.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::compute::registry::support::PaletteInput;
use crate::raster::{DynamicImage, GrayImage, GrayAlphaImage, RgbaImage};
use super::super::super::helpers::luma::{finish};

pub(crate) type Input = GrayAlphaImage;
pub(crate) struct Kernel<'a> { source: RgbaImage, dimensions: CheckedDims, input: &'a Input }
pub(crate) fn prepare<'a>(input: &'a Input, palette: PaletteInput<'_>) -> Result<Kernel<'a>, PilError> {
    let source = match crate::image::expand_palette_alpha(input, palette.rgb.unwrap_or_default()) {
        DynamicImage::ImageRgba8(rgba) => rgba,
        _ => unreachable!("PA expansion constructs RGBA"),
    };
    let dimensions = CheckedDims::new_allow_empty(input.width(), input.height(), 1)?;
    Ok(Kernel { source, dimensions, input })
}
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let source = input.source;
    let output = crate::color::grayscale_rgb_bytes::<4>(source.as_raw(), input.dimensions, 32768);
    finish(input.input.width(), input.input.height(), output)
}

//! Palette colors are injected before the archived expansion and luma stages.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::compute::registry::support::PaletteInput;
use crate::raster::{DynamicImage, GrayImage, RgbImage, RgbaImage};
use super::super::super::helpers::luma::{finish, rgb};

pub(crate) type Input = GrayImage;
pub(crate) enum Pixels<'a> { Indices(&'a GrayImage), Rgb(RgbImage), Rgba(RgbaImage) }
pub(crate) struct Kernel<'a> { source: Pixels<'a>, dimensions: CheckedDims }
pub(crate) fn prepare<'a>(input: &'a Input, palette: PaletteInput<'_>) -> Result<Kernel<'a>, PilError> {
    let source = match palette.rgb {
        Some(rgb) => match crate::image::expand_palette(input, rgb, palette.alpha) {
            DynamicImage::ImageRgb8(rgb) => Pixels::Rgb(rgb),
            DynamicImage::ImageRgba8(rgba) => Pixels::Rgba(rgba),
            _ => unreachable!("palette expansion constructs RGB or RGBA"),
        },
        None => Pixels::Indices(input),
    };
    let dimensions = CheckedDims::new_allow_empty(input.width(), input.height(), 1)?;
    Ok(Kernel { source, dimensions })
}
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    match input.source {
        Pixels::Indices(indices) => finish(indices.width(), indices.height(), indices.as_raw().to_vec()),
        Pixels::Rgb(source) => rgb(&source, input.dimensions),
        Pixels::Rgba(source) => {
            let output = crate::color::grayscale_rgb_bytes::<4>(source.as_raw(), input.dimensions, 32768);
            finish(source.width(), source.height(), output)
        }
    }
}

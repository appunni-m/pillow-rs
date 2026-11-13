//! Convert native palette-alpha indices to luma without materializing RGBA.

use super::super::super::helpers::luma::finish;
use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::palette as palette_lookup;
use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::{GrayAlphaImage, GrayImage};

pub(crate) type Input = GrayAlphaImage;
pub(crate) struct Kernel<'a> {
    input: &'a Input,
    palette: PaletteInput<'a>,
    dimensions: CheckedDims,
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: &'a Input,
    palette: PaletteInput<'a>,
) -> Result<Kernel<'a>, PilError> {
    let dimensions = CheckedDims::new_allow_empty(input.width(), input.height(), 1)?;
    Ok(Kernel {
        input,
        palette,
        dimensions,
    })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let mut output: Vec<u8> = input
        .input
        .as_raw()
        .chunks_exact(2)
        .take(input.dimensions.total_pixels())
        .map(|sample| {
            let rgb = palette_lookup::color(sample[0], input.palette);
            crate::color::grayscale_rgb_pixel(&rgb, 32768)
        })
        .collect();
    output.resize(input.dimensions.total_pixels(), 0);
    finish(input.input.width(), input.input.height(), output)
}

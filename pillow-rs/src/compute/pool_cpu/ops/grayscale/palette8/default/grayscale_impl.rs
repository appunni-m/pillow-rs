//! Convert native palette indices to luma without materializing RGB pixels.

use super::super::super::helpers::luma::finish;
use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::palette as palette_lookup;
use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) type Input = GrayImage;
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
    let source = input.input;
    let output = match input.palette.rgb {
        None => source.as_raw().to_vec(),
        Some(_) => source
            .as_raw()
            .iter()
            .take(input.dimensions.total_pixels())
            .map(|index| {
                let rgb = palette_lookup::color(*index, input.palette);
                crate::color::grayscale_rgb_pixel(&rgb, 32768)
            })
            .collect(),
    };
    finish(source.width(), source.height(), output)
}

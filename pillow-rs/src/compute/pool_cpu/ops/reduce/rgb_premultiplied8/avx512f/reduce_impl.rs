//! AVX-512F 2x2 reduction strategy for premultiplied RGBa8.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    context: &ExecutionContext,
    selection: &StrategySelection,
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    let output = super::super::super::helpers::avx512f_2x2::execute::<false>(
        context,
        selection,
        Mode::ImageRgbPremultiplied8,
        input,
        x_factor,
        y_factor,
    )?;
    Ok(DynamicImage::ImageRgbPremultiplied8(output))
}

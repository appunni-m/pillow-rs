//! Native RGBA8 quantization strategy for the CPU Default architecture.

use crate::compute::pool_cpu::helpers::quantization::quantize_rgba_samples;
use crate::error::PilError;
use crate::raster::RgbaImage;

/// Quantizes the native RGBA buffer while retaining alpha in palette entries.
pub(crate) fn execute(
    image: &RgbaImage,
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let (indices, palette, alpha) = quantize_rgba_samples(image.as_raw(), width, height, colors)?;
    Ok((indices, palette, Some(alpha)))
}

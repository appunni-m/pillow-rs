//! Native L quantization strategy for the CPU Default architecture.

use crate::compute::pool_cpu::helpers::quantization::RgbPixelSource;
use crate::compute::pool_cpu::helpers::quantization::map_luma_to_fixed_palette;
use crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source;
use crate::error::PilError;
use crate::raster::GrayImage;

/// Quantizes L samples using Pillow's grayscale-as-RGB quantizer semantics.
pub(crate) fn execute(
    image: &GrayImage,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    quantize_rgb_source::execute(
        &LumaSamples(image.as_raw()),
        width,
        height,
        colors,
        kmeans,
        method,
    )
}

/// Maps native L samples to a caller-supplied RGB palette.
pub(crate) fn execute_with_palette(
    image: &GrayImage,
    palette: &[[u8; 3]],
    dither: bool,
) -> Result<Vec<u8>, PilError> {
    map_luma_to_fixed_palette(
        image.as_raw(),
        image.width(),
        image.height(),
        palette,
        dither,
    )
}

struct LumaSamples<'a>(&'a [u8]);

impl RgbPixelSource for LumaSamples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        let sample = self.0[index];
        [sample, sample, sample]
    }
}

//! AVX2 RGB8-to-Luma8 Convert route.

use crate::compute::pool_cpu::ops::grayscale::rgb8 as grayscale_rgb8;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

/// Executes a prepared AVX2 RGB8-to-Luma8 strategy.
///
/// # Safety
///
/// The caller must use an AVX2 architecture selected from this mode's
/// support declaration for the same source and target.
#[allow(unsafe_code)]
pub(crate) unsafe fn execute(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let kernel = grayscale_rgb8::default::grayscale_impl::prepare(source)?;
    let pixels = kernel.dimensions.total_pixels();

    #[cfg(target_arch = "x86_64")]
    let output = {
        // SAFETY: support selection establishes AVX2, source storage was
        // checked by prepare, and the grayscale leaf bounds every vector block.
        let (bytes, blocks) = unsafe {
            grayscale_rgb8::avx2::grayscale_impl::execute(kernel.input.as_raw(), pixels)
        };
        (bytes, blocks, 0)
    };

    #[cfg(target_arch = "x86")]
    let output = grayscale_rgb8::avx2::grayscale_wide_impl::execute(kernel);

    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        output,
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}

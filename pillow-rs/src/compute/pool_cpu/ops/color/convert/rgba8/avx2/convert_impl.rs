//! AVX2 RGBA8-to-Luma8 Convert route.

use crate::compute::pool_cpu::ops::grayscale::rgba8::default::grayscale_impl;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let kernel = grayscale_impl::prepare(source)?;
    let output = crate::compute::pool_cpu::ops::grayscale::rgba8::avx2::grayscale_impl::execute(
        kernel,
    );
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        output,
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}

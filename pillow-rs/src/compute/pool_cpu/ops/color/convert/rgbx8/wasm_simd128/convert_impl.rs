//! WebAssembly SIMD128 RGBX-to-Luma8 Convert route.

use crate::compute::pool_cpu::ops::grayscale::rgbx8::default::grayscale_impl;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target != Mode::ImageLuma8 {
        return super::super::helpers::vector::execute(source, target);
    }
    luma8(source)
}

fn luma8(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let kernel = grayscale_impl::prepare(source)?;
    let output = crate::compute::pool_cpu::ops::grayscale::rgbx8::wasm_simd128::grayscale_impl::execute(
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

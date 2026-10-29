//! WASM_SIMD128 implementation for exact ImageRgba8 Reduce.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

fn make_output(
    width: u32,
    height: u32,
    bytes: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    let image = crate::raster::RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError(
            "failed to construct ImageRgba8 Reduce output".into(),
        ))?;
    Ok(DynamicImage::ImageRgba8(image))
}

pub(crate) fn execute(
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let output_width = input.width().div_ceil(x_factor);
    let output_height = input.height().div_ceil(y_factor);
    let bytes = crate::compute::pool_cpu::ops::reduce::helpers::native_bytes::kernels::wide::reduce::<
        4, true, false,
    >(input.as_raw(), input.width(), input.height(), x_factor, y_factor)?;
    make_output(output_width, output_height, bytes)
}

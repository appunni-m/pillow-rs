//! AVX2 implementation for exact ImageYCbCr8 Reduce.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

fn make_output(width: u32, height: u32, bytes: Vec<u8>) -> Result<DynamicImage, PilError> {
    let image = RgbImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("failed to construct ImageYCbCr8 Reduce output".into()))?;
    Ok(DynamicImage::ImageYCbCr8(image))
}

pub(crate) fn execute(
    input: &RgbImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let output_width = input.width().div_ceil(x_factor);
    let output_height = input.height().div_ceil(y_factor);
    let bytes = crate::compute::pool_cpu::ops::reduce::helpers::native_bytes::kernels::avx2::reduce::<
        3, false, false,
    >(input.as_raw(), input.width(), input.height(), x_factor, y_factor)?;
    make_output(output_width, output_height, bytes)
}

//! AVX2/FMA I-mode strategy for 5x5 convolution.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute(
    image: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI32(source) = image else {
        return Err(PilError::InternalError(
            "AVX2/FMA Filter5x5 strategy received a non-I raster".into(),
        ));
    };
    let normalized = std::array::from_fn(|index| kernel[index] / scale);
    let mut output = source.as_raw().to_vec();
    super::filter5x5_kernel::native_filter_5x5_i32_rows(
        source.as_raw(),
        &mut output,
        source.width() as usize,
        source.height() as usize,
        &normalized,
        offset + 0.5,
    )
    .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?;
    let output = RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?;
    Ok(DynamicImage::ImageI32(output))
}

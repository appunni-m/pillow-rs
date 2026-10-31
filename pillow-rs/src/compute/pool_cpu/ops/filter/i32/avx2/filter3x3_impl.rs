//! AVX2/FMA I-mode strategy for 3x3 convolution.

use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute(
    image: &DynamicImage,
    request: FilterRequest,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI32(source) = image else {
        return Err(PilError::InternalError(
            "AVX2/FMA Filter3x3 strategy received a non-I raster".into(),
        ));
    };
    let FilterRequest::Filter3x3 { kernel, scale, offset } = request else {
        return Err(PilError::InternalError(
            "AVX2/FMA Filter3x3 strategy received a non-3x3 request".into(),
        ));
    };
    let normalized = std::array::from_fn(|index| kernel[index] / scale);
    let mut output = source.as_raw().to_vec();
    let (vector_blocks, scalar_tail) = super::filter3x3_kernel::filter_3x3_i32_rows(
        source.as_raw(),
        &mut output,
        source.width() as usize,
        source.height() as usize,
        &normalized,
        offset + 0.5,
    )
    .ok_or_else(|| PilError::ValueError("filter_3x3_i32: buffer error".into()))?;
    crate::compute::record_pipeline_operation_path("vector");
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    let output = RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::ValueError("filter_3x3_i32: buffer error".into()))?;
    Ok(DynamicImage::ImageI32(output))
}

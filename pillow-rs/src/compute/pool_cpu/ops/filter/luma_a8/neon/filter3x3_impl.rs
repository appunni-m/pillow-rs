//! NEON LA-mode implementation and retained binomial fast path for Filter3x3.

use crate::compute::helpers::filters::binomial3x3_parameters_match;
use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage};

pub(crate) fn execute(
    image: &DynamicImage,
    request: FilterRequest,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLumaA8(source) = image else {
        return Err(PilError::InternalError(
            "NEON Filter3x3 strategy received a non-LA raster".into(),
        ));
    };
    let FilterRequest::Filter3x3 { kernel, scale, offset } = request else {
        return Err(PilError::InternalError(
            "NEON Filter3x3 strategy received a non-3x3 request".into(),
        ));
    };
    if !binomial3x3_parameters_match(&kernel, scale, offset) || source.width() < 16 {
        return super::super::ops::execute_filter3x3_vectorized(
            image,
            FilterRequest::Filter3x3 {
                kernel,
                scale,
                offset,
            },
        );
    }

    let mut output = source.as_raw().to_vec();
    let (vector_blocks, scalar_tail) = super::binomial3x3_kernel::filter_3x3_binomial_la_rows(
        source.as_raw(),
        &mut output,
        source.width() as usize,
        source.height() as usize,
    )
    .ok_or_else(|| PilError::ValueError("filter output buffer does not match LA mode".into()))?;
    crate::compute::record_pipeline_operation_path("vector");
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    GrayAlphaImage::from_raw(source.width(), source.height(), output)
        .map(DynamicImage::ImageLumaA8)
        .ok_or_else(|| PilError::ValueError("filter output buffer does not match LA mode".into()))
}

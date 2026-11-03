//! NEON LA-mode implementations of selected exact 5x5 filters.

use crate::compute::helpers::filters::{
    binomial5x5_parameters_match, la_single_outlier5x5_parameters_match,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage};

pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::super::ops::FilterRequest,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLumaA8(source) = image else {
        return Err(PilError::InternalError(
            "NEON Filter5x5 strategy received a non-LA raster".into(),
        ));
    };
    let super::super::super::ops::FilterRequest::Filter5x5 { kernel, scale, offset } = request
    else {
        return Err(PilError::InternalError(
            "NEON Filter5x5 strategy received a non-5x5 request".into(),
        ));
    };
    if source.width() < 5 || source.height() < 5 {
        return super::super::ops::execute_filter5x5_vectorized(
            image,
            super::super::ops::FilterRequest::Filter5x5 {
                kernel,
                scale,
                offset,
            },
        );
    }
    let mut output = source.as_raw().to_vec();
    let dimensions = (source.width() as usize, source.height() as usize);
    let result = if la_single_outlier5x5_parameters_match(&kernel, scale, offset) {
        super::super::helpers::single_outlier5x5_kernel::filter_5x5_single_outlier_la_rows(
            source.as_raw(),
            &mut output,
            dimensions.0,
            dimensions.1,
        )
    } else if binomial5x5_parameters_match(&kernel, scale, offset) {
        super::super::helpers::binomial5x5_kernel::filter_5x5_binomial_la_rows(
            source.as_raw(),
            &mut output,
            dimensions.0,
            dimensions.1,
        )
    } else {
        return super::super::ops::execute_filter5x5_vectorized(
            image,
            super::super::ops::FilterRequest::Filter5x5 {
                kernel,
                scale,
                offset,
            },
        );
    };
    let Some((vector_blocks, scalar_tail)) = result else {
        return super::super::ops::execute_filter5x5_vectorized(
            image,
            super::super::ops::FilterRequest::Filter5x5 {
                kernel,
                scale,
                offset,
            },
        );
    };
    crate::compute::record_pipeline_operation_path("vector");
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    let output = GrayAlphaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::ValueError("filter output buffer does not match its mode".into()))?;
    Ok(DynamicImage::ImageLumaA8(output))
}

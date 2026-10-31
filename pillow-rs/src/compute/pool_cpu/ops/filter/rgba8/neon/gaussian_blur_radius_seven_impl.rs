//! RGBA radius-seven GaussianBlur orchestration for NEON.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

use super::gaussian_blur_radius_seven::{NeonKernel, blur_vertical_rows};

pub(super) fn execute_gaussian_blur_radius_seven(
    image: &DynamicImage,
    sigma: f32,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(input) = image else {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur NEON strategy received a mismatched raster".into(),
        ));
    };
    if !sigma.is_finite() || sigma == 0.0 {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur NEON strategy received invalid sigma".into(),
        ));
    }
    let (radius, whole_weight, fractional_weight) =
        crate::compute::pool_cpu::ops::filter::helpers::gaussian_blur_box_parameters(sigma);
    if radius != 7 || fractional_weight == 0 {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur NEON strategy received an unsupported radius".into(),
        ));
    }
    let dimensions = CheckedDims::new(input.width(), input.height(), 4)?;
    if dimensions.width < 64
        || dimensions.height < 64
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur NEON strategy received invalid native dimensions".into(),
        ));
    }

    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    let mut work = input.as_raw().to_vec();
    let mut scratch = dimensions.alloc_buffer();
    crate::compute::record_pipeline_operation_path("vector");
    for _ in 0..3 {
        let blocks = super::super::helpers::blur_horizontal_rows::<NeonKernel>(
            &work,
            &mut scratch,
            width,
            height,
            whole_weight,
            fractional_weight,
        );
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
        std::mem::swap(&mut work, &mut scratch);
    }
    for _ in 0..3 {
        let (blocks, scalar_tail) = blur_vertical_rows(
            &work,
            &mut scratch,
            width,
            height,
            whole_weight,
            fractional_weight,
        );
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("neon-rgba-row-major-vertical");
        std::mem::swap(&mut work, &mut scratch);
    }
    RgbaImage::from_raw(input.width(), input.height(), work)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| PilError::InternalError("RGBA GaussianBlur output shape mismatch".into()))
}

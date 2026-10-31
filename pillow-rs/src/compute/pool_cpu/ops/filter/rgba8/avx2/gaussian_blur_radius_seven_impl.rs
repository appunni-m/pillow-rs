//! RGBA radius-seven GaussianBlur orchestration for AVX2.

use crate::checked_dims::CheckedDims;
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

use super::gaussian_blur_radius_seven::Avx2Kernel;

pub(super) fn execute_gaussian_blur_radius_seven(
    image: &DynamicImage,
    sigma: f32,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(input) = image else {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur AVX2 strategy received a mismatched raster".into(),
        ));
    };
    if !sigma.is_finite() || sigma == 0.0 {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur AVX2 strategy received invalid sigma".into(),
        ));
    }
    let (radius, whole_weight, fractional_weight) =
        crate::compute::pool_cpu::ops::filter::helpers::gaussian_blur_box_parameters(
            sigma,
        );
    if radius != 7 || fractional_weight == 0 {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur AVX2 strategy received an unsupported radius".into(),
        ));
    }

    let dimensions = CheckedDims::new(input.width(), input.height(), 4)?;
    if dimensions.width == 0
        || dimensions.height == 0
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur AVX2 strategy received invalid native dimensions".into(),
        ));
    }

    let (mut work, mut scratch) = blur_passes::<Avx2Kernel>(
        input.as_raw().to_vec(),
        dimensions.alloc_buffer(),
        dimensions.width as usize,
        dimensions.height as usize,
        whole_weight,
        fractional_weight,
    );
    super::super::helpers::transpose_rgba_in_place(
        &mut work,
        &mut scratch,
        input.width(),
        input.height(),
        context,
    )?;
    let transposed_dims = CheckedDims::new(input.height(), input.width(), 4)?;
    debug_assert_eq!(scratch.len(), transposed_dims.total_bytes());
    (work, scratch) = blur_passes::<Avx2Kernel>(
        work,
        scratch,
        transposed_dims.width as usize,
        transposed_dims.height as usize,
        whole_weight,
        fractional_weight,
    );
    super::super::helpers::transpose_rgba_in_place(
        &mut work,
        &mut scratch,
        transposed_dims.width,
        transposed_dims.height,
        context,
    )?;
    let output = RgbaImage::from_raw(input.width(), input.height(), work)
        .ok_or_else(|| PilError::InternalError("RGBA GaussianBlur output shape mismatch".into()))?;
    crate::compute::record_pipeline_operation_path("avx2-transpose");
    Ok(DynamicImage::ImageRgba8(output))
}
fn blur_passes<K: super::super::helpers::RadiusSevenHorizontalKernel>(
    mut work: Vec<u8>,
    mut scratch: Vec<u8>,
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> (Vec<u8>, Vec<u8>) {
    crate::compute::record_pipeline_operation_path("vector");
    for _ in 0..3 {
        let blocks = super::super::helpers::blur_horizontal_rows::<K>(
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
    (work, scratch)
}

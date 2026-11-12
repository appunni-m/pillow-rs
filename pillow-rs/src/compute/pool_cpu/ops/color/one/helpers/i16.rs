//! Native bilevel expansion into exact unsigned 16-bit mode carriers.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, ImageBuffer, Luma, Mode};
use wide::{u8x16, u16x16};

pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    let dims = CheckedDims::new_allow_empty(source.width(), source.height(), 1)?;
    if source.as_raw().len() != dims.total_pixels() {
        return Err(PilError::InternalError(
            "One Convert source storage did not match its dimensions".into(),
        ));
    }
    if !matches!(
        target,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    ) {
        return Err(PilError::InternalError(
            "One Convert selected an undeclared I;16 target".into(),
        ));
    }

    let pixels = dims.total_pixels();
    let mut samples = Vec::with_capacity(pixels);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(16) {
        let active = (pixels - start).min(16);
        let lanes =
            std::array::from_fn(|lane| source.as_raw().get(start + lane).copied().unwrap_or(0));
        let bytes = u8x16::new(lanes);
        let expanded = bytes
            .simd_ne(u8x16::ZERO)
            .select(u8x16::splat(u8::MAX), u8x16::ZERO);
        samples.extend(u16x16::from(expanded).to_array().into_iter().take(active));
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if samples.len() != pixels {
        return Err(PilError::InternalError(
            "One Convert I;16 output shape mismatch".into(),
        ));
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    if samples.capacity() != 0 {
        crate::compute::record_pipeline_allocation(
            samples
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        );
    }
    let output =
        ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(source.width(), source.height(), samples)
            .ok_or_else(|| {
                PilError::InternalError("One Convert I;16 output shape mismatch".into())
            })?;
    match target {
        Mode::ImageI16 => Ok(DynamicImage::ImageI16(output)),
        Mode::ImageI16L => Ok(DynamicImage::ImageI16L(output)),
        Mode::ImageI16B => Ok(DynamicImage::ImageI16B(output)),
        Mode::ImageI16N => Ok(DynamicImage::ImageI16N(output)),
        _ => Err(PilError::InternalError(
            "One Convert selected an undeclared I;16 target".into(),
        )),
    }
}

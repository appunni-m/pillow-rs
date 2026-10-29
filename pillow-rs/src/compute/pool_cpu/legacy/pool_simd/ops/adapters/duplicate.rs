//! Native-layout SIMD duplication and its admission contract.

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use super::copy_native_bytes;
use super::{
    DynamicImage, PilError, PipelineOp, SimdImageShape, has_nonempty_byte_data, native_copy_layout,
    preserve_mode, shape_native_copy_channels, simd_unsupported,
};
use crate::raster::GenericImageView;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use fearless_simd::prelude::SimdBase;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use fearless_simd::{Level, Neon, u8x16 as SafeU8x16};

pub(super) fn supports_for_image(img: &DynamicImage, mode: Option<&str>) -> bool {
    native_copy_layout(img, mode).is_some_and(|channels| has_nonempty_byte_data(img, channels))
}

pub(super) fn supports_for_shape(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape_native_copy_channels(shape, mode).is_some_and(|channels| {
        shape.width != 0
            && shape.height != 0
            && shape
                .width
                .checked_mul(shape.height)
                .and_then(|pixels| pixels.checked_mul(channels as u32))
                .is_some_and(|bytes| bytes != 0)
    })
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fearless_simd::kernel!(
    #[inline(always)]
    fn copy_native_lanes_neon(neon: Neon, source: &[u8; 16], output: &mut [u8; 16]) {
        SafeU8x16::<Neon>::from_slice(neon, source).store_array(output);
    }
);

/// Duplicate an image through its native byte layout.
///
/// Unlike the packed scalar adapter, this preserves raw P/PA samples and the
/// fourth CMYK byte. Safe vector loads append fully initialized lane arrays to
/// the final allocation without a separate zero-fill pass.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(super) fn simd_copy_native_bytes(source: &[u8]) -> (Vec<u8>, u64, u64) {
    let vector_len = source.len() / 64 * 64;
    let scalar_tail = source.len() - vector_len;
    let neon = Level::new()
        .as_neon()
        .expect("AArch64 NEON target configuration has a NEON execution token");
    let mut output = Vec::<u8>::with_capacity(source.len());

    for block in source[..vector_len].chunks_exact(64) {
        let mut copied = [0u8; 64];
        let source_lanes = block.as_chunks::<16>().0;
        let output_lanes = copied.as_chunks_mut::<16>().0;
        for (source, destination) in source_lanes.iter().zip(output_lanes) {
            copy_native_lanes_neon(neon, source, destination);
        }
        output.extend_from_slice(&copied);
    }
    output.extend_from_slice(&source[vector_len..]);

    (output, (vector_len / 16) as u64, scalar_tail as u64)
}

/// Duplicate an image through its native byte layout.
///
/// The NEON copy handles full blocks without a zero-fill pass; other targets
/// retain the portable vector implementation and its bounded final block.
pub fn simd_duplicate(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !matches!(op, PipelineOp::Duplicate) {
        return Err(PilError::ValueError("expected Duplicate op".into()));
    }
    let Some(channels) = native_copy_layout(img, mode) else {
        return Err(simd_unsupported("Duplicate"));
    };
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Err(simd_unsupported("Duplicate"));
    }
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD duplicate image length overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != expected_len {
        return Err(simd_unsupported("Duplicate"));
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let (output, vector_blocks, scalar_tail) = simd_copy_native_bytes(source);
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let (output, vector_blocks, scalar_tail) = {
        let mut output = vec![0u8; expected_len];
        let (vector_blocks, scalar_tail) =
            copy_native_bytes(source, &mut output).ok_or_else(|| {
                PilError::InternalError("SIMD duplicate buffer shape mismatch".into())
            })?;
        (output, vector_blocks, scalar_tail)
    };
    let result = crate::image_utils::raw_bytes_to_image(width, height, output, channels)?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    crate::compute::record_pipeline_operation_path("neon-copy");
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    crate::compute::record_pipeline_operation_path("native-copy");
    Ok(preserve_mode(img, result))
}

#[cfg(test)]
mod tests;

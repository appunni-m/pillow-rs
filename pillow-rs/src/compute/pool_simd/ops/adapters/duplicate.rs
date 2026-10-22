//! Native-layout SIMD duplication and its admission contract.

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use super::copy_native_bytes;
use super::{
    DynamicImage, PilError, PipelineOp, SimdImageShape, has_nonempty_byte_data, native_copy_layout,
    preserve_mode, shape_native_copy_channels, simd_unsupported,
};
use crate::raster::GenericImageView;

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

/// Duplicate an image through its native byte layout.
///
/// Unlike the packed scalar adapter, this preserves raw P/PA samples and the
/// fourth CMYK byte. The NEON path copies directly into uninitialized output
/// storage, so it does not first zero every byte before overwriting it.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
pub(super) fn simd_copy_native_bytes(source: &[u8]) -> (Vec<u8>, u64, u64) {
    use core::arch::aarch64 as neon;

    let vector_len = source.len() / 64 * 64;
    let scalar_tail = source.len() - vector_len;
    let mut output = Vec::<u8>::with_capacity(source.len());

    // SAFETY: `output` has capacity for the full source but its length is
    // still zero. Each NEON load/store covers a complete 16-byte block within
    // `vector_len`; the remaining bytes are copied with the exact bounded
    // scalar-tail length. The source is immutable and cannot overlap the new
    // allocation. After these writes, every byte in `0..source.len()` is
    // initialized before the vector length is published.
    unsafe {
        let source_ptr = source.as_ptr();
        let output_ptr = output.as_mut_ptr();
        for offset in (0..vector_len).step_by(64) {
            let block = neon::vld1q_u8_x4(source_ptr.add(offset));
            neon::vst1q_u8_x4(output_ptr.add(offset), block);
        }
        std::ptr::copy_nonoverlapping(
            source_ptr.add(vector_len),
            output_ptr.add(vector_len),
            scalar_tail,
        );
        output.set_len(source.len());
    }

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

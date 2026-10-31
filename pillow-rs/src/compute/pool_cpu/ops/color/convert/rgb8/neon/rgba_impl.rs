//! NEON RGB-to-RGBA conversion for the exact RGB8 source mode.

use crate::error::PilError;
use crate::raster::{RgbImage, RgbaImage};

/// Expands RGB triplets to opaque RGBA pixels using sixteen-pixel blocks.
///
/// # Safety
///
/// The caller must ensure that the host supports NEON.
pub(crate) unsafe fn execute(source: &RgbImage) -> Result<RgbaImage, PilError> {
    let pixels = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("RGB Convert dimensions overflowed".into()))?;
    let source_bytes = pixels
        .checked_mul(3)
        .ok_or_else(|| PilError::DimensionError("RGB Convert source size overflowed".into()))?;
    let output_bytes = pixels
        .checked_mul(4)
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert output size overflowed".into()))?;
    if source.as_raw().len() != source_bytes {
        return Err(PilError::InternalError(
            "RGB Convert source storage did not match its dimensions".into(),
        ));
    }

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_bytes)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    // SAFETY: prepared selection establishes NEON; source length and output
    // capacity are checked, and the leaf handles complete sixteen-pixel blocks.
    unsafe { rgb_to_rgba_neon(source.as_raw(), vector_pixels, &mut output) };
    for pixel in vector_pixels..pixels {
        let offset = pixel * 3;
        output.extend_from_slice(&[
            source.as_raw()[offset],
            source.as_raw()[offset + 1],
            source.as_raw()[offset + 2],
            u8::MAX,
        ]);
    }

    crate::compute::record_pipeline_operation_vector_blocks((vector_pixels / 16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((pixels - vector_pixels) as u64);
    crate::compute::record_pipeline_operation_path("vector");
    RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBA Convert output shape mismatch".into()))
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[allow(unsafe_code)]
unsafe fn rgb_to_rgba_neon(source: &[u8], vector_pixels: usize, output: &mut Vec<u8>) {
    use core::arch::aarch64 as neon;

    // SAFETY: the caller checked the source extent and reserved the full
    // destination. Each structure load reads 48 bytes and store writes 64.
    unsafe {
        let opaque = neon::vdupq_n_u8(u8::MAX);
        let output_pointer = output.as_mut_ptr();
        for pixel in (0..vector_pixels).step_by(16) {
            let input = neon::vld3q_u8(source.as_ptr().add(pixel * 3));
            let rgba = neon::uint8x16x4_t(input.0, input.1, input.2, opaque);
            neon::vst4q_u8(output_pointer.add(pixel * 4), rgba);
        }
        output.set_len(vector_pixels * 4);
    }
}

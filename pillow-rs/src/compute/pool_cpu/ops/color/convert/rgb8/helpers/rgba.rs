//! Portable wide RGB-to-RGBA expansion shared by SSE2 and Wasm SIMD128.

use crate::error::PilError;
use crate::raster::{RgbImage, RgbaImage};
use wide::u8x16;

const RGB_TO_RGBA_BYTES: u8x16 =
    u8x16::new([0, 1, 2, 15, 3, 4, 5, 15, 6, 7, 8, 15, 9, 10, 11, 15]);

pub(crate) fn execute(source: &RgbImage) -> Result<RgbaImage, PilError> {
    let pixels = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(unsupported)?;
    let source_bytes = pixels
        .checked_mul(3)
        .ok_or_else(unsupported)?;
    let output_bytes = pixels
        .checked_mul(4)
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(unsupported)?;
    if source.as_raw().len() != source_bytes {
        return Err(unsupported());
    }

    let vector_pixels = pixels / 4 * 4;
    let mut output = Vec::new();
    output.try_reserve_exact(output_bytes).map_err(|_| unsupported())?;
    for pixel in (0..vector_pixels).step_by(4) {
        output.extend_from_slice(&rgb_to_rgba_block(source.as_raw(), pixel));
    }
    for pixel in vector_pixels..pixels {
        let offset = pixel * 3;
        output.extend_from_slice(&[
            source.as_raw()[offset],
            source.as_raw()[offset + 1],
            source.as_raw()[offset + 2],
            u8::MAX,
        ]);
    }

    if pixels == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks((vector_pixels / 4) as u64);
        crate::compute::record_pipeline_operation_scalar_tail((pixels - vector_pixels) as u64);
        crate::compute::record_pipeline_operation_path("vector");
    }
    RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBA Convert output shape mismatch".into()))
}

pub(crate) fn unsupported() -> PilError {
    PilError::NotImplementedError(
        "SIMD Convert is unsupported for the validated image layout/parameters".into(),
    )
}

#[inline]
fn rgb_to_rgba_block(source: &[u8], start_pixel: usize) -> [u8; 16] {
    let source_start = start_pixel * 3;
    let mut input = [0u8; 16];
    input[..12].copy_from_slice(&source[source_start..source_start + 12]);
    input[15] = u8::MAX;
    u8x16::new(input)
        .swizzle_relaxed(RGB_TO_RGBA_BYTES)
        .to_array()
}

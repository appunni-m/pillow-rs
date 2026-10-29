//! AVX2 tiles for transposing opaque four-byte pixels.

#![allow(unsafe_code)]

use core::arch::x86_64::{
    _mm256_loadu_si256, _mm256_permute2x128_si256, _mm256_permutevar8x32_epi32, _mm256_setr_epi32,
    _mm256_setzero_si256, _mm256_storeu_si256, _mm256_unpackhi_epi32, _mm256_unpackhi_epi64,
    _mm256_unpacklo_epi32, _mm256_unpacklo_epi64,
};

use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::{DynamicImage, RgbaImage};

const TILE: usize = 8;
const CHANNELS: usize = 4;

/// Executes the selected RGBA AVX2 transpose strategy.
///
/// # Safety
///
/// The caller must have prepared this strategy from an `ExecutionContext`
/// whose captured host capabilities include AVX2 and whose request guard
/// admits this image layout, size, and transpose method.
pub(crate) unsafe fn execute(
    image: &super::super::Input,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    let (width, height) = image.dimensions();
    let (reverse_x, reverse_y) = match method {
        TransposeMethod::Rotate90 => (true, false),
        TransposeMethod::Rotate270 => (false, true),
        TransposeMethod::Transpose => (false, false),
        TransposeMethod::Transverse => (true, true),
        _ => return Err(PilError::InternalError("AVX2 transpose selected for a non-axis method".into())),
    };
    let output_dims = crate::checked_dims::CheckedDims::new(height, width, CHANNELS as u8)?;
    let mut output = output_dims.alloc_buffer();
    let width = width as usize;
    let height = height as usize;
    let full_rows = width / TILE * TILE;
    let full_columns = height / TILE * TILE;

    // SAFETY: the prepared AVX2 strategy requires captured AVX2 capability,
    // exact RGBA storage, axis-swap methods, and dimensions of at least one
    // tile. The checked output and typed input bound every full-tile access.
    unsafe {
        transpose_full_tiles(
            image.as_raw(),
            &mut output,
            width,
            height,
            full_rows,
            full_columns,
            reverse_x,
            reverse_y,
        );
    }
    copy_bottom_edge(
        image.as_raw(),
        &mut output,
        width,
        height,
        full_rows,
        reverse_x,
        reverse_y,
    );
    copy_right_edge(
        image.as_raw(),
        &mut output,
        width,
        height,
        full_rows,
        full_columns,
        reverse_x,
        reverse_y,
    );

    let tiles = (full_rows / TILE) * (full_columns / TILE);
    let scalar_pixels = width * height - full_rows * full_columns;
    crate::compute::record_pipeline_operation_vector_blocks((tiles * TILE) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((scalar_pixels * CHANNELS) as u64);
    crate::compute::record_pipeline_operation_path("native-copy");
    let output = RgbaImage::from_raw(height as u32, width as u32, output)
        .ok_or_else(|| PilError::InternalError("transpose RGBA buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageRgba8(output))
}

fn copy_bottom_edge(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    full_rows: usize,
    reverse_x: bool,
    reverse_y: bool,
) {
    for output_y in full_rows..width {
        let source_x = if reverse_x { width - 1 - output_y } else { output_y };
        for output_x in 0..height {
            copy_pixel(source, output, width, height, source_x, output_y, output_x, reverse_y);
        }
    }
}

fn copy_right_edge(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    full_rows: usize,
    full_columns: usize,
    reverse_x: bool,
    reverse_y: bool,
) {
    for output_y in 0..full_rows {
        let source_x = if reverse_x { width - 1 - output_y } else { output_y };
        for output_x in full_columns..height {
            copy_pixel(source, output, width, height, source_x, output_y, output_x, reverse_y);
        }
    }
}

fn copy_pixel(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    source_x: usize,
    output_y: usize,
    output_x: usize,
    reverse_y: bool,
) {
    let source_y = if reverse_y { height - 1 - output_x } else { output_x };
    let source_start = (source_y * width + source_x) * CHANNELS;
    let output_start = (output_y * height + output_x) * CHANNELS;
    output[output_start..output_start + CHANNELS]
        .copy_from_slice(&source[source_start..source_start + CHANNELS]);
}

#[target_feature(enable = "avx2")]
unsafe fn transpose_full_tiles(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    full_rows: usize,
    full_columns: usize,
    reverse_x: bool,
    reverse_y: bool,
) {
    let source_stride = width * CHANNELS;
    let output_stride = height * CHANNELS;
    let reverse_lanes = _mm256_setr_epi32(7, 6, 5, 4, 3, 2, 1, 0);
    // SAFETY: the caller proves AVX2 availability and passes full tile bounds.
    unsafe {
        for output_x_start in (0..full_columns).step_by(TILE) {
            let source_y = if reverse_y {
                height - output_x_start - TILE
            } else {
                output_x_start
            };
            for output_y_start in (0..full_rows).step_by(TILE) {
                let source_x = if reverse_x {
                    width - output_y_start - TILE
                } else {
                    output_y_start
                };
                let columns = transpose_tile(source, width, source_y, source_x);
                for (column_index, mut column) in columns.into_iter().enumerate() {
                    if reverse_y {
                        column = _mm256_permutevar8x32_epi32(column, reverse_lanes);
                    }
                    let row = if reverse_x { TILE - 1 - column_index } else { column_index };
                    let start = (output_y_start + row) * output_stride + output_x_start * CHANNELS;
                    _mm256_storeu_si256(output.as_mut_ptr().add(start).cast(), column);
                }
            }
        }
    }
}

#[target_feature(enable = "avx2")]
unsafe fn transpose_tile(
    source: &[u8],
    width: usize,
    source_y: usize,
    source_x: usize,
) -> [core::arch::x86_64::__m256i; TILE] {
    let source_stride = width * CHANNELS;
    // SAFETY: the caller's complete-tile bounds place these eight 32-byte
    // loads wholly inside the exact source allocation.
    unsafe {
        let mut rows = [_mm256_setzero_si256(); TILE];
        for (row, value) in rows.iter_mut().enumerate() {
            let start = (source_y + row) * source_stride + source_x * CHANNELS;
            *value = _mm256_loadu_si256(source.as_ptr().add(start).cast());
        }
        let r01_lo = _mm256_unpacklo_epi32(rows[0], rows[1]);
        let r01_hi = _mm256_unpackhi_epi32(rows[0], rows[1]);
        let r23_lo = _mm256_unpacklo_epi32(rows[2], rows[3]);
        let r23_hi = _mm256_unpackhi_epi32(rows[2], rows[3]);
        let r45_lo = _mm256_unpacklo_epi32(rows[4], rows[5]);
        let r45_hi = _mm256_unpackhi_epi32(rows[4], rows[5]);
        let r67_lo = _mm256_unpacklo_epi32(rows[6], rows[7]);
        let r67_hi = _mm256_unpackhi_epi32(rows[6], rows[7]);
        let a0 = _mm256_unpacklo_epi64(r01_lo, r23_lo);
        let a1 = _mm256_unpackhi_epi64(r01_lo, r23_lo);
        let a2 = _mm256_unpacklo_epi64(r01_hi, r23_hi);
        let a3 = _mm256_unpackhi_epi64(r01_hi, r23_hi);
        let b0 = _mm256_unpacklo_epi64(r45_lo, r67_lo);
        let b1 = _mm256_unpackhi_epi64(r45_lo, r67_lo);
        let b2 = _mm256_unpacklo_epi64(r45_hi, r67_hi);
        let b3 = _mm256_unpackhi_epi64(r45_hi, r67_hi);
        [
            _mm256_permute2x128_si256::<0x20>(a0, b0),
            _mm256_permute2x128_si256::<0x20>(a1, b1),
            _mm256_permute2x128_si256::<0x20>(a2, b2),
            _mm256_permute2x128_si256::<0x20>(a3, b3),
            _mm256_permute2x128_si256::<0x31>(a0, b0),
            _mm256_permute2x128_si256::<0x31>(a1, b1),
            _mm256_permute2x128_si256::<0x31>(a2, b2),
            _mm256_permute2x128_si256::<0x31>(a3, b3),
        ]
    }
}

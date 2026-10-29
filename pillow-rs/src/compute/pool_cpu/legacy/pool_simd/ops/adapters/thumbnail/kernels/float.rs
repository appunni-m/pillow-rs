//! Native scalar-F SIMD thumbnail reduction kernels.

use super::super::{SIMD_RESIZE_LANES, simd_unsupported};
use crate::error::PilError;
use crate::ops::pil_resize::f32_samples_from_le_bytes;
use crate::raster::DynamicImage;
use wide::f32x8;

/// Reduce the scalar samples used by `Image.thumbnail` without interpreting
/// their four-byte storage as RGBA. The source gathers and block geometry are
/// scalar control work; each eight-lane block performs the average in the
/// scalar sample domain and writes only its valid prefix.
#[inline]
pub(super) fn thumbnail_f_reduce_value(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    factor_x: usize,
    factor_y: usize,
    output_x: usize,
    output_y: usize,
) -> f32 {
    let source_x = output_x * factor_x;
    let source_y = output_y * factor_y;
    let block_width = factor_x.min(source_width - source_x);
    let block_height = factor_y.min(source_height - source_y);
    let main_width = source_width / factor_x;
    let main_height = source_height / factor_y;
    let mut sum = 0.0f64;

    // Match Pillow's ImagingReduceNxN_32bpc ordering. Interior blocks add
    // float 2x2 groups before promoting each group to the double accumulator;
    // partial right/bottom blocks use the scalar corner helper instead.
    if output_x < main_width && output_y < main_height {
        let mut dy = 0;
        while dy + 1 < block_height {
            let mut dx = 0;
            while dx + 1 < block_width {
                let row0 = (source_y + dy) * source_width + source_x + dx;
                let row1 = (source_y + dy + 1) * source_width + source_x + dx;
                let quartet = ((source[row0] + source[row0 + 1]) + source[row1]) + source[row1 + 1];
                sum += f64::from(quartet);
                dx += 2;
            }
            if dx < block_width {
                let row0 = (source_y + dy) * source_width + source_x + dx;
                let row1 = (source_y + dy + 1) * source_width + source_x + dx;
                sum += f64::from(source[row0] + source[row1]);
            }
            dy += 2;
        }
        if dy < block_height {
            let row = (source_y + dy) * source_width + source_x;
            let mut dx = 0;
            while dx + 1 < block_width {
                sum += f64::from(source[row + dx] + source[row + dx + 1]);
                dx += 2;
            }
            if dx < block_width {
                sum += f64::from(source[row + dx]);
            }
        }
    } else {
        for dy in 0..block_height {
            let row = (source_y + dy) * source_width + source_x;
            for dx in 0..block_width {
                sum += f64::from(source[row + dx]);
            }
        }
    }

    (sum * (1.0 / (block_width * block_height) as f64)) as f32
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn simd_thumbnail_reduce_f_2x2_neon(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    output_height: usize,
    output: &mut Vec<u8>,
) -> Result<u64, PilError> {
    const OUTPUT_LANES: usize = 8;
    const NEON_LANES: usize = 4;

    let source_len = source_width
        .checked_mul(source_height)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let output_len = output_width
        .checked_mul(output_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if source.len() != source_len
        || source_width / 2 != output_width
        || source_height / 2 != output_height
        || !source_width.is_multiple_of(2)
        || !source_height.is_multiple_of(2)
        || !output_width.is_multiple_of(OUTPUT_LANES)
        || !output.is_empty()
        || output.capacity() < output_len
    {
        return Err(simd_unsupported("Thumbnail"));
    }

    let mut vector_blocks = 0u64;
    for output_y in 0..output_height {
        let source_row_top = output_y
            .checked_mul(2)
            .and_then(|row| row.checked_mul(source_width))
            .ok_or_else(|| simd_unsupported("Thumbnail"))?;
        let source_row_bottom = source_row_top
            .checked_add(source_width)
            .ok_or_else(|| simd_unsupported("Thumbnail"))?;
        for output_x in (0..output_width).step_by(OUTPUT_LANES) {
            let source_x = output_x
                .checked_mul(2)
                .ok_or_else(|| simd_unsupported("Thumbnail"))?;
            let top = source_row_top
                .checked_add(source_x)
                .ok_or_else(|| simd_unsupported("Thumbnail"))?;
            let bottom = source_row_bottom
                .checked_add(source_x)
                .ok_or_else(|| simd_unsupported("Thumbnail"))?;
            if top
                .checked_add(OUTPUT_LANES * 2)
                .is_none_or(|end| end > source_len)
                || bottom
                    .checked_add(OUTPUT_LANES * 2)
                    .is_none_or(|end| end > source_len)
            {
                return Err(simd_unsupported("Thumbnail"));
            }

            let mut quartets = [0.0f32; OUTPUT_LANES];
            // SAFETY: The shape checks above guarantee two complete source
            // rows for every output row. Each pair of `vld2q_f32` calls reads
            // sixteen initialized samples from the top or bottom row, bounded
            // by the checked `top`/`bottom` offsets. The NEON stores write four
            // initialized lanes into each half of the local eight-float array.
            // All pointers are valid for the complete vector access; the
            // AArch64 NEON structure loads permit the source slice's f32
            // alignment.
            unsafe {
                use core::arch::aarch64::{vaddq_f32, vld2q_f32, vst1q_f32};

                let top_first = vld2q_f32(source.as_ptr().add(top));
                let top_second = vld2q_f32(source.as_ptr().add(top + NEON_LANES * 2));
                let bottom_first = vld2q_f32(source.as_ptr().add(bottom));
                let bottom_second = vld2q_f32(source.as_ptr().add(bottom + NEON_LANES * 2));

                let quartet_first = vaddq_f32(
                    vaddq_f32(vaddq_f32(top_first.0, top_first.1), bottom_first.0),
                    bottom_first.1,
                );
                let quartet_second = vaddq_f32(
                    vaddq_f32(vaddq_f32(top_second.0, top_second.1), bottom_second.0),
                    bottom_second.1,
                );
                vst1q_f32(quartets.as_mut_ptr(), quartet_first);
                vst1q_f32(quartets.as_mut_ptr().add(NEON_LANES), quartet_second);
            }

            let values = quartets.map(|quartet| {
                let mut sum = 0.0f64;
                sum += f64::from(quartet);
                (sum * 0.25) as f32
            });
            #[cfg(target_endian = "little")]
            output.extend_from_slice(bytemuck::cast_slice(&values));
            #[cfg(target_endian = "big")]
            for value in values {
                output.extend_from_slice(&value.to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }

    Ok(vector_blocks)
}

pub(super) fn simd_thumbnail_reduce_f(
    img: &DynamicImage,
    factor_x: u32,
    factor_y: u32,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let factor_x = usize::try_from(factor_x).map_err(|_| simd_unsupported("Thumbnail"))?;
    let factor_y = usize::try_from(factor_y).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_pixels = source_width
        .checked_mul(source_height)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let source_len = source_pixels
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if !matches!(img, DynamicImage::ImageRgba8(_)) || img.as_bytes().len() != source_len {
        return Err(simd_unsupported("Thumbnail"));
    }
    let output_width = source_width.div_ceil(factor_x.max(1));
    let output_height = source_height.div_ceil(factor_y.max(1));
    let output_pixels = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let source = f32_samples_from_le_bytes(img.as_bytes(), source_len / 4);
    // The common 4× thumbnail first reduces each native F sample block to a
    // 2×2 average. Process eight independent output samples with vector adds,
    // preserving Pillow's left-to-right f32 quartet order before promoting
    // each result to its f64 accumulator. Odd dimensions and partial blocks
    // stay on the general scalar path below.
    if factor_x == 2
        && factor_y == 2
        && source_width % 2 == 0
        && source_height % 2 == 0
        && output_width == source_width / 2
        && output_height == source_height / 2
        && output_width % SIMD_RESIZE_LANES == 0
    {
        let (image, vector_blocks) = simd_thumbnail_reduce_f_even_2x2(
            &source,
            source_width,
            source_height,
            output_width,
            output_height,
            output_pixels,
        )?;
        return Ok((image, vector_blocks, 0));
    }
    simd_thumbnail_reduce_f_general(
        &source,
        source_width,
        source_height,
        factor_x,
        factor_y,
        output_width,
        output_height,
        output_pixels,
    )
}

fn simd_thumbnail_reduce_f_even_2x2(
    source: &[f32],
    source_width: usize,
    _source_height: usize,
    output_width: usize,
    output_height: usize,
    output_pixels: usize,
) -> Result<(DynamicImage, u64), PilError> {
    let output_len = output_pixels
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let mut output = Vec::with_capacity(output_len);
        let vector_blocks = simd_thumbnail_reduce_f_2x2_neon(
            source,
            source_width,
            _source_height,
            output_width,
            output_height,
            &mut output,
        )?;
        let result = crate::image_utils::raw_bytes_to_image(
            output_width as u32,
            output_height as u32,
            output,
            4,
        )?;
        Ok((result, vector_blocks))
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    {
        let mut output = Vec::with_capacity(output_len);
        let mut vector_blocks = 0u64;
        for output_y in 0..output_height {
            let source_row_top = output_y * 2 * source_width;
            let source_row_bottom = source_row_top + source_width;
            vector_blocks = vector_blocks.saturating_add(reduce_f_even_2x2_row(
                source,
                source_row_top,
                source_row_bottom,
                output_width,
                &mut output,
            ));
        }
        let result = crate::image_utils::raw_bytes_to_image(
            output_width as u32,
            output_height as u32,
            output,
            4,
        )?;
        Ok((result, vector_blocks))
    }
}

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
fn reduce_f_even_2x2_row(
    source: &[f32],
    source_row_top: usize,
    source_row_bottom: usize,
    output_width: usize,
    output: &mut Vec<u8>,
) -> u64 {
    let mut vector_blocks = 0u64;
    for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
        let source_x = output_x * 2;
        let top_left = f32x8::new(std::array::from_fn(|lane| {
            source[source_row_top + source_x + lane * 2]
        }));
        let top_right = f32x8::new(std::array::from_fn(|lane| {
            source[source_row_top + source_x + lane * 2 + 1]
        }));
        let bottom_left = f32x8::new(std::array::from_fn(|lane| {
            source[source_row_bottom + source_x + lane * 2]
        }));
        let bottom_right = f32x8::new(std::array::from_fn(|lane| {
            source[source_row_bottom + source_x + lane * 2 + 1]
        }));
        let quartets = (((top_left + top_right) + bottom_left) + bottom_right).to_array();
        let values = quartets.map(|quartet| {
            let mut sum = 0.0f64;
            sum += f64::from(quartet);
            (sum * 0.25) as f32
        });
        #[cfg(target_endian = "little")]
        output.extend_from_slice(bytemuck::cast_slice(&values));
        #[cfg(target_endian = "big")]
        for value in values {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    vector_blocks
}

fn simd_thumbnail_reduce_f_general(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    factor_x: usize,
    factor_y: usize,
    output_width: usize,
    output_height: usize,
    output_pixels: usize,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let mut output = Vec::with_capacity(
        output_pixels
            .checked_mul(4)
            .ok_or_else(|| simd_unsupported("Thumbnail"))?,
    );
    let mut vector_blocks = 0u64;
    for start in (0..output_pixels).step_by(SIMD_RESIZE_LANES) {
        let count = (output_pixels - start).min(SIMD_RESIZE_LANES);
        let mut values = [0.0f32; SIMD_RESIZE_LANES];
        for lane in 0..count {
            let index = start + lane;
            let output_y = index / output_width;
            let output_x = index % output_width;
            values[lane] = thumbnail_f_reduce_value(
                source,
                source_width,
                source_height,
                factor_x,
                factor_y,
                output_x,
                output_y,
            );
        }
        for value in f32x8::new(values).to_array().into_iter().take(count) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?;
    Ok((result, vector_blocks, 0))
}

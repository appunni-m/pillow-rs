//! Serial CPU I-mode thumbnail resize that fuses reducing-gap and resampling.

use crate::checked_dims::CheckedDims;
use crate::compute::ExecutionContext;

use crate::compute::pool_cpu::ops::resize::{
    i32_sample_bytes_from_native_storage, resample_kernel, resize_i_contiguous_eight_tap_weights,
    resize_i_horizontal_row, resize_i_round_up_to_i32, resize_i_sum_eight, resize_i_sum_general,
};

use crate::error::PilError;
#[cfg(target_arch = "x86_64")]
use crate::ops::pil_resize::X86FmaToken;

use crate::ops::pil_resize::{
    F64MulAdd, PortableFma, i32_samples_from_le_bytes, precompute_coeffs_f64, round_up,
};

use crate::pipeline::ResampleFilter;

use crate::raster::DynamicImage;

pub(in crate::compute::pool_cpu::ops::thumbnail) fn resize_i_thumbnail_reduce2x2(
    context: &ExecutionContext,
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    #[cfg(target_arch = "x86_64")]
    if let Some(fma) = X86FmaToken::from_context(context) {
        return resize_i_thumbnail_reduce2x2_with_fma(img, dst_w, dst_h, filter, &fma);
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = context;
    resize_i_thumbnail_reduce2x2_with_fma(img, dst_w, dst_h, filter, &PortableFma)
}

/// Stream native-I 2×2 reducing-gap rows directly into the existing serial
/// resize ring. Pillow still rounds and stores each reduced sample and each
/// horizontal intermediate as INT32; this only removes the full reduced image
/// buffer between those exact stages.
fn resize_i_thumbnail_reduce2x2_with_fma<F: F64MulAdd>(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    fma: &F,
) -> Result<DynamicImage, PilError> {
    let (source_width, source_height, source_bytes) = i32_sample_bytes_from_native_storage(img)?;
    if source_width % 2 != 0 || source_height % 2 != 0 {
        return Err(PilError::InternalError(
            "streamed I thumbnail reduction requires even source dimensions".into(),
        ));
    }
    let source_pixels =
        CheckedDims::new_allow_empty(source_width, source_height, 4)?.total_pixels();
    let source = i32_samples_from_le_bytes(source_bytes, source_pixels);
    let reduced_width = source_width / 2;
    let reduced_height = source_height / 2;
    let (kernel, support) = resample_kernel(&filter);
    let horizontal = precompute_coeffs_f64(dst_w, reduced_width, kernel, support);
    let vertical = precompute_coeffs_f64(dst_h, reduced_height, kernel, support);
    let fixed_horizontal_weights = resize_i_contiguous_eight_tap_weights(&horizontal.weights);
    let max_vertical_taps = vertical.weights.iter().map(Vec::len).max().unwrap_or(0);
    let ring_rows = max_vertical_taps
        .checked_next_power_of_two()
        .ok_or_else(|| {
            PilError::ValueError("resize_i: vertical coefficient span is too large".into())
        })?;
    let ring_samples = ring_rows
        .checked_mul(dst_w as usize)
        .ok_or_else(|| PilError::ValueError("resize_i: vertical ring is too large".into()))?;
    let mut ring = vec![0_i32; ring_samples];
    let mut reduced_row = vec![0_i32; reduced_width as usize];
    let output_dims = CheckedDims::new(dst_w, dst_h, 4)?;
    let output_stride = output_dims.row_stride();
    let mut output_bytes = output_dims.alloc_buffer();
    let ring_mask = ring_rows - 1;
    let source_width = source_width as usize;
    let mut next_source_row = 0_usize;

    for (output_y, output_row) in output_bytes.chunks_mut(output_stride).enumerate() {
        let first_vertical_row = vertical.xmin[output_y] as usize;
        let vertical_weights = &vertical.weights[output_y];
        let source_end = first_vertical_row + vertical_weights.len();
        while next_source_row < source_end {
            let top_start = next_source_row * 2 * source_width;
            let bottom_start = top_start + source_width;
            let top = &source[top_start..top_start + source_width];
            let bottom = &source[bottom_start..bottom_start + source_width];
            for (output_x, reduced) in reduced_row.iter_mut().enumerate() {
                let source_x = output_x * 2;
                let quartet = top[source_x]
                    .wrapping_add(top[source_x + 1])
                    .wrapping_add(bottom[source_x])
                    .wrapping_add(bottom[source_x + 1]);
                *reduced = round_up(f64::from(quartet) / 4.0) as i32;
            }

            let ring_start = (next_source_row & ring_mask) * dst_w as usize;
            let horizontal_row = &mut ring[ring_start..ring_start + dst_w as usize];
            resize_i_horizontal_row(
                &reduced_row,
                horizontal_row,
                &horizontal,
                fixed_horizontal_weights.as_ref(),
                fma,
            );
            next_source_row += 1;
        }

        if let Ok(weights) = <&[f64; 8]>::try_from(vertical_weights.as_slice()) {
            for (output_x, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = resize_i_sum_eight(
                    weights,
                    first_vertical_row,
                    |source_y| ring[(source_y & ring_mask) * dst_w as usize + output_x],
                    fma,
                );
                output.copy_from_slice(&resize_i_round_up_to_i32(accumulator).to_le_bytes());
            }
        } else {
            for (output_x, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = resize_i_sum_general(
                    vertical_weights,
                    first_vertical_row,
                    |source_y| ring[(source_y & ring_mask) * dst_w as usize + output_x],
                    fma,
                );
                output.copy_from_slice(&resize_i_round_up_to_i32(accumulator).to_le_bytes());
            }
        }
    }

    let output = crate::raster::RgbaImage::from_raw(dst_w, dst_h, output_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))?;
    Ok(DynamicImage::ImageRgba8(output))
}

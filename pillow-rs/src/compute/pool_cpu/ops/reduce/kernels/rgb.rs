//! Packed RGB reduction kernels and their sparse-row proof path.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::{DynamicImage, GenericImageView};

pub(in crate::compute::pool_cpu::ops) type SparseRgbReduction = (DynamicImage, Option<Vec<bool>>);
pub(in crate::compute::pool_cpu::ops) type SparseRgbReductionResult =
    Result<Option<SparseRgbReduction>, PilError>;

#[derive(Clone, Copy)]
struct RgbReduceRounding {
    multiplier: u32,
    amend: u32,
}

struct RgbReduceRowContext<'a> {
    source: &'a [u8],
    source_stride: usize,
    output_width: u32,
    x_factor: u32,
    y_factor: u32,
    main_width: u32,
    main_height: u32,
    right_width: u32,
    bottom_height: u32,
    full: RgbReduceRounding,
    right: RgbReduceRounding,
    bottom: RgbReduceRounding,
    corner: RgbReduceRounding,
}

/// Reduce native RGB blocks without the generic per-channel mode checks.
///
/// Keep Pillow's 24-bit reciprocal-and-amend rounding unchanged. Interior,
/// right-edge, bottom-edge, and corner blocks each retain their own divisor.
pub(in crate::compute::pool_cpu::ops::reduce) fn execute_reduce_rgb(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    execute_reduce_rgb_with_sparse_rows(img, x_factor, y_factor, explicit_mode)
        .map(|reduced| reduced.map(|(image, _)| image))
}

fn try_reduce_sparse_2x2(
    img: &DynamicImage,
    factors: (u32, u32),
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> SparseRgbReductionResult {
    let ((fx, fy), (width, height), (new_width, new_height)) =
        (factors, source_dimensions, output_dimensions);
    if fx != 2 || fy != 2 || width % 2 != 0 || height % 2 != 0 {
        return Ok(None);
    }
    Ok(
        execute_reduce_rgb_sparse_2x2_with_rows(img, new_width, new_height)?
            .map(|(image, nonzero_rows)| (image, Some(nonzero_rows))),
    )
}

/// Reduce RGB while retaining exact active rows when the sparse 2×2 path is
/// selected. Thumbnail can use that proof to skip zero rows in both resample
/// passes without rescanning the reduced image.
pub(in crate::compute::pool_cpu::ops) fn execute_reduce_rgb_with_sparse_rows(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> SparseRgbReductionResult {
    if explicit_mode != Some("RGB") || !matches!(img, DynamicImage::ImageRgb8(_)) {
        return Ok(None);
    }

    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let Some(full_divider) = fx.checked_mul(fy) else {
        return Ok(None);
    };
    // Keep both the channel sums plus amend and their fixed-point multiply
    // within u32 for the hot path. Larger factors retain the generic u64 code.
    if full_divider > u32::MAX / 256 {
        return Ok(None);
    }
    let (width, height) = img.dimensions();
    let new_width = width.div_ceil(fx);
    let new_height = height.div_ceil(fy);
    let source_stride = (width as usize).checked_mul(3);
    let expected_source_len = source_stride.and_then(|stride| stride.checked_mul(height as usize));
    let source = img.as_bytes();
    if expected_source_len != Some(source.len()) {
        return Ok(None);
    }

    if let Some(reduced) =
        try_reduce_sparse_2x2(img, (fx, fy), (width, height), (new_width, new_height))?
    {
        return Ok(Some(reduced));
    }

    let mut output = CheckedDims::new(new_width, new_height, 3)?.alloc_buffer();
    if new_width == 0 || new_height == 0 {
        return raw_bytes_to_image(new_width, new_height, output, 3)
            .map(|image| Some((image, None)));
    }

    let division_multiplier = |divider: u32| -> u32 {
        // Pillow's division_UINT32 reciprocal: 2^32 / (256 * divider).
        ((1u128 << 32) / (u128::from(divider) * 256)) as u32
    };
    let main_width = width / fx;
    let main_height = height / fy;
    let right_width = width % fx;
    let bottom_height = height % fy;
    let full_multiplier = division_multiplier(full_divider);
    let full_amend = full_divider / 2;
    let right_divider = right_width * fy;
    let right_multiplier = division_multiplier(right_divider.max(1));
    let right_amend = right_divider / 2;
    let bottom_divider = fx * bottom_height;
    let bottom_multiplier = division_multiplier(bottom_divider.max(1));
    let bottom_amend = bottom_divider / 2;
    let corner_divider = right_width * bottom_height;
    let corner_multiplier = division_multiplier(corner_divider.max(1));
    let corner_amend = corner_divider / 2;
    let source_stride = source_stride.expect("RGB source stride was checked");

    // Pillow's reducing-gap thumbnail path commonly reduces RGB by 2×2.
    // Keep the dense even-sized specialization ahead of the general edge path.
    if fx == 2 && fy == 2 && width % 2 == 0 && height % 2 == 0 {
        reduce_rgb_dense_even_2x2(
            source,
            source_stride,
            width,
            height,
            new_width,
            new_height,
            &mut output,
        );
        return raw_bytes_to_image(new_width, new_height, output, 3)
            .map(|image| Some((image, None)));
    }

    let context = RgbReduceRowContext {
        source,
        source_stride,
        output_width: new_width,
        x_factor: fx,
        y_factor: fy,
        main_width,
        main_height,
        right_width,
        bottom_height,
        full: RgbReduceRounding {
            multiplier: full_multiplier,
            amend: full_amend,
        },
        right: RgbReduceRounding {
            multiplier: right_multiplier,
            amend: right_amend,
        },
        bottom: RgbReduceRounding {
            multiplier: bottom_multiplier,
            amend: bottom_amend,
        },
        corner: RgbReduceRounding {
            multiplier: corner_multiplier,
            amend: corner_amend,
        },
    };
    execute_reduce_rgb_general(&context, width, height, new_height, output)
}

/// Reduce large sparse RGB sources by updating only blocks with nonzero input.
/// Dense sources stop at the density bound and use the ordinary row kernel.
pub(super) fn execute_reduce_rgb_sparse_2x2_with_rows(
    img: &DynamicImage,
    new_width: u32,
    new_height: u32,
) -> Result<Option<(DynamicImage, Vec<bool>)>, PilError> {
    let (width, height) = img.dimensions();
    let source = img.as_bytes();
    let source_pixels = source.len() / 3;
    if source_pixels < 512 * 512 || width == 0 || height == 0 {
        return Ok(None);
    }
    // Scanning remains worthwhile only when very few input pixels contribute
    // nonzero samples. Stop early on denser images so they pay only a bounded
    // prefix scan before continuing through the established reducer.
    let maximum_nonzero_pixels = source_pixels / 1024;
    let mut nonzero_pixels = 0usize;
    let mut block_sums = std::collections::HashMap::<usize, [u16; 3]>::new();
    let mut accumulate_pixel = |source_index: usize, pixel: &[u8]| {
        if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
            return true;
        }
        nonzero_pixels += 1;
        if nonzero_pixels > maximum_nonzero_pixels {
            return false;
        }

        let source_x = source_index % width as usize;
        let source_y = source_index / width as usize;
        let block_index = (source_y / 2) * new_width as usize + source_x / 2;
        let sums = block_sums.entry(block_index).or_insert([0; 3]);
        sums[0] += u16::from(pixel[0]);
        sums[1] += u16::from(pixel[1]);
        sums[2] += u16::from(pixel[2]);
        true
    };

    // The common sparse case contains long runs of black RGB pixels. Check
    // three native words at a time, which covers eight complete pixels, before
    // entering the per-pixel accounting loop. Keep the pixel loop for any
    // nonzero block so channel order, density bounds, and edge coordinates
    // remain identical to the ordinary sparse scan.
    let grouped_pixels = source.len() / 24 * 8;
    for block_index in 0..source.len() / 24 {
        let byte_start = block_index * 24;
        let block = &source[byte_start..byte_start + 24];
        let words_are_zero = [0usize, 8, 16].into_iter().all(|offset| {
            u64::from_ne_bytes(
                block[offset..offset + 8]
                    .try_into()
                    .expect("each sparse RGB scan word has eight bytes"),
            ) == 0
        });
        if words_are_zero {
            continue;
        }
        for (pixel_offset, pixel) in block.chunks_exact(3).enumerate() {
            let source_index = block_index * 8 + pixel_offset;
            if !accumulate_pixel(source_index, pixel) {
                return Ok(None);
            }
        }
    }
    for (pixel_offset, pixel) in source[grouped_pixels * 3..].chunks_exact(3).enumerate() {
        if !accumulate_pixel(grouped_pixels + pixel_offset, pixel) {
            return Ok(None);
        }
    }

    let mut output = CheckedDims::new(new_width, new_height, 3)?.alloc_buffer();
    let mut nonzero_rows = vec![false; new_height as usize];
    for (block_index, sums) in block_sums {
        let output_index = block_index * 3;
        let pixel = [
            ((sums[0] + 2) >> 2) as u8,
            ((sums[1] + 2) >> 2) as u8,
            ((sums[2] + 2) >> 2) as u8,
        ];
        output[output_index..output_index + 3].copy_from_slice(&pixel);
        if pixel != [0; 3] {
            nonzero_rows[block_index / new_width as usize] = true;
        }
    }

    raw_bytes_to_image(new_width, new_height, output, 3).map(|image| Some((image, nonzero_rows)))
}

#[cfg(test)]
pub(in crate::compute::pool_cpu::ops::reduce) fn execute_reduce_rgb_sparse_2x2(
    img: &DynamicImage,
    new_width: u32,
    new_height: u32,
) -> Result<Option<DynamicImage>, PilError> {
    execute_reduce_rgb_sparse_2x2_with_rows(img, new_width, new_height)
        .map(|reduced| reduced.map(|(image, _)| image))
}

#[inline(always)]
fn write_reduce_rgb_block(
    context: &RgbReduceRowContext<'_>,
    row: &mut [u8],
    output_x: u32,
    source_x: u32,
    source_y: usize,
    block_width: u32,
    block_height: u32,
    rounding: RgbReduceRounding,
) {
    let mut red_sum = 0u32;
    let mut green_sum = 0u32;
    let mut blue_sum = 0u32;
    let mut source_row_start = source_y * context.source_stride + source_x as usize * 3;
    for _ in 0..block_height {
        let mut red_row_sum = 0u32;
        let mut green_row_sum = 0u32;
        let mut blue_row_sum = 0u32;
        let mut source_index = source_row_start;
        for _ in 0..block_width {
            red_row_sum += u32::from(context.source[source_index]);
            green_row_sum += u32::from(context.source[source_index + 1]);
            blue_row_sum += u32::from(context.source[source_index + 2]);
            source_index += 3;
        }
        red_sum += red_row_sum;
        green_sum += green_row_sum;
        blue_sum += blue_row_sum;
        source_row_start += context.source_stride;
    }

    let destination_index = output_x as usize * 3;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
    )]
    let red = ((red_sum + rounding.amend) * rounding.multiplier) >> 24;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
    )]
    let green = ((green_sum + rounding.amend) * rounding.multiplier) >> 24;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
    )]
    let blue = ((blue_sum + rounding.amend) * rounding.multiplier) >> 24;
    let red = red as u8;
    let green = green as u8;
    let blue = blue as u8;
    row[destination_index] = red;
    row[destination_index + 1] = green;
    row[destination_index + 2] = blue;
}

#[inline(always)]
fn process_reduce_rgb_row(context: &RgbReduceRowContext<'_>, y: u32, row: &mut [u8]) {
    let full_y = y < context.main_height;
    let source_y = if full_y {
        (y * context.y_factor) as usize
    } else {
        (context.main_height * context.y_factor) as usize
    };
    let block_height = if full_y {
        context.y_factor
    } else {
        context.bottom_height
    };
    let (interior_rounding, edge_rounding) = if full_y {
        (context.full, context.right)
    } else {
        (context.bottom, context.corner)
    };
    for x in 0..context.main_width {
        write_reduce_rgb_block(
            context,
            row,
            x,
            x * context.x_factor,
            source_y,
            context.x_factor,
            block_height,
            interior_rounding,
        );
    }
    if context.right_width != 0 {
        write_reduce_rgb_block(
            context,
            row,
            context.main_width,
            context.main_width * context.x_factor,
            source_y,
            context.right_width,
            block_height,
            edge_rounding,
        );
    }
}

fn execute_reduce_rgb_general(
    context: &RgbReduceRowContext<'_>,
    _width: u32,
    _height: u32,
    new_height: u32,
    mut output: Vec<u8>,
) -> Result<Option<SparseRgbReduction>, PilError> {
    let output_stride = context.output_width as usize * 3;
    #[cfg(feature = "parallel")]
    {
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (_width as usize).saturating_mul(_height as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                &mut output,
                output_stride,
                new_height as usize,
                |_row_start, _row_end, y, row| {
                    process_reduce_rgb_row(context, y, row);
                }
            );
        } else {
            for y in 0..new_height {
                let start = y as usize * output_stride;
                process_reduce_rgb_row(context, y, &mut output[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_height {
        let start = y as usize * output_stride;
        process_reduce_rgb_row(context, y, &mut output[start..start + output_stride]);
    }

    raw_bytes_to_image(context.output_width, new_height, output, 3).map(|image| Some((image, None)))
}

#[inline(always)]
fn reduce_rgb_dense_even_2x2(
    source: &[u8],
    source_stride: usize,
    _width: u32,
    _height: u32,
    new_width: u32,
    new_height: u32,
    output: &mut [u8],
) {
    let output_stride = new_width as usize * 3;
    let process_row = |output_y: usize, output_row: &mut [u8]| {
        let source_top = output_y * 2 * source_stride;
        let source_bottom = source_top + source_stride;
        let top_row = &source[source_top..source_top + source_stride];
        let bottom_row = &source[source_bottom..source_bottom + source_stride];

        for (block_x, output_pixel) in output_row.chunks_exact_mut(3).enumerate() {
            let source_x = block_x * 6;
            let top_right = source_x + 3;
            for channel in 0..3 {
                let sum = u32::from(top_row[source_x + channel])
                    + u32::from(top_row[top_right + channel])
                    + u32::from(bottom_row[source_x + channel])
                    + u32::from(bottom_row[top_right + channel]);
                // For a 2×2 block Pillow's reciprocal is exactly `(sum + 2) / 4`.
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "four byte samples plus the Pillow rounding bias fit u32"
                )]
                let average = (sum + 2) >> 2;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the Pillow 2x2 average is bounded to one byte"
                )]
                {
                    output_pixel[channel] = average as u8;
                }
            }
        }
    };

    #[cfg(feature = "parallel")]
    {
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (_width as usize).saturating_mul(_height as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                output,
                output_stride,
                new_height as usize,
                |_row_start, _row_end, y, row| { process_row(y as usize, row) }
            );
        } else {
            for y in 0..new_height as usize {
                let start = y * output_stride;
                process_row(y, &mut output[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_height as usize {
        let start = y * output_stride;
        process_row(y, &mut output[start..start + output_stride]);
    }
}

//! Exact RGB byte reducer and its dense and sparse CPU specializations.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

use super::general::reduce_bytes;

type SparseRgbReduction = (Vec<u8>, Vec<bool>);

#[derive(Clone, Copy)]
pub(super) struct RgbReduceRounding {
    pub(super) multiplier: u32,
    pub(super) amend: u32,
}

pub(crate) struct RgbReduceRowContext<'a> {
    pub(super) source: &'a [u8],
    pub(super) source_stride: usize,
    pub(crate) output_width: u32,
    pub(super) x_factor: u32,
    pub(super) y_factor: u32,
    pub(crate) main_width: u32,
    pub(crate) main_height: u32,
    pub(super) right_width: u32,
    pub(super) bottom_height: u32,
    pub(super) full: RgbReduceRounding,
    pub(super) right: RgbReduceRounding,
    pub(super) bottom: RgbReduceRounding,
    pub(super) corner: RgbReduceRounding,
}

#[derive(Clone, Copy)]
struct RgbGeometry {
    main_width: u32,
    main_height: u32,
    right_width: u32,
    bottom_height: u32,
    full: RgbReduceRounding,
    right: RgbReduceRounding,
    bottom: RgbReduceRounding,
    corner: RgbReduceRounding,
}

fn division_multiplier(divider: u32) -> u32 {
    ((1u128 << 32) / (u128::from(divider) * 256)) as u32
}

fn rounding(divider: u32) -> RgbReduceRounding {
    RgbReduceRounding {
        multiplier: division_multiplier(divider.max(1)),
        amend: divider / 2,
    }
}

fn rgb_geometry(width: u32, height: u32, x_factor: u32, y_factor: u32) -> RgbGeometry {
    let main_width = width / x_factor;
    let main_height = height / y_factor;
    let right_width = width % x_factor;
    let bottom_height = height % y_factor;
    RgbGeometry {
        main_width,
        main_height,
        right_width,
        bottom_height,
        full: rounding(x_factor * y_factor),
        right: rounding(right_width * y_factor),
        bottom: rounding(x_factor * bottom_height),
        corner: rounding(right_width * bottom_height),
    }
}

pub(crate) fn reduce_rgb_row_context<'a>(
    source: &'a [u8],
    width: u32,
    height: u32,
    x_factor: u32,
    y_factor: u32,
) -> RgbReduceRowContext<'a> {
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let geometry = rgb_geometry(width, height, x_factor, y_factor);
    RgbReduceRowContext {
        source,
        source_stride: width as usize * 3,
        output_width: width.div_ceil(x_factor),
        x_factor,
        y_factor,
        main_width: geometry.main_width,
        main_height: geometry.main_height,
        right_width: geometry.right_width,
        bottom_height: geometry.bottom_height,
        full: geometry.full,
        right: geometry.right,
        bottom: geometry.bottom,
        corner: geometry.corner,
    }
}

fn try_sparse_2x2(
    source: &[u8],
    width: u32,
    height: u32,
    output_width: u32,
    output_height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Option<SparseRgbReduction>, PilError> {
    if x_factor != 2 || y_factor != 2 || width % 2 != 0 || height % 2 != 0 {
        return Ok(None);
    }
    reduce_sparse_2x2(source, width, height, output_width, output_height)
}

pub(crate) fn reduce_sparse_2x2_if_applicable(
    source: &[u8],
    width: u32,
    height: u32,
    output_width: u32,
    output_height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Option<Vec<u8>>, PilError> {
    try_sparse_2x2(
        source,
        width,
        height,
        output_width,
        output_height,
        x_factor,
        y_factor,
    )
    .map(|reduction| reduction.map(|(bytes, _)| bytes))
}

/// Keeps exact active-row facts for the thumbnail reducing-gap consumer.
fn reduce_rgb_with_sparse_rows(
    source: &[u8],
    width: u32,
    height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Option<(Vec<u8>, Option<Vec<bool>>)>, PilError> {
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let output_width = width.div_ceil(x_factor);
    let output_height = height.div_ceil(y_factor);
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(3));
    if expected_len != Some(source.len()) {
        return Ok(None);
    }
    let Some(area) = x_factor.checked_mul(y_factor) else {
        return Ok(None);
    };
    if area > u32::MAX / 256 {
        return Ok(None);
    }
    if let Some((bytes, rows)) = try_sparse_2x2(
        source,
        width,
        height,
        output_width,
        output_height,
        x_factor,
        y_factor,
    )? {
        return Ok(Some((bytes, Some(rows))));
    }
    reduce_rgb_dense_or_general(
        source,
        width,
        height,
        output_width,
        output_height,
        x_factor,
        y_factor,
    )
    .map(|bytes| Some((bytes, None)))
}

/// Reduces a native `RgbImage`; no logical mode tests or conversions are needed.
pub(crate) fn reduce_rgb(
    source: &[u8],
    width: u32,
    height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Vec<u8>, PilError> {
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    if let Some((bytes, _)) =
        reduce_rgb_with_sparse_rows(source, width, height, x_factor, y_factor)?
    {
        return Ok(bytes);
    }
    reduce_bytes::<3, false>(source, width, height, x_factor, y_factor)
}

fn reduce_rgb_dense_or_general(
    source: &[u8],
    width: u32,
    height: u32,
    output_width: u32,
    output_height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Vec<u8>, PilError> {
    let mut output = CheckedDims::new(output_width, output_height, 3)?.alloc_buffer();
    if output_width == 0 || output_height == 0 {
        return Ok(output);
    }
    if x_factor == 2 && y_factor == 2 && width % 2 == 0 && height % 2 == 0 {
        super::kernels::reduce_rgb_dense_even_2x2(
            source,
            width as usize * 3,
            output_width,
            output_height,
            &mut output,
        );
        return Ok(output);
    }
    let context = reduce_rgb_row_context(source, width, height, x_factor, y_factor);
    super::kernels::reduce_rgb_general(&context, output_height, &mut output);
    Ok(output)
}

fn reduce_sparse_2x2(
    source: &[u8],
    width: u32,
    height: u32,
    output_width: u32,
    output_height: u32,
) -> Result<Option<SparseRgbReduction>, PilError> {
    let pixels = source.len() / 3;
    if pixels < 512 * 512 || width == 0 || height == 0 {
        return Ok(None);
    }
    let maximum_nonzero_pixels = pixels / 1024;
    let Some(block_sums) = sparse_block_sums(source, width, output_width, maximum_nonzero_pixels)
    else {
        return Ok(None);
    };
    let mut output = CheckedDims::new(output_width, output_height, 3)?.alloc_buffer();
    let mut nonzero_rows = vec![false; output_height as usize];
    for (block_index, sums) in block_sums {
        let output_index = block_index * 3;
        let pixel = [
            ((sums[0] + 2) >> 2) as u8,
            ((sums[1] + 2) >> 2) as u8,
            ((sums[2] + 2) >> 2) as u8,
        ];
        output[output_index..output_index + 3].copy_from_slice(&pixel);
        if pixel != [0; 3] {
            nonzero_rows[block_index / output_width as usize] = true;
        }
    }
    Ok(Some((output, nonzero_rows)))
}

fn sparse_block_sums(
    source: &[u8],
    width: u32,
    output_width: u32,
    maximum_nonzero_pixels: usize,
) -> Option<std::collections::HashMap<usize, [u16; 3]>> {
    let mut count = 0usize;
    let mut sums = std::collections::HashMap::<usize, [u16; 3]>::new();
    let mut accumulate = |source_index: usize, pixel: &[u8]| {
        if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
            return true;
        }
        count += 1;
        if count > maximum_nonzero_pixels {
            return false;
        }
        let source_x = source_index % width as usize;
        let source_y = source_index / width as usize;
        let block_index = (source_y / 2) * output_width as usize + source_x / 2;
        let block = sums.entry(block_index).or_insert([0; 3]);
        block[0] += u16::from(pixel[0]);
        block[1] += u16::from(pixel[1]);
        block[2] += u16::from(pixel[2]);
        true
    };
    let grouped_pixels = source.len() / 24 * 8;
    for group in 0..source.len() / 24 {
        let byte_start = group * 24;
        let bytes = &source[byte_start..byte_start + 24];
        if words_are_zero(bytes) {
            continue;
        }
        for (offset, pixel) in bytes.chunks_exact(3).enumerate() {
            if !accumulate(group * 8 + offset, pixel) {
                return None;
            }
        }
    }
    for (offset, pixel) in source[grouped_pixels * 3..].chunks_exact(3).enumerate() {
        if !accumulate(grouped_pixels + offset, pixel) {
            return None;
        }
    }
    Some(sums)
}

fn words_are_zero(bytes: &[u8]) -> bool {
    [0usize, 8, 16].into_iter().all(|offset| {
        u64::from_ne_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("each sparse RGB scan word has eight bytes"),
        ) == 0
    })
}

//! LA-mode three-pass GaussianBlur vector kernels over native `[L, A]` bytes.

use crate::checked_dims::CheckedDims;
use crate::compute::helpers::filters::gaussian_blur_three_pass_radius;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage};
use wide::{i16x8, i32x8, u8x16, u16x16, u32x16};

const LANES: usize = 16;
const PIXELS_PER_VECTOR: usize = LANES / 2;
const BLUR_SCALE: u32 = 1 << 24;
const BLUR_BIAS: u32 = 1 << 23;

#[inline(always)]
fn load_u8x16(source: &[u8], start: usize) -> u8x16 {
    bytemuck::pod_read_unaligned(&source[start..start + LANES])
}

#[inline(always)]
fn load_padded_u8x16(source: &[u8], start: usize, count: usize) -> u8x16 {
    let mut bytes = [0u8; LANES];
    bytes[..count].copy_from_slice(&source[start..start + count]);
    u8x16::new(bytes)
}

#[inline(always)]
fn pack_bytes(values: u32x16) -> [u8; LANES] {
    let [low, high]: [i32x8; 2] = bytemuck::cast(values);
    let low = i16x8::from_i32x8_saturate(low);
    let high = i16x8::from_i32x8_saturate(high);
    bytemuck::cast(u8x16::narrow_i16x8(low, high))
}

#[inline(always)]
fn scalar_horizontal_pixel(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    x: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let left = x.saturating_sub(1);
    let right = x.saturating_add(1).min(width - 1);
    let far_left = x.saturating_sub(2);
    let far_right = x.saturating_add(2).min(width - 1);
    let center = x * 2;
    for channel in 0..2 {
        let sum = u32::from(source[left * 2 + channel])
            + u32::from(source[center + channel])
            + u32::from(source[right * 2 + channel]);
        let edge =
            u32::from(source[far_left * 2 + channel]) + u32::from(source[far_right * 2 + channel]);
        let weighted = sum
            .wrapping_mul(whole_weight)
            .wrapping_add(edge.wrapping_mul(fractional_weight))
            .wrapping_add(BLUR_BIAS);
        destination[center + channel] = (weighted >> 24) as u8;
    }
}

fn horizontal_radius_one_row(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let interior_end = width.saturating_sub(2);
    let prefix_end = width.min(2);
    for x in 0..prefix_end {
        scalar_horizontal_pixel(
            source,
            destination,
            width,
            x,
            whole_weight,
            fractional_weight,
        );
    }
    let mut x = 2usize;
    while x + PIXELS_PER_VECTOR <= interior_end {
        let byte_start = x * 2;
        let load = |pixel: usize| u32x16::from(u16x16::from(load_u8x16(source, pixel * 2)));
        let neighbors = load(x - 1) + load(x) + load(x + 1);
        let edges = if fractional_weight == 0 {
            u32x16::splat(0)
        } else {
            load(x - 2) + load(x + 2)
        };
        let weighted = neighbors * u32x16::splat(whole_weight)
            + edges * u32x16::splat(fractional_weight)
            + u32x16::splat(BLUR_BIAS);
        let packed = pack_bytes(weighted >> 24u32);
        destination[byte_start..byte_start + LANES].copy_from_slice(&packed);
        x += PIXELS_PER_VECTOR;
    }
    for x in x..interior_end {
        scalar_horizontal_pixel(
            source,
            destination,
            width,
            x,
            whole_weight,
            fractional_weight,
        );
    }
    for x in interior_end.max(prefix_end)..width {
        scalar_horizontal_pixel(
            source,
            destination,
            width,
            x,
            whole_weight,
            fractional_weight,
        );
    }
}

fn horizontal_radius_one_three_passes(
    source: &[u8],
    destination: &mut [u8],
    dimensions: CheckedDims,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = dimensions.row_stride();
    let mut first = vec![0u8; row_stride];
    let mut second = vec![0u8; row_stride];
    let width = dimensions.width as usize;
    for y in 0..dimensions.height as usize {
        let start = y * row_stride;
        let end = start + row_stride;
        let source_row = &source[start..end];
        let output_row = &mut destination[start..end];
        horizontal_radius_one_row(
            source_row,
            &mut first,
            width,
            whole_weight,
            fractional_weight,
        );
        horizontal_radius_one_row(&first, &mut second, width, whole_weight, fractional_weight);
        horizontal_radius_one_row(&second, output_row, width, whole_weight, fractional_weight);
    }
    let interior_end = width.saturating_sub(2);
    let blocks_per_row = interior_end.saturating_sub(2) / PIXELS_PER_VECTOR;
    crate::compute::record_pipeline_operation_vector_blocks(
        blocks_per_row
            .saturating_mul(dimensions.height as usize)
            .saturating_mul(3) as u64,
    );
    let scalar_tail = width
        .saturating_sub(blocks_per_row * PIXELS_PER_VECTOR)
        .saturating_mul(dimensions.height as usize)
        .saturating_mul(3);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
}

fn vertical_radius_one_pass(
    source: &[u8],
    destination: &mut [u8],
    dimensions: CheckedDims,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = dimensions.row_stride();
    let height = dimensions.height as usize;
    let last_y = height - 1;
    for y in 0..height {
        let sum_top = y.saturating_sub(1) * row_stride;
        let sum_middle = y * row_stride;
        let sum_bottom = y.saturating_add(1).min(last_y) * row_stride;
        let edge_top = y.saturating_sub(2) * row_stride;
        let edge_bottom = y.saturating_add(2).min(last_y) * row_stride;
        let output_row = y * row_stride;
        for x in (0..row_stride).step_by(LANES) {
            let count = (row_stride - x).min(LANES);
            let load = |base: usize| {
                if count == LANES {
                    u32x16::from(u16x16::from(load_u8x16(source, base + x)))
                } else {
                    u32x16::from(u16x16::from(load_padded_u8x16(source, base + x, count)))
                }
            };
            let sum = load(sum_top) + load(sum_middle) + load(sum_bottom);
            let edges = if fractional_weight == 0 {
                u32x16::splat(0)
            } else {
                load(edge_top) + load(edge_bottom)
            };
            let weighted = sum * u32x16::splat(whole_weight)
                + edges * u32x16::splat(fractional_weight)
                + u32x16::splat(BLUR_BIAS);
            let packed = pack_bytes(weighted >> 24u32);
            destination[output_row + x..output_row + x + count].copy_from_slice(&packed[..count]);
        }
    }
    let vector_blocks = row_stride.div_ceil(LANES).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
}

pub(in crate::compute::pool_cpu::ops::filter::luma_a8) fn execute_radius_one(
    image: &DynamicImage,
    sigma: f32,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLumaA8(input) = image else {
        return Err(PilError::InternalError(
            "LA GaussianBlur strategy received a mismatched raster variant".into(),
        ));
    };
    let radius = gaussian_blur_three_pass_radius(sigma.abs());
    if !sigma.is_finite() || !(1.0..2.0).contains(&radius) {
        return Err(PilError::InternalError(
            "LA GaussianBlur strategy received an unsupported radius".into(),
        ));
    }
    let dimensions = CheckedDims::new(input.width(), input.height(), 2)?;
    if dimensions.width == 0
        || dimensions.height == 0
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "LA GaussianBlur strategy received invalid native dimensions".into(),
        ));
    }
    let whole_weight = (BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    let fractional_weight = BLUR_SCALE.wrapping_sub(3u32.wrapping_mul(whole_weight)) / 2;
    let output = if super::super::super::helpers::native_small_uniform_bytes(
        input.as_raw(),
        dimensions.width,
        dimensions.height,
        2,
        super::super::super::helpers::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        let mut output = vec![0; input.as_raw().len()];
        let (blocks, scalar_tail) =
            crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
                input.as_raw(),
                &mut output,
            )
            .ok_or_else(|| {
                PilError::InternalError("LA GaussianBlur identity copy shape mismatch".into())
            })?;
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        output
    } else {
        crate::compute::record_pipeline_operation_path("vector");
        let mut work = vec![0u8; dimensions.total_bytes()];
        let mut scratch = vec![0u8; dimensions.total_bytes()];
        horizontal_radius_one_three_passes(
            input.as_raw(),
            &mut work,
            dimensions,
            whole_weight,
            fractional_weight,
        );
        for _ in 0..3 {
            vertical_radius_one_pass(
                &work,
                &mut scratch,
                dimensions,
                whole_weight,
                fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
        work
    };
    GrayAlphaImage::from_raw(input.width(), input.height(), output)
        .map(DynamicImage::ImageLumaA8)
        .ok_or_else(|| PilError::InternalError("LA GaussianBlur output shape mismatch".into()))
}

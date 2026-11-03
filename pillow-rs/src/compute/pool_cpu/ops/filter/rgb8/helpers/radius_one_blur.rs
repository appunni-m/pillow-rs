//! RGB GaussianBlur and BoxBlur radius-one kernels over native interleaved bytes.

use crate::checked_dims::CheckedDims;
use crate::compute::helpers::filters::gaussian_blur_three_pass_radius;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};
use super::super::super::ops::FilterRequest;
use wide::{i16x8, i32x8, u8x16, u16x16, u32x16};

const LANES: usize = 16;
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
fn scalar_horizontal_sample(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    byte_index: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let x = byte_index / 3;
    let channel = byte_index % 3;
    let left = x.saturating_sub(1);
    let right = x.saturating_add(1).min(width - 1);
    let far_left = x.saturating_sub(2);
    let far_right = x.saturating_add(2).min(width - 1);
    let sample = |pixel: usize| u32::from(source[pixel * 3 + channel]);
    let neighbors = sample(left) + sample(x) + sample(right);
    let edges = if fractional_weight == 0 {
        0
    } else {
        sample(far_left) + sample(far_right)
    };
    let weighted = neighbors
        .wrapping_mul(whole_weight)
        .wrapping_add(edges.wrapping_mul(fractional_weight))
        .wrapping_add(BLUR_BIAS);
    destination[byte_index] = (weighted >> 24) as u8;
}

#[inline]
fn horizontal_radius_one_row(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    horizontal_radius_one_row_with_vectors(
        source,
        destination,
        width,
        whole_weight,
        fractional_weight,
        u32x16::splat(whole_weight),
        u32x16::splat(fractional_weight),
        u32x16::splat(BLUR_BIAS),
    );
}

#[inline]
fn horizontal_radius_one_row_with_vectors(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
    whole_weight_vector: u32x16,
    fractional_weight_vector: u32x16,
    rounding_bias: u32x16,
) {
    let row_stride = width * 3;
    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    for byte_index in 0..prefix_end {
        scalar_horizontal_sample(
            source,
            destination,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }
    let mut byte_index = prefix_end;
    while byte_index + LANES <= interior_end {
        let load = |offset: usize| u16x16::from(load_u8x16(source, offset));
        let neighbors = u32x16::from(load(byte_index - 3) + load(byte_index) + load(byte_index + 3));
        let edges = if fractional_weight == 0 {
            u32x16::splat(0)
        } else {
            u32x16::from(load(byte_index - 6) + load(byte_index + 6))
        };
        let weighted = neighbors * whole_weight_vector + edges * fractional_weight_vector + rounding_bias;
        let packed = pack_bytes(weighted >> 24u32);
        destination[byte_index..byte_index + LANES].copy_from_slice(&packed);
        byte_index += LANES;
    }
    for byte_index in byte_index..interior_end {
        scalar_horizontal_sample(
            source,
            destination,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }
    for byte_index in interior_end.max(prefix_end)..row_stride {
        scalar_horizontal_sample(
            source,
            destination,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }
}

fn horizontal_radius_one_pass(
    source: &[u8],
    destination: &mut [u8],
    dimensions: CheckedDims,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    let row_stride = dimensions.row_stride();
    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    let blocks_per_row = interior_end.saturating_sub(prefix_end) / LANES;
    crate::compute::record_pipeline_operation_vector_blocks(
        blocks_per_row.saturating_mul(height) as u64,
    );
    let scalar_bytes = row_stride.saturating_sub(blocks_per_row * LANES);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_bytes.saturating_mul(height) as u64);
    for y in 0..height {
        let start = y * row_stride;
        let end = start + row_stride;
        horizontal_radius_one_row(
            &source[start..end],
            &mut destination[start..end],
            width,
            whole_weight,
            fractional_weight,
        );
    }
}

fn vertical_radius_one_pass(
    source: &[u8],
    destination: &mut [u8],
    dimensions: CheckedDims,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let height = dimensions.height as usize;
    let row_stride = dimensions.row_stride();
    let last_y = height - 1;
    crate::compute::record_pipeline_operation_vector_blocks(
        row_stride.div_ceil(LANES).saturating_mul(height) as u64,
    );
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
                    u32x16::from(u16x16::from(load_padded_u8x16(
                        source,
                        base + x,
                        count,
                    )))
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
}

pub(super) fn execute_radius_one(
    image: &DynamicImage,
    sigma: f32,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgb8(input) = image else {
        return Err(PilError::InternalError(
            "RGB GaussianBlur strategy received a mismatched raster variant".into(),
        ));
    };
    let radius = gaussian_blur_three_pass_radius(sigma.abs());
    if !sigma.is_finite() || !(1.0..2.0).contains(&radius) {
        return Err(PilError::InternalError(
            "RGB GaussianBlur strategy received an unsupported radius".into(),
        ));
    }
    let dimensions = CheckedDims::new(input.width(), input.height(), 3)?;
    if dimensions.width == 0
        || dimensions.height == 0
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "RGB GaussianBlur strategy received invalid native dimensions".into(),
        ));
    }
    let output = if super::super::super::helpers::native_small_uniform_bytes(
        input.as_raw(),
        dimensions.width,
        dimensions.height,
        3,
        super::super::super::helpers::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        let mut output = vec![0; input.as_raw().len()];
        let (blocks, scalar_tail) = crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
            input.as_raw(),
            &mut output,
        )
        .ok_or_else(|| PilError::InternalError("RGB GaussianBlur identity copy shape mismatch".into()))?;
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        output
    } else {
        let whole_weight = (BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
        let fractional_weight = BLUR_SCALE.wrapping_sub(3u32.wrapping_mul(whole_weight)) / 2;
        crate::compute::record_pipeline_operation_path("vector");
        let mut work = input.as_raw().to_vec();
        let mut scratch = vec![0u8; dimensions.total_bytes()];
        for _ in 0..3 {
            horizontal_radius_one_pass(
                &work,
                &mut scratch,
                dimensions,
                whole_weight,
                fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
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
    RgbImage::from_raw(input.width(), input.height(), output)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("RGB GaussianBlur output shape mismatch".into()))
}

fn box_blur_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    dimensions: CheckedDims,
) -> Result<(), PilError> {
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    let row_stride = dimensions.row_stride();
    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    let blocks_per_row = interior_end.saturating_sub(prefix_end) / LANES;
    crate::compute::record_pipeline_operation_vector_blocks(
        blocks_per_row.saturating_mul(height) as u64,
    );
    let scalar_bytes = row_stride.saturating_sub(blocks_per_row * LANES);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_bytes.saturating_mul(height) as u64);

    let whole_weight = BLUR_SCALE / 3;
    if height == 1 {
        horizontal_radius_one_row(source, destination, width, whole_weight, 0);
        return Ok(());
    }

    let ring_height = dimensions.height.min(3);
    let ring_dimensions = CheckedDims::new(dimensions.width, ring_height, 3)?;
    let mut ring = ring_dimensions.alloc_buffer();
    let whole_weight_vector = u32x16::splat(whole_weight);
    let fractional_weight_vector = u32x16::splat(0);
    let horizontal_rounding_bias = u32x16::splat(BLUR_BIAS);
    for source_y in 0..height.min(2) {
        let start = source_y * row_stride;
        horizontal_radius_one_row_with_vectors(
            &source[start..start + row_stride],
            &mut ring[source_y * row_stride..(source_y + 1) * row_stride],
            width,
            whole_weight,
            0,
            whole_weight_vector,
            fractional_weight_vector,
            horizontal_rounding_bias,
        );
    }
    if height > 2 {
        let start = 2 * row_stride;
        horizontal_radius_one_row_with_vectors(
            &source[start..start + row_stride],
            &mut ring[2 * row_stride..3 * row_stride],
            width,
            whole_weight,
            0,
            whole_weight_vector,
            fractional_weight_vector,
            horizontal_rounding_bias,
        );
    }

    crate::compute::record_pipeline_operation_vector_blocks(
        row_stride.div_ceil(LANES).saturating_mul(height) as u64,
    );
    let vertical_weight_vector = u32x16::splat(whole_weight);
    let vertical_rounding_bias = u32x16::splat(BLUR_BIAS);
    let last_y = height - 1;
    for y in 0..height {
        let top_slot = y.saturating_sub(1) % 3;
        let center_slot = y % 3;
        let bottom_slot = y.saturating_add(1).min(last_y) % 3;
        let top_start = top_slot * row_stride;
        let center_start = center_slot * row_stride;
        let bottom_start = bottom_slot * row_stride;
        let output_start = y * row_stride;
        for x in (0..row_stride).step_by(LANES) {
            let count = (row_stride - x).min(LANES);
            let load = |start: usize| {
                if count == LANES {
                    u16x16::from(load_u8x16(&ring, start + x))
                } else {
                    u16x16::from(load_padded_u8x16(&ring, start + x, count))
                }
            };
            let sum = u32x16::from(
                load(top_start) + load(center_start) + load(bottom_start),
            );
            let weighted = sum * vertical_weight_vector + vertical_rounding_bias;
            let packed = pack_bytes(weighted >> 24u32);
            destination[output_start + x..output_start + x + count]
                .copy_from_slice(&packed[..count]);
        }
        let incoming_y = y + 2;
        if incoming_y < height && incoming_y >= 3 {
            let start = incoming_y * row_stride;
            let ring_start = (incoming_y % 3) * row_stride;
            horizontal_radius_one_row_with_vectors(
                &source[start..start + row_stride],
                &mut ring[ring_start..ring_start + row_stride],
                width,
                whole_weight,
                0,
                whole_weight_vector,
                fractional_weight_vector,
                horizontal_rounding_bias,
            );
        }
    }
    Ok(())
}

pub(super) fn execute_box_blur_radius_one(
    image: &DynamicImage,
    request: FilterRequest,
) -> Result<DynamicImage, PilError> {
    let radius_one = match request {
        FilterRequest::BoxBlur { radius } => radius == 1,
        FilterRequest::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => radius_x == 1.0 && radius_y == 1.0 && passes == 1,
        _ => false,
    };
    if !radius_one {
        return Err(PilError::InternalError(
            "RGB BoxBlur strategy received a non-radius-one request".into(),
        ));
    }
    let DynamicImage::ImageRgb8(input) = image else {
        return Err(PilError::InternalError(
            "RGB BoxBlur strategy received a mismatched raster variant".into(),
        ));
    };
    let dimensions = CheckedDims::new(input.width(), input.height(), 3)?;
    if dimensions.width == 0
        || dimensions.height == 0
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "RGB BoxBlur strategy received invalid native dimensions".into(),
        ));
    }
    if super::super::super::helpers::native_small_uniform_bytes(
        input.as_raw(),
        dimensions.width,
        dimensions.height,
        3,
        super::super::super::helpers::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        return super::super::super::helpers::copy_native_byte_blur_input(
            image,
            crate::raster::Mode::ImageRgb8,
            3,
        );
    }
    let mut output = vec![0u8; dimensions.total_bytes()];
    box_blur_radius_one_rows(input.as_raw(), &mut output, dimensions)?;
    crate::compute::record_pipeline_operation_path("vector");
    RgbImage::from_raw(input.width(), input.height(), output)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("RGB BoxBlur output shape mismatch".into()))
}

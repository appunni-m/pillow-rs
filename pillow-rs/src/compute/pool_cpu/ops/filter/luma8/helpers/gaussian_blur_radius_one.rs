//! L-mode three-pass GaussianBlur vector kernels shared by CPU vector arches.

use crate::checked_dims::CheckedDims;
use crate::compute::helpers::filters::gaussian_blur_three_pass_radius;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};
use wide::{i16x8, i32x8, u8x16, u16x16, u32x16};

const LANES: usize = 16;
const BLUR_SCALE: u32 = 1 << 24;
const BLUR_BIAS: u32 = 1 << 23;

#[inline(always)]
fn load_block(source: &[u8], start: usize, edge: u8) -> u8x16 {
    let mut bytes = [edge; LANES];
    if start < source.len() {
        let count = (source.len() - start).min(LANES);
        bytes[..count].copy_from_slice(&source[start..start + count]);
    }
    u8x16::new(bytes)
}

#[inline(always)]
fn pack_bytes(values: u32x16) -> [u8; LANES] {
    let [low, high]: [i32x8; 2] = bytemuck::cast(values);
    let low = i16x8::from_i32x8_saturate(low);
    let high = i16x8::from_i32x8_saturate(high);
    bytemuck::cast(u8x16::narrow_i16x8(low, high))
}

fn weighted_horizontal_radius_one_row(
    source: &[u8],
    destination: &mut [u8],
    whole_weight: u32,
    fractional_weight: u32,
) {
    let width = source.len();
    let right_edge = source[width - 1];
    let left_one_indices = u8x16::new([0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    let left_two_indices = u8x16::new([0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
    let whole_weight = u32x16::splat(whole_weight);
    let fractional_weight = u32x16::splat(fractional_weight);
    let rounding_bias = u32x16::splat(BLUR_BIAS);
    crate::compute::record_pipeline_operation_vector_blocks(width.div_ceil(LANES) as u64);

    for x in (0..width).step_by(LANES) {
        let center_bytes = load_block(source, x, right_edge);
        let left_one_bytes = if x == 0 {
            center_bytes.shuffle(left_one_indices)
        } else {
            load_block(source, x - 1, right_edge)
        };
        let left_two_bytes = if x == 0 {
            center_bytes.shuffle(left_two_indices)
        } else {
            load_block(source, x - 2, right_edge)
        };
        let right_one_bytes = load_block(source, x + 1, right_edge);
        let right_two_bytes = load_block(source, x + 2, right_edge);
        let center = u32x16::from(u16x16::from(center_bytes));
        let neighbors = u32x16::from(u16x16::from(left_one_bytes))
            + center
            + u32x16::from(u16x16::from(right_one_bytes));
        let far_edges = u32x16::from(u16x16::from(left_two_bytes))
            + u32x16::from(u16x16::from(right_two_bytes));
        let weighted = neighbors * whole_weight + far_edges * fractional_weight + rounding_bias;
        let packed = pack_bytes(weighted >> 24u32);
        let count = (width - x).min(LANES);
        destination[x..x + count].copy_from_slice(&packed[..count]);
    }
}

fn horizontal_radius_one_three_passes(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let mut first = vec![0u8; width];
    let mut second = vec![0u8; width];
    for y in 0..height {
        let start = y * width;
        let end = start + width;
        let source_row = &source[start..end];
        weighted_horizontal_radius_one_row(source_row, &mut first, whole_weight, fractional_weight);
        weighted_horizontal_radius_one_row(&first, &mut second, whole_weight, fractional_weight);
        weighted_horizontal_radius_one_row(
            &second,
            &mut destination[start..end],
            whole_weight,
            fractional_weight,
        );
    }
}

fn vertical_radius_one_pass(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    crate::compute::record_pipeline_operation_vector_blocks(
        width.div_ceil(LANES).saturating_mul(height) as u64,
    );
    let last_y = height - 1;
    let whole_weight = u32x16::splat(whole_weight);
    let fractional_weight = u32x16::splat(fractional_weight);
    let rounding_bias = u32x16::splat(BLUR_BIAS);

    for x in (0..width).step_by(LANES) {
        let count = (width - x).min(LANES);
        let load_row = |y: usize| {
            let mut bytes = [0u8; LANES];
            let start = y * width + x;
            bytes[..count].copy_from_slice(&source[start..start + count]);
            u8x16::new(bytes)
        };
        let mut leaving = load_row(0);
        let mut center = load_row(0);
        let mut current = load_row(0);
        let mut entering = load_row(1.min(last_y));
        let mut far_edge = load_row(2.min(last_y));
        let mut accumulator = u32x16::from(u16x16::from(leaving)) * u32x16::splat(3);

        for y in 0..height {
            let leaving_wide = u32x16::from(u16x16::from(leaving));
            accumulator = accumulator - leaving_wide + u32x16::from(u16x16::from(entering));
            let edge_sum = leaving_wide + u32x16::from(u16x16::from(far_edge));
            let weighted =
                accumulator * whole_weight + edge_sum * fractional_weight + rounding_bias;
            let packed = pack_bytes(weighted >> 24u32);
            let output_start = y * width + x;
            destination[output_start..output_start + count].copy_from_slice(&packed[..count]);

            leaving = center;
            center = current;
            current = entering;
            entering = far_edge;
            far_edge = load_row(y.saturating_add(3).min(last_y));
        }
    }
}

fn gaussian_radius_one(raw: &[u8], width: usize, height: usize, radius: f32) -> Vec<u8> {
    let whole_weight = (BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    let fractional_weight = BLUR_SCALE.wrapping_sub(3u32.wrapping_mul(whole_weight)) / 2;
    let mut work = vec![0u8; raw.len()];
    let mut scratch = vec![0u8; raw.len()];
    horizontal_radius_one_three_passes(
        raw,
        &mut work,
        width,
        height,
        whole_weight,
        fractional_weight,
    );
    for _ in 0..3 {
        vertical_radius_one_pass(
            &work,
            &mut scratch,
            width,
            height,
            whole_weight,
            fractional_weight,
        );
        std::mem::swap(&mut work, &mut scratch);
    }
    work
}

pub(in crate::compute::pool_cpu::ops::filter::luma8) fn execute_radius_one(
    image: &DynamicImage,
    sigma: f32,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLuma8(input) = image else {
        return Err(PilError::InternalError(
            "L GaussianBlur strategy received a mismatched raster variant".into(),
        ));
    };
    let radius = gaussian_blur_three_pass_radius(sigma.abs());
    if !sigma.is_finite() || !(1.0..2.0).contains(&radius) {
        return Err(PilError::InternalError(
            "L GaussianBlur strategy received an unsupported radius".into(),
        ));
    }
    let dimensions = CheckedDims::new(input.width(), input.height(), 1)?;
    if dimensions.width == 0
        || dimensions.height == 0
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "L GaussianBlur strategy received invalid native dimensions".into(),
        ));
    }
    let output = if super::super::super::helpers::native_small_uniform_bytes(
        input.as_raw(),
        dimensions.width,
        dimensions.height,
        1,
        super::super::super::helpers::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        let mut output = vec![0; input.as_raw().len()];
        let (blocks, scalar_tail) =
            crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
                input.as_raw(),
                &mut output,
            )
            .ok_or_else(|| {
                PilError::InternalError("L GaussianBlur identity copy shape mismatch".into())
            })?;
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        output
    } else {
        crate::compute::record_pipeline_operation_path("vector");
        gaussian_radius_one(
            input.as_raw(),
            dimensions.width as usize,
            dimensions.height as usize,
            radius,
        )
    };
    GrayImage::from_raw(input.width(), input.height(), output)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("L GaussianBlur output shape mismatch".into()))
}

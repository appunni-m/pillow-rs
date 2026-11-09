//! Portable-wide L-mode radius-one BoxBlur kernel shared by CPU vector architectures.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};
use wide::{i16x8, i32x8, u8x16, u16x16, u32x16};

const BOX_BLUR_SCALE: u32 = 1 << 24;
const BOX_BLUR_BIAS: u32 = 1 << 23;

fn load_radius_one_block(source: &[u8], start: usize, edge: u8) -> u8x16 {
    if start >= source.len() {
        return u8x16::splat(edge);
    }
    let count = (source.len() - start).min(16);
    let mut bytes = [edge; 16];
    bytes[..count].copy_from_slice(&source[start..start + count]);
    u8x16::new(bytes)
}

fn pack_blur_bytes(values: u32x16) -> [u8; 16] {
    // Rounded blur lanes are in 0..=255, so signed saturating packs are exact.
    let [low, high]: [i32x8; 2] = bytemuck::cast(values);
    let low = i16x8::from_i32x8_saturate(low);
    let high = i16x8::from_i32x8_saturate(high);
    bytemuck::cast(u8x16::narrow_i16x8(low, high))
}

/// Apply the exact rounded horizontal L BoxBlur radius-one pass.
fn horizontal_radius_one_row(source: &[u8], destination: &mut [u8]) {
    let width = source.len();
    let left_edge = source[0];
    let right_edge = source[width - 1];
    let reciprocal = u32x16::splat(21_846);
    let one = u32x16::splat(1);

    for x in (0..width).step_by(16) {
        let left = if x == 0 {
            let mut bytes = [left_edge; 16];
            let count = (width - 1).min(15);
            bytes[1..count + 1].copy_from_slice(&source[..count]);
            u8x16::new(bytes)
        } else {
            load_radius_one_block(source, x - 1, right_edge)
        };
        let center = load_radius_one_block(source, x, right_edge);
        let right = load_radius_one_block(source, x + 1, right_edge);
        let left = u32x16::from(u16x16::from(left));
        let center = u32x16::from(u16x16::from(center));
        let right = u32x16::from(u16x16::from(right));
        let average = ((left + center + right + one) * reciprocal) >> 16u32;
        let packed = pack_blur_bytes(average);
        let count = (width - x).min(16);
        destination[x..x + count].copy_from_slice(&packed[..count]);
    }
}

/// Keep three horizontal rows in a ring and preserve Pillow's pass rounding.
fn filter_radius_one(raw: &[u8], output: &mut [u8], width: usize, height: usize) {
    let mut rows = [vec![0u8; width], vec![0u8; width], vec![0u8; width]];
    let mut next_horizontal_row = 0usize;
    let last_row = height - 1;
    let whole_weight = u32x16::splat(BOX_BLUR_SCALE / 3);
    let rounding_bias = u32x16::splat(BOX_BLUR_BIAS);

    for y in 0..height {
        let bottom_row = y.saturating_add(1).min(last_row);
        while next_horizontal_row <= bottom_row {
            let start = next_horizontal_row * width;
            let source_row = &raw[start..start + width];
            horizontal_radius_one_row(source_row, &mut rows[next_horizontal_row % 3]);
            next_horizontal_row += 1;
        }

        let top_row = y.saturating_sub(1);
        let top = &rows[top_row % 3];
        let center = &rows[y % 3];
        let bottom = &rows[bottom_row % 3];
        let output_start = y * width;
        let output_row = &mut output[output_start..output_start + width];

        for x in (0..width).step_by(16) {
            let top = u32x16::from(u16x16::from(load_radius_one_block(top, x, top[width - 1])));
            let center = u32x16::from(u16x16::from(load_radius_one_block(
                center,
                x,
                center[width - 1],
            )));
            let bottom = u32x16::from(u16x16::from(load_radius_one_block(
                bottom,
                x,
                bottom[width - 1],
            )));
            let weighted = (top + center + bottom) * whole_weight + rounding_bias;
            let packed = pack_blur_bytes(weighted >> 24u32);
            let count = (width - x).min(16);
            output_row[x..x + count].copy_from_slice(&packed[..count]);
        }
    }
}

pub(in crate::compute::pool_cpu::ops::filter::luma8) fn execute_radius_one(
    image: &DynamicImage,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLuma8(input) = image else {
        return Err(PilError::InternalError(
            "L BoxBlur strategy received a mismatched raster variant".into(),
        ));
    };
    let width = input.width() as usize;
    let height = input.height() as usize;
    let expected = width.checked_mul(height).ok_or_else(|| {
        PilError::InternalError("L BoxBlur dimensions overflow the native buffer".into())
    })?;
    if width == 0 || height == 0 || input.as_raw().len() != expected {
        return Err(PilError::InternalError(
            "L BoxBlur strategy received invalid native dimensions".into(),
        ));
    }
    if super::super::super::helpers::native_small_uniform_bytes(
        input.as_raw(),
        input.width(),
        input.height(),
        1,
        super::super::super::helpers::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        return super::super::super::helpers::copy_native_byte_blur_input(
            image,
            crate::raster::Mode::ImageLuma8,
            1,
        );
    }

    let mut output = vec![0u8; expected];
    filter_radius_one(input.as_raw(), &mut output, width, height);
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(
        width.div_ceil(16) as u64 * height as u64 * 2,
    );
    GrayImage::from_raw(input.width(), input.height(), output)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::ValueError("BoxBlur output buffer does not match L mode".into()))
}

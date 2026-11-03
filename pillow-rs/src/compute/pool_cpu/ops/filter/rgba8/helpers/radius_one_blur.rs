//! RGBA radius-one BoxBlur and GaussianBlur architecture strategy.

// TODO(pillow-rs#3ptb): freeze RGBA BoxBlur/BoxBlurXY/GaussianBlur radius-one
// output and architecture-selection receipts in CPU parity.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};
use wide::{i16x8, i32x8, u8x16, u16x16, u32x16};

const LANES: usize = 16;
const BLUR_BIAS: u32 = 1 << 23;

#[derive(Clone, Copy)]
struct RadiusOnePlan {
    horizontal: (u32, u32),
    vertical: (u32, u32),
    passes: u32,
}

fn axis(radius: f32) -> (usize, u32, u32) {
    crate::compute::helpers::filters::box_blur_parameters(radius)
}

fn radius_one_plan(request: FilterRequest) -> Option<RadiusOnePlan> {
    let (horizontal, vertical, passes) = match request {
        FilterRequest::GaussianBlur { sigma } if sigma.is_finite() && sigma != 0.0 => {
            let parameters =
                crate::compute::helpers::filters::gaussian_blur_box_parameters(sigma);
            (parameters, parameters, 3)
        }
        FilterRequest::BoxBlur { radius } if radius == 1 => {
            let parameters = axis(radius as f32);
            (parameters, parameters, 1)
        }
        FilterRequest::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } if passes != 0
            && radius_x.is_finite()
            && radius_y.is_finite()
            && radius_x >= 0.0
            && radius_y >= 0.0 =>
        {
            (axis(radius_x), axis(radius_y), passes)
        }
        _ => return None,
    };

    (horizontal.0 == 1 && vertical.0 == 1).then_some(RadiusOnePlan {
        horizontal: (horizontal.1, horizontal.2),
        vertical: (vertical.1, vertical.2),
        passes,
    })
}

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
fn blur_horizontal_pixel(
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
    for channel in 0..4 {
        let sample = |pixel: usize| u32::from(source[pixel * 4 + channel]);
        let neighbors = sample(left) + sample(x) + sample(right);
        let edges = sample(far_left) + sample(far_right);
        let weighted = neighbors
            .wrapping_mul(whole_weight)
            .wrapping_add(edges.wrapping_mul(fractional_weight))
            .wrapping_add(BLUR_BIAS);
        destination[x * 4 + channel] = (weighted >> 24) as u8;
    }
}

fn horizontal_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> Result<(), PilError> {
    let row_stride = width.checked_mul(4).ok_or_else(|| {
        PilError::InternalError("RGBA blur row stride overflows native dimensions".into())
    })?;
    let interior_end = width.saturating_sub(2);
    let vector_blocks_per_row = interior_end.saturating_sub(2) / 4;
    let vector_blocks = vector_blocks_per_row.saturating_mul(height) as u64;
    let scalar_tail = width
        .saturating_sub(vector_blocks_per_row.saturating_mul(4))
        .saturating_mul(height) as u64;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }

    let whole_weight_scalar = whole_weight;
    let fractional_weight_scalar = fractional_weight;
    let whole_weight = u32x16::splat(whole_weight_scalar);
    let fractional_weight = u32x16::splat(fractional_weight_scalar);
    let rounding_bias = u32x16::splat(BLUR_BIAS);
    for y in 0..height {
        let start = y * row_stride;
        let input = &source[start..start + row_stride];
        let output = &mut destination[start..start + row_stride];
        let prefix_end = width.min(2);
        for x in 0..prefix_end {
            blur_horizontal_pixel(
                input,
                output,
                width,
                x,
                whole_weight_scalar,
                fractional_weight_scalar,
            );
        }

        let mut x = 2usize;
        while x + 4 <= interior_end {
            let byte_start = x * 4;
            let load = |pixel: usize| {
                u32x16::from(u16x16::from(load_u8x16(input, pixel * 4)))
            };
            let neighbors = load(x - 1) + load(x) + load(x + 1);
            let edges = if fractional_weight_scalar == 0 {
                u32x16::splat(0)
            } else {
                load(x - 2) + load(x + 2)
            };
            let weighted = neighbors * whole_weight + edges * fractional_weight + rounding_bias;
            let packed = pack_bytes(weighted >> 24u32);
            output[byte_start..byte_start + LANES].copy_from_slice(&packed);
            x += 4;
        }

        for x in x..interior_end {
            blur_horizontal_pixel(
                input,
                output,
                width,
                x,
                whole_weight_scalar,
                fractional_weight_scalar,
            );
        }
        for x in interior_end.max(prefix_end)..width {
            blur_horizontal_pixel(
                input,
                output,
                width,
                x,
                whole_weight_scalar,
                fractional_weight_scalar,
            );
        }
    }
    Ok(())
}

fn vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> Result<(), PilError> {
    let row_stride = width.checked_mul(4).ok_or_else(|| {
        PilError::InternalError("RGBA blur row stride overflows native dimensions".into())
    })?;
    let last_y = height - 1;
    let vector_blocks = row_stride.div_ceil(LANES).saturating_mul(height) as u64;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    let fractional_weight_is_zero = fractional_weight == 0;
    let whole_weight = u32x16::splat(whole_weight);
    let fractional_weight = u32x16::splat(fractional_weight);
    let rounding_bias = u32x16::splat(BLUR_BIAS);

    for y in 0..height {
        let top = y.saturating_sub(1) * row_stride;
        let center = y * row_stride;
        let bottom = y.saturating_add(1).min(last_y) * row_stride;
        let far_top = y.saturating_sub(2) * row_stride;
        let far_bottom = y.saturating_add(2).min(last_y) * row_stride;
        let output = y * row_stride;
        for x in (0..row_stride).step_by(LANES) {
            let count = (row_stride - x).min(LANES);
            let load = |start: usize| {
                if count == LANES {
                    u32x16::from(u16x16::from(load_u8x16(source, start + x)))
                } else {
                    u32x16::from(u16x16::from(load_padded_u8x16(
                        source,
                        start + x,
                        count,
                    )))
                }
            };
            let neighbors = load(top) + load(center) + load(bottom);
            let edges = if fractional_weight_is_zero {
                u32x16::splat(0)
            } else {
                load(far_top) + load(far_bottom)
            };
            let weighted = neighbors * whole_weight + edges * fractional_weight + rounding_bias;
            let packed = pack_bytes(weighted >> 24u32);
            destination[output + x..output + x + count].copy_from_slice(&packed[..count]);
        }
    }
    Ok(())
}

pub(in crate::compute::pool_cpu::ops::filter::rgba8) fn try_execute(
    image: &DynamicImage,
    request: FilterRequest,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(plan) = radius_one_plan(request) else {
        return Ok(None);
    };
    if image.mode() != Mode::ImageRgba8 {
        return Err(PilError::InternalError(
            "RGBA radius-one blur received a mismatched raster mode".into(),
        ));
    }
    let dimensions = CheckedDims::new(image.width(), image.height(), 4)?;
    if image.as_bytes().len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "RGBA radius-one blur input does not match its dimensions".into(),
        ));
    }
    if super::super::super::helpers::native_small_uniform_bytes(
        image.as_bytes(),
        image.width(),
        image.height(),
        4,
        super::super::super::helpers::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        return super::super::super::helpers::copy_native_byte_blur_input(
            image,
            Mode::ImageRgba8,
            4,
        )
        .map(Some);
    }

    crate::compute::record_pipeline_operation_path("vector");
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    let mut work = image.as_bytes().to_vec();
    let mut scratch = dimensions.alloc_buffer();
    for _ in 0..plan.passes {
        horizontal_radius_one_rows(
            &work,
            &mut scratch,
            width,
            height,
            plan.horizontal.0,
            plan.horizontal.1,
        )?;
        std::mem::swap(&mut work, &mut scratch);
    }
    for _ in 0..plan.passes {
        vertical_radius_one_rows(
            &work,
            &mut scratch,
            width,
            height,
            plan.vertical.0,
            plan.vertical.1,
        )?;
        std::mem::swap(&mut work, &mut scratch);
    }

    RgbaImage::from_raw(dimensions.width, dimensions.height, work)
        .map(DynamicImage::ImageRgba8)
        .map(Some)
        .ok_or_else(|| PilError::InternalError("RGBA radius-one blur output shape mismatch".into()))
}

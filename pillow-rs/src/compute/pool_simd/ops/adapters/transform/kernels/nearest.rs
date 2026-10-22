//! Transform nearest sampling kernels.

use super::*;

fn simd_affine_fixed_bytes<const CHANNELS: usize>(
    source: &[u8],
    source_width: usize,
    source_height: usize,
    width: usize,
    height: usize,
    plan: [i64; 6],
    fill: [u8; 4],
) -> Vec<u8> {
    let [a, b, c, d, e, f] = plan;
    let offsets_x = i64x8::new(std::array::from_fn(|lane| lane as i64 * a));
    let offsets_y = i64x8::new(std::array::from_fn(|lane| lane as i64 * d));
    let step_x = i64x8::splat(a * 8);
    let step_y = i64x8::splat(d * 8);
    let mut output = vec![0; width * height * CHANNELS];
    for (y, row) in output.chunks_exact_mut(width * CHANNELS).enumerate() {
        let mut source_x = i64x8::splat(c + y as i64 * b) + offsets_x;
        let mut source_y = i64x8::splat(f + y as i64 * e) + offsets_y;
        for block in row.chunks_mut(8 * CHANNELS) {
            let x = (source_x >> 16_i32).to_array();
            let y = (source_y >> 16_i32).to_array();
            for (lane, pixel) in block.chunks_exact_mut(CHANNELS).enumerate() {
                if x[lane] >= 0
                    && x[lane] < source_width as i64
                    && y[lane] >= 0
                    && y[lane] < source_height as i64
                {
                    let start = (y[lane] as usize * source_width + x[lane] as usize) * CHANNELS;
                    pixel.copy_from_slice(&source[start..start + CHANNELS]);
                } else {
                    pixel.copy_from_slice(&fill[..CHANNELS]);
                }
            }
            source_x += step_x;
            source_y += step_y;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks((height * width.div_ceil(8)) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    output
}

pub(in super::super) fn simd_affine_nearest_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_affine_nearest_transform_channels(img, mode) else {
        return Ok(None);
    };
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Transform"))?;
    let destination_width = usize::try_from(width).map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height).map_err(|_| simd_unsupported("Transform"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD transform source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or(if channels == 2 || channels == 4 {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, u8::MAX)
    });
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let [a, b, c, d, e, f] = data else {
        return Ok(None);
    };
    if let Some(plan) = crate::compute::pool_cpu::ops::effects::affine_nearest_fixed(
        [*a, *b, *c, *d, *e, *f],
        width,
        height,
    ) {
        let output = match channels {
            1 => simd_affine_fixed_bytes::<1>(
                source,
                source_width,
                source_height,
                destination_width,
                destination_height,
                plan,
                fill,
            ),
            2 => simd_affine_fixed_bytes::<2>(
                source,
                source_width,
                source_height,
                destination_width,
                destination_height,
                plan,
                fill,
            ),
            3 => simd_affine_fixed_bytes::<3>(
                source,
                source_width,
                source_height,
                destination_width,
                destination_height,
                plan,
                fill,
            ),
            4 => simd_affine_fixed_bytes::<4>(
                source,
                source_width,
                source_height,
                destination_width,
                destination_height,
                plan,
                fill,
            ),
            _ => return Ok(None),
        };
        return crate::image_utils::raw_bytes_to_image(width, height, output, channels)
            .map(|result| Some(preserve_mode(img, result)));
    }
    let pixels_per_vector = SIMD_RESIZE_NEAREST_BYTES / channels;
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    for destination_y in 0..destination_height {
        let row_start = destination_y
            .checked_mul(destination_width)
            .and_then(|pixels| pixels.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Transform"))?;
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = pixels_per_vector.min(destination_width - destination_x);
            let mut block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
            for pixel in 0..count {
                let x = destination_x + pixel;
                let y = destination_y;
                let source_x = *a * (x as f64 + 0.5) + *b * (y as f64 + 0.5) + *c;
                let source_y = *d * (x as f64 + 0.5) + *e * (y as f64 + 0.5) + *f;
                // Match Geometry.c's nearest affine contract: negative
                // coordinates are outside, while non-negative coordinates
                // truncate toward zero before the bounds check.
                let input_x = if source_x < 0.0 { -1 } else { source_x as i64 };
                let input_y = if source_y < 0.0 { -1 } else { source_y as i64 };
                let block_start = pixel
                    .checked_mul(channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?;
                if input_x >= 0
                    && input_x < source_width as i64
                    && input_y >= 0
                    && input_y < source_height as i64
                {
                    let source_start = (input_y as usize)
                        .checked_mul(source_width)
                        .and_then(|pixels| pixels.checked_add(input_x as usize))
                        .and_then(|pixel| pixel.checked_mul(channels))
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    let source_end = source_start
                        .checked_add(channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    block
                        .get_mut(block_start..block_start + channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?
                        .copy_from_slice(
                            source
                                .get(source_start..source_end)
                                .ok_or_else(|| simd_unsupported("Transform"))?,
                        );
                } else {
                    for channel in 0..channels {
                        block[block_start + channel] = fill[channel];
                    }
                }
            }
            let output_start = row_start
                .checked_add(
                    destination_x
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?,
                )
                .ok_or_else(|| simd_unsupported("Transform"))?;
            let output_bytes = count
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Transform"))?;
            output
                .get_mut(output_start..output_start + output_bytes)
                .ok_or_else(|| simd_unsupported("Transform"))?
                .copy_from_slice(&u8x16::new(block).to_array()[..output_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

/// Vectorized nearest sampling for perspective and quadrilateral transforms.
/// The homography/quad coordinate equations run in eight `f64` lanes; source
/// address validation and the irregular gathers remain scalar control work.
/// No CPU transform is retried after this adapter has been admitted.
pub(in super::super) fn simd_projective_nearest_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
    quad: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_affine_nearest_transform_channels(img, mode) else {
        return Ok(None);
    };
    let destination_width = usize::try_from(width).map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height).map_err(|_| simd_unsupported("Transform"))?;
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Transform"))?;
    if destination_width == 0
        || destination_height == 0
        || source_width == 0
        || source_height == 0
        || data.len() != 8
    {
        return Ok(None);
    }
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD projective transform source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 255));
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let [a, b, c, d, e, f, g, h] = data else {
        return Ok(None);
    };
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let destination_width_f = destination_width as f64;
    let destination_height_f = destination_height as f64;
    // Pillow 12.2.0's Geometry.c evaluates projective and quad maps at
    // destination pixel centers and its COORD macro truncates nonnegative
    // coordinates toward zero, returning -1 for any negative coordinate.
    let nearest_coordinate = |value: f64| {
        if !value.is_finite() {
            -1
        } else if value < 0.0 {
            -1
        } else {
            value as i64
        }
    };

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = (destination_width - destination_x).min(SIMD_F64_LANES);
            let x_values = std::array::from_fn(|lane| {
                if lane < count {
                    (destination_x + lane) as f64
                } else {
                    0.0
                }
            });
            let y_values = [destination_y as f64; SIMD_F64_LANES];
            let x = f64x8::new(x_values);
            let y = f64x8::new(y_values);
            let (source_x, source_y) = if quad {
                let x = x + f64x8::splat(0.5);
                let y = y + f64x8::splat(0.5);
                let x0 = *a;
                let y0 = *b;
                let inv_width = 1.0 / destination_width_f;
                let inv_height = 1.0 / destination_height_f;
                let x1 = (*g - x0) * inv_width;
                let x2 = (*c - x0) * inv_height;
                let x3 = (*e - *c - *g + x0) * inv_width * inv_height;
                let y1 = (*h - y0) * inv_width;
                let y2 = (*d - y0) * inv_height;
                let y3 = (*f - *d - *h + y0) * inv_width * inv_height;
                let sx = f64x8::splat(x0)
                    + f64x8::splat(x1) * x
                    + f64x8::splat(x2) * y
                    + f64x8::splat(x3) * x * y;
                let sy = f64x8::splat(y0)
                    + f64x8::splat(y1) * x
                    + f64x8::splat(y2) * y
                    + f64x8::splat(y3) * x * y;
                (sx.to_array(), sy.to_array())
            } else {
                let x = x + f64x8::splat(0.5);
                let y = y + f64x8::splat(0.5);
                let denominator = f64x8::splat(*g) * x + f64x8::splat(*h) * y + f64x8::splat(1.0);
                let sx =
                    (f64x8::splat(*a) * x + f64x8::splat(*b) * y + f64x8::splat(*c)) / denominator;
                let sy =
                    (f64x8::splat(*d) * x + f64x8::splat(*e) * y + f64x8::splat(*f)) / denominator;
                (sx.to_array(), sy.to_array())
            };

            for lane in 0..count {
                let output_start = (destination_y * destination_width + destination_x + lane)
                    .checked_mul(channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?;
                let sx = source_x[lane];
                let sy = source_y[lane];
                let input_x = nearest_coordinate(sx);
                let input_y = nearest_coordinate(sy);
                if input_x >= 0
                    && input_x < source_width as i64
                    && input_y >= 0
                    && input_y < source_height as i64
                {
                    let source_start = (input_y as usize * source_width + input_x as usize)
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    output[output_start..output_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                } else {
                    output[output_start..output_start + channels]
                        .copy_from_slice(&fill[..channels]);
                }
            }
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

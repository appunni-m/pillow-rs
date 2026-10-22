//! Transform bilinear sampling kernels.

use super::{
    DynamicImage, PilError, SIMD_F64_LANES, SimdRotateSample, bilinear_rotate_scalar,
    bilinear_rotate_vector, f64x8, gather_rotate_neighborhood_vectors,
    native_affine_bilinear_layout, native_affine_nearest_transform_channels, preserve_mode,
    rotate_fill_sample, rotate_unpremultiply_vector, simd_rotate_sample_coordinates,
    simd_unsupported,
};

/// Vectorized bilinear sampling for perspective and quadrilateral transforms.
///
/// The inverse-map equations and four-neighbour interpolation run in
/// `f64x8` blocks. Source addresses are irregular gathers, so their bounds
/// checks and byte loads remain scalar control work; they are packed into the
/// vector lanes before interpolation. Invalid coordinates use Pillow's fill
/// sample and never trigger a CPU retry.
pub(in super::super) fn simd_projective_bilinear_transform_bytes(
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
            "SIMD projective bilinear source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 255));
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let [a, b, c, d, e, f, g, h] = data else {
        return Ok(None);
    };
    let inverse_width = 1.0 / destination_width as f64;
    let inverse_height = 1.0 / destination_height as f64;
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let one = f64x8::splat(1.0);

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = (destination_width - destination_x).min(SIMD_F64_LANES);
            let x = f64x8::new(std::array::from_fn(|lane| {
                if lane < count {
                    (destination_x + lane) as f64
                } else {
                    0.0
                }
            }));
            let y = f64x8::splat(destination_y as f64);
            let (source_x, source_y) = if quad {
                let source_x = f64x8::splat(*a)
                    + f64x8::splat(*g - *a) * x * f64x8::splat(inverse_width)
                    + f64x8::splat(*c - *a) * y * f64x8::splat(inverse_height)
                    + f64x8::splat(*e - *c - *g + *a)
                        * x
                        * y
                        * f64x8::splat(inverse_width * inverse_height);
                let source_y = f64x8::splat(*b)
                    + f64x8::splat(*h - *b) * x * f64x8::splat(inverse_width)
                    + f64x8::splat(*d - *b) * y * f64x8::splat(inverse_height)
                    + f64x8::splat(*f - *d - *h + *b)
                        * x
                        * y
                        * f64x8::splat(inverse_width * inverse_height);
                (source_x.to_array(), source_y.to_array())
            } else {
                let denominator = f64x8::splat(*g) * x + f64x8::splat(*h) * y + f64x8::splat(1.0);
                let source_x =
                    (f64x8::splat(*a) * x + f64x8::splat(*b) * y + f64x8::splat(*c)) / denominator;
                let source_y =
                    (f64x8::splat(*d) * x + f64x8::splat(*e) * y + f64x8::splat(*f)) / denominator;
                (source_x.to_array(), source_y.to_array())
            };

            let mut fx_values = [0.0; SIMD_F64_LANES];
            let mut fy_values = [0.0; SIMD_F64_LANES];
            let mut neighbors = [[[0.0; SIMD_F64_LANES]; 4]; 4];
            for channel in 0..channels {
                for neighbor in 0..4 {
                    for lane in 0..count {
                        neighbors[channel][neighbor][lane] = fill[channel] as f64;
                    }
                }
            }
            for lane in 0..count {
                let sx = source_x[lane];
                let sy = source_y[lane];
                if !sx.is_finite()
                    || !sy.is_finite()
                    || sx < 0.0
                    || sx >= source_width as f64
                    || sy < 0.0
                    || sy >= source_height as f64
                {
                    continue;
                }
                let x0 = sx.floor() as usize;
                let y0 = sy.floor() as usize;
                let x1 = (x0 + 1).min(source_width - 1);
                let y1 = (y0 + 1).min(source_height - 1);
                fx_values[lane] = sx - x0 as f64;
                fy_values[lane] = sy - y0 as f64;
                let coordinates = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)];
                for (neighbor, &(source_x, source_y)) in coordinates.iter().enumerate() {
                    let source_start = (source_y * source_width + source_x) * channels;
                    for channel in 0..channels {
                        neighbors[channel][neighbor][lane] = source[source_start + channel] as f64;
                    }
                }
            }

            let fx = f64x8::new(fx_values);
            let fy = f64x8::new(fy_values);
            let mut block = [0u8; SIMD_F64_LANES * 4];
            for channel in 0..channels {
                let p00 = f64x8::new(neighbors[channel][0]);
                let p10 = f64x8::new(neighbors[channel][1]);
                let p01 = f64x8::new(neighbors[channel][2]);
                let p11 = f64x8::new(neighbors[channel][3]);
                let interpolated =
                    (one - fx) * ((one - fy) * p00 + fy * p01) + fx * ((one - fy) * p10 + fy * p11);
                let values = interpolated
                    .to_array()
                    .map(|value| value.clamp(0.0, 255.0).round() as u8);
                for lane in 0..count {
                    block[lane * channels + channel] = values[lane];
                }
            }
            let output_start = (destination_y * destination_width + destination_x) * channels;
            let output_bytes = count * channels;
            output[output_start..output_start + output_bytes]
                .copy_from_slice(&block[..output_bytes]);
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

/// Vectorized affine bilinear sampling for native byte layouts.
///
/// Coordinate construction, bounds checks, and source-neighbour indices are
/// scalar control work. The four-neighbour interpolation itself runs in
/// `f64x8` blocks, with the same premultiplied-alpha byte boundaries used by
/// Pillow for `LA` and `RGBA`. A short final row segment is handled by the
/// scalar tail; it never retries the CPU transform implementation.
pub(in super::super) fn simd_affine_bilinear_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some((channels, alpha_channel)) = native_affine_bilinear_layout(img, mode) else {
        return Ok(None);
    };
    let destination_width = usize::try_from(width).map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height).map_err(|_| simd_unsupported("Transform"))?;
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Transform"))?;
    if destination_height == 0 || source_width == 0 || source_height == 0 {
        return Ok(None);
    }
    let [a, b, c, d, e, f] = data else {
        return Ok(None);
    };
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
            "SIMD affine bilinear source buffer shape mismatch".into(),
        ));
    }

    // `Image.transform` carries LA/PA alpha in fill.1, while the shared
    // rotate gather helper reads a two-channel alpha fill from fill.3.
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let fill = if channels == 2 {
        (fill.0, 0, 0, fill.1)
    } else {
        fill
    };
    let affine = [*a, *b, *c, *d, *e, *f];
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    // A narrow output row still uses one padded vector block. Inactive lanes
    // are computed into the temporary block and omitted from the store; they
    // are never exposed as a scalar-only admission failure.
    let vector_width = if destination_width < SIMD_F64_LANES {
        destination_width
    } else {
        destination_width / SIMD_F64_LANES * SIMD_F64_LANES
    };

    for y in 0..destination_height {
        let output_row = y * destination_width * channels;
        let mut x = 0usize;
        while x < vector_width {
            let count = (destination_width - x).min(SIMD_F64_LANES);
            let samples = std::array::from_fn(|lane| {
                if lane < count {
                    simd_rotate_sample_coordinates(affine, source_width, source_height, x + lane, y)
                } else {
                    SimdRotateSample::default()
                }
            });
            let fx = f64x8::new(samples.map(|sample| sample.fx));
            let fy = f64x8::new(samples.map(|sample| sample.fy));
            let mut block = [0u8; SIMD_F64_LANES * 4];
            let mut alpha = [0u8; SIMD_F64_LANES];

            if let Some(alpha_channel) = alpha_channel {
                let neighbors = gather_rotate_neighborhood_vectors(
                    source,
                    source_width,
                    channels,
                    alpha_channel,
                    fill,
                    &samples,
                    None,
                );
                alpha = bilinear_rotate_vector(
                    neighbors[0],
                    neighbors[1],
                    neighbors[2],
                    neighbors[3],
                    fx,
                    fy,
                )
                .to_array()
                .map(|value| value as u8);
                for (lane, &value) in alpha.iter().enumerate() {
                    block[lane * channels + alpha_channel] = value;
                }
            }

            for channel in 0..channels {
                if Some(channel) == alpha_channel {
                    continue;
                }
                let neighbors = gather_rotate_neighborhood_vectors(
                    source,
                    source_width,
                    channels,
                    channel,
                    fill,
                    &samples,
                    alpha_channel,
                );
                let interpolated = bilinear_rotate_vector(
                    neighbors[0],
                    neighbors[1],
                    neighbors[2],
                    neighbors[3],
                    fx,
                    fy,
                );
                let values = if alpha_channel.is_some() {
                    rotate_unpremultiply_vector(interpolated, alpha)
                } else {
                    interpolated.to_array().map(|value| value as u8)
                };
                for (lane, &value) in values.iter().enumerate() {
                    block[lane * channels + channel] = value;
                }
            }

            let output_start = output_row + x * channels;
            let block_bytes = count * channels;
            output[output_start..output_start + block_bytes].copy_from_slice(&block[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += count;
        }

        while x < destination_width {
            let sample = simd_rotate_sample_coordinates(affine, source_width, source_height, x, y);
            let output_start = output_row + x * channels;
            let mut alpha = 0u8;
            if let Some(alpha_channel) = alpha_channel {
                let alpha_value = if sample.valid {
                    let read = |source_x: usize, source_y: usize| {
                        source[(source_y * source_width + source_x) * channels + alpha_channel]
                            as f64
                    };
                    bilinear_rotate_scalar(
                        read(sample.x0, sample.y0),
                        read(sample.x1, sample.y0),
                        read(sample.x0, sample.y1),
                        read(sample.x1, sample.y1),
                        sample.fx,
                        sample.fy,
                    )
                } else {
                    rotate_fill_sample(fill, channels, alpha_channel) as f64
                };
                alpha = alpha_value as u8;
                output[output_start + alpha_channel] = alpha;
            }

            for channel in 0..channels {
                if Some(channel) == alpha_channel {
                    continue;
                }
                let value = if sample.valid {
                    let read = |source_x: usize, source_y: usize| {
                        source[(source_y * source_width + source_x) * channels + channel] as f64
                    };
                    let mut p00 = read(sample.x0, sample.y0);
                    let mut p10 = read(sample.x1, sample.y0);
                    let mut p01 = read(sample.x0, sample.y1);
                    let mut p11 = read(sample.x1, sample.y1);
                    if let Some(alpha_channel) = alpha_channel {
                        let read_alpha = |source_x: usize, source_y: usize| {
                            source[(source_y * source_width + source_x) * channels + alpha_channel]
                                as f64
                        };
                        let premultiply =
                            |value: f64, alpha: f64| (value * alpha / 255.0 + 0.5) as u8 as f64;
                        p00 = premultiply(p00, read_alpha(sample.x0, sample.y0));
                        p10 = premultiply(p10, read_alpha(sample.x1, sample.y0));
                        p01 = premultiply(p01, read_alpha(sample.x0, sample.y1));
                        p11 = premultiply(p11, read_alpha(sample.x1, sample.y1));
                    }
                    bilinear_rotate_scalar(p00, p10, p01, p11, sample.fx, sample.fy)
                } else {
                    rotate_fill_sample(fill, channels, channel) as f64
                };
                let value = value as u8;
                output[output_start + channel] = if alpha_channel.is_some() && alpha != 0 {
                    (f64::from(value) * 255.0 / f64::from(alpha)) as u8
                } else {
                    value
                };
            }
            x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

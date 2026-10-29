//! Affine bilinear sampling for native byte layouts.

use super::super::{
    DynamicImage, PilError, SIMD_F64_LANES, SimdRotateSample, bilinear_rotate_scalar,
    bilinear_rotate_vector, f64x8, gather_rotate_neighborhood_vectors,
    native_affine_bilinear_layout, preserve_mode, rotate_fill_sample, rotate_unpremultiply_vector,
    simd_rotate_sample_coordinates, simd_unsupported,
};

struct AffineByteBilinearSampler<'a> {
    source: &'a [u8],
    source_width: usize,
    channels: usize,
    alpha_channel: Option<usize>,
    fill: (u8, u8, u8, u8),
}

impl AffineByteBilinearSampler<'_> {
    #[inline(always)]
    fn vector_block(
        &self,
        samples: &[SimdRotateSample; SIMD_F64_LANES],
        fx: f64x8,
        fy: f64x8,
    ) -> [u8; SIMD_F64_LANES * 4] {
        let mut block = [0u8; SIMD_F64_LANES * 4];
        let mut alpha = [0u8; SIMD_F64_LANES];
        if let Some(alpha_channel) = self.alpha_channel {
            let neighbors = gather_rotate_neighborhood_vectors(
                self.source,
                self.source_width,
                self.channels,
                alpha_channel,
                self.fill,
                samples,
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
                block[lane * self.channels + alpha_channel] = value;
            }
        }

        for channel in 0..self.channels {
            if Some(channel) == self.alpha_channel {
                continue;
            }
            let neighbors = gather_rotate_neighborhood_vectors(
                self.source,
                self.source_width,
                self.channels,
                channel,
                self.fill,
                samples,
                self.alpha_channel,
            );
            let interpolated = bilinear_rotate_vector(
                neighbors[0],
                neighbors[1],
                neighbors[2],
                neighbors[3],
                fx,
                fy,
            );
            let values = if self.alpha_channel.is_some() {
                rotate_unpremultiply_vector(interpolated, alpha)
            } else {
                interpolated.to_array().map(|value| value as u8)
            };
            for (lane, &value) in values.iter().enumerate() {
                block[lane * self.channels + channel] = value;
            }
        }
        block
    }

    #[inline(always)]
    fn scalar_pixel(&self, output: &mut [u8], output_start: usize, sample: SimdRotateSample) {
        let mut alpha = 0u8;
        if let Some(alpha_channel) = self.alpha_channel {
            let alpha_value = if sample.valid {
                let read = |source_x: usize, source_y: usize| {
                    self.source
                        [(source_y * self.source_width + source_x) * self.channels + alpha_channel]
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
                rotate_fill_sample(self.fill, self.channels, alpha_channel) as f64
            };
            alpha = alpha_value as u8;
            output[output_start + alpha_channel] = alpha;
        }

        for channel in 0..self.channels {
            if Some(channel) == self.alpha_channel {
                continue;
            }
            let value = if sample.valid {
                let read = |source_x: usize, source_y: usize| {
                    self.source[(source_y * self.source_width + source_x) * self.channels + channel]
                        as f64
                };
                let mut p00 = read(sample.x0, sample.y0);
                let mut p10 = read(sample.x1, sample.y0);
                let mut p01 = read(sample.x0, sample.y1);
                let mut p11 = read(sample.x1, sample.y1);
                if let Some(alpha_channel) = self.alpha_channel {
                    let read_alpha = |source_x: usize, source_y: usize| {
                        self.source[(source_y * self.source_width + source_x) * self.channels
                            + alpha_channel] as f64
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
                rotate_fill_sample(self.fill, self.channels, channel) as f64
            };
            let value = value as u8;
            output[output_start + channel] = if self.alpha_channel.is_some() && alpha != 0 {
                (f64::from(value) * 255.0 / f64::from(alpha)) as u8
            } else {
                value
            };
        }
    }
}

/// Vectorized affine bilinear sampling for native byte layouts.
///
/// Coordinate construction, bounds checks, and source-neighbour indices are
/// scalar control work. The four-neighbour interpolation itself runs in
/// `f64x8` blocks, with the same premultiplied-alpha byte boundaries used by
/// Pillow for `LA` and `RGBA`. A short final row segment is handled by the
/// scalar tail; it never retries the CPU transform implementation.
pub(in super::super::super) fn simd_affine_bilinear_transform_bytes(
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
    let sampler = AffineByteBilinearSampler {
        source,
        source_width,
        channels,
        alpha_channel,
        fill,
    };
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
            let block = sampler.vector_block(&samples, fx, fy);

            let output_start = output_row + x * channels;
            let block_bytes = count * channels;
            output[output_start..output_start + block_bytes].copy_from_slice(&block[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += count;
        }

        while x < destination_width {
            let sample = simd_rotate_sample_coordinates(affine, source_width, source_height, x, y);
            let output_start = output_row + x * channels;
            sampler.scalar_pixel(&mut output, output_start, sample);
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

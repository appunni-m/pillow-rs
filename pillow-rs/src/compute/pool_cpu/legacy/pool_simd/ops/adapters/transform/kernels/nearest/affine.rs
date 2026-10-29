//! Affine nearest sampling for native byte layouts.

use super::super::{
    DynamicImage, PilError, SIMD_RESIZE_NEAREST_BYTES, i64x8,
    native_affine_nearest_transform_channels, preserve_mode, simd_unsupported, u8x16,
};

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

struct AffineNearestByteSampler<'a> {
    source: &'a [u8],
    source_width: usize,
    source_height: usize,
    channels: usize,
    affine: [f64; 6],
    fill: [u8; 4],
}

impl AffineNearestByteSampler<'_> {
    #[inline(always)]
    fn copy_block(
        &self,
        output: &mut [u8],
        row_start: usize,
        destination_y: usize,
        destination_x: usize,
        count: usize,
    ) -> Result<(), PilError> {
        let [a, b, c, d, e, f] = self.affine;
        let mut block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
        for pixel in 0..count {
            let x = destination_x + pixel;
            let source_x = a * (x as f64 + 0.5) + b * (destination_y as f64 + 0.5) + c;
            let source_y = d * (x as f64 + 0.5) + e * (destination_y as f64 + 0.5) + f;
            // Match Geometry.c's nearest affine contract: negative
            // coordinates are outside, while non-negative coordinates
            // truncate toward zero before the bounds check.
            let input_x = if source_x < 0.0 { -1 } else { source_x as i64 };
            let input_y = if source_y < 0.0 { -1 } else { source_y as i64 };
            let block_start = pixel
                .checked_mul(self.channels)
                .ok_or_else(|| simd_unsupported("Transform"))?;
            if input_x >= 0
                && input_x < self.source_width as i64
                && input_y >= 0
                && input_y < self.source_height as i64
            {
                let source_start = (input_y as usize)
                    .checked_mul(self.source_width)
                    .and_then(|pixels| pixels.checked_add(input_x as usize))
                    .and_then(|pixel| pixel.checked_mul(self.channels))
                    .ok_or_else(|| simd_unsupported("Transform"))?;
                let source_end = source_start
                    .checked_add(self.channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?;
                block
                    .get_mut(block_start..block_start + self.channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?
                    .copy_from_slice(
                        self.source
                            .get(source_start..source_end)
                            .ok_or_else(|| simd_unsupported("Transform"))?,
                    );
            } else {
                block
                    .get_mut(block_start..block_start + self.channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?
                    .copy_from_slice(&self.fill[..self.channels]);
            }
        }
        let output_start = row_start
            .checked_add(
                destination_x
                    .checked_mul(self.channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?,
            )
            .ok_or_else(|| simd_unsupported("Transform"))?;
        let output_bytes = count
            .checked_mul(self.channels)
            .ok_or_else(|| simd_unsupported("Transform"))?;
        output
            .get_mut(output_start..output_start + output_bytes)
            .ok_or_else(|| simd_unsupported("Transform"))?
            .copy_from_slice(&u8x16::new(block).to_array()[..output_bytes]);
        Ok(())
    }
}

struct AffineNearestDimensions {
    source_width: usize,
    source_height: usize,
    destination_width: usize,
    destination_height: usize,
    source_len: usize,
    destination_len: usize,
}

fn affine_nearest_dimensions(
    img: &DynamicImage,
    width: u32,
    height: u32,
    channels: usize,
) -> Result<AffineNearestDimensions, PilError> {
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
    Ok(AffineNearestDimensions {
        source_width,
        source_height,
        destination_width,
        destination_height,
        source_len,
        destination_len,
    })
}

fn affine_nearest_source_bytes(img: &DynamicImage, source_len: usize) -> Result<&[u8], PilError> {
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD transform source buffer shape mismatch".into(),
        ));
    }
    Ok(source)
}

pub(in super::super::super) fn simd_affine_nearest_transform_bytes(
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
    let AffineNearestDimensions {
        source_width,
        source_height,
        destination_width,
        destination_height,
        source_len,
        destination_len,
    } = affine_nearest_dimensions(img, width, height, channels)?;
    let source = affine_nearest_source_bytes(img, source_len)?;
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
    let sampler = AffineNearestByteSampler {
        source,
        source_width,
        source_height,
        channels,
        affine: [*a, *b, *c, *d, *e, *f],
        fill,
    };
    for destination_y in 0..destination_height {
        let row_start = destination_y
            .checked_mul(destination_width)
            .and_then(|pixels| pixels.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Transform"))?;
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = pixels_per_vector.min(destination_width - destination_x);
            sampler.copy_block(&mut output, row_start, destination_y, destination_x, count)?;
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

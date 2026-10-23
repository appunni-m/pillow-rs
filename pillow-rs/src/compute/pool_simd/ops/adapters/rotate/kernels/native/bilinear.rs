//! Native byte bilinear Rotate kernel.

use super::super::super::super::SIMD_F64_LANES;
use super::super::super::{
    SimdRotateSample, bilinear_rotate_scalar, bilinear_rotate_vector,
    gather_rotate_neighborhood_vectors, native_rotate_layout, rotate_fill_sample,
    rotate_unpremultiply_vector, simd_rotate_geometry, simd_rotate_sample_coordinates,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::DynamicImage;
use wide::f64x8;

struct NativeByteBilinearRotateSampler<'a> {
    source: &'a [u8],
    width: usize,
    channels: usize,
    alpha_channel: Option<usize>,
    fill: (u8, u8, u8, u8),
}

impl NativeByteBilinearRotateSampler<'_> {
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
                self.width,
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
                self.width,
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
                    self.source[(source_y * self.width + source_x) * self.channels + alpha_channel]
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
                    self.source[(source_y * self.width + source_x) * self.channels + channel] as f64
                };
                let mut p00 = read(sample.x0, sample.y0);
                let mut p10 = read(sample.x1, sample.y0);
                let mut p01 = read(sample.x0, sample.y1);
                let mut p11 = read(sample.x1, sample.y1);
                if let Some(alpha_channel) = self.alpha_channel {
                    let read_alpha = |source_x: usize, source_y: usize| {
                        self.source
                            [(source_y * self.width + source_x) * self.channels + alpha_channel]
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
                rotate_fill_sample(self.fill, self.channels, channel) as f64
            };
            let value = value as u8;
            output[output_start + channel] = if self.alpha_channel.is_some() && alpha != 0 {
                (value as f64 * 255.0 / alpha as f64) as u8
            } else {
                value
            };
        }
    }
}

/// Vectorized ordinary-byte bilinear rotation.
///
/// Affine coordinate construction and source gathers are scalar control;
/// neighbor interpolation runs eight output pixels at a time in `f64x8`.
/// LA/RGBA retain Pillow's premultiplied-alpha byte-rounding boundaries while
/// avoiding a whole-frame mode conversion.
pub(in crate::compute::pool_simd::ops::adapters::rotate) fn simd_bilinear_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    if matches!(mode, Some("I" | "F")) {
        return Ok(None);
    }
    let Some(geometry) =
        simd_rotate_geometry(img.width(), img.height(), angle, expand, center, translate)
    else {
        return Ok(None);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let destination_width = geometry.width as usize;
    let destination_height = geometry.height as usize;
    if width == 0 || height == 0 || destination_width == 0 || destination_height == 0 {
        return Ok(None);
    }
    let source_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let pa_mode = mode == Some("PA");
    // Pillow's rotate/transform path treats RGBa as already-premultiplied
    // stored samples and RGBX/CMYK as raw four-byte planes. Only RGBA has a
    // straight-alpha channel that needs premultiply/interpolate/unpremultiply.
    let alpha_channel = match (channels, mode) {
        (2, _) if !pa_mode => Some(1),
        (4, None | Some("RGBA")) => Some(3),
        _ => None,
    };
    let mut output = vec![0u8; destination_len];
    let sampler = NativeByteBilinearRotateSampler {
        source,
        width,
        channels,
        alpha_channel,
        fill,
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..destination_height {
        let output_row = y * destination_width * channels;
        let vector_width = destination_width;
        let mut x = 0usize;
        while x < vector_width {
            let active = (destination_width - x).min(SIMD_F64_LANES);
            let samples = std::array::from_fn(|lane| {
                if lane < active {
                    simd_rotate_sample_coordinates(geometry.affine, width, height, x + lane, y)
                } else {
                    SimdRotateSample::default()
                }
            });
            let fx = f64x8::new(samples.map(|sample| sample.fx));
            let fy = f64x8::new(samples.map(|sample| sample.fy));
            let block = sampler.vector_block(&samples, fx, fy);
            let output_start = output_row + x * channels;
            let block_bytes = active * channels;
            output[output_start..output_start + block_bytes].copy_from_slice(&block[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += active;
        }
        while x < destination_width {
            let sample = simd_rotate_sample_coordinates(geometry.affine, width, height, x, y);
            let output_start = output_row + x * channels;
            sampler.scalar_pixel(&mut output, output_start, sample);
            x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(geometry.width, geometry.height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

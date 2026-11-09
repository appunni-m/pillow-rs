//! Affine bilinear sampling for exact native byte layouts.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::rotate::helpers::vector_interpolation::{
    self, LANES, RotateSample,
};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::pipeline::{ResampleFilter, TransformMethod};
use wide::f64x8;

pub(crate) fn execute<const CHANNELS: usize, const ALPHA: bool>(
    source: &[u8],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
) -> Result<Vec<u8>, PilError> {
    let (source_layout, destination, affine) =
        prepare::<CHANNELS>(source, source_dimensions, request)?;
    let mut output = destination.alloc_buffer();
    execute_pixels::<CHANNELS, ALPHA>(
        source,
        source_layout,
        destination,
        affine,
        normalize_fill::<CHANNELS>(request.fill),
        &mut output,
    )?;
    Ok(output)
}

pub(crate) fn execute_into<const CHANNELS: usize, const ALPHA: bool>(
    source: &[u8],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    output: &mut [u8],
) -> Result<(), PilError> {
    let (source_layout, destination, affine) =
        prepare::<CHANNELS>(source, source_dimensions, request)?;
    execute_pixels::<CHANNELS, ALPHA>(
        source,
        source_layout,
        destination,
        affine,
        normalize_fill::<CHANNELS>(request.fill),
        output,
    )
}

fn prepare<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
) -> Result<(CheckedDims, CheckedDims, [f64; 6]), PilError> {
    if CHANNELS == 0 || CHANNELS > 4 {
        return Err(PilError::InternalError(
            "Transform bilinear kernel has an invalid native channel count".into(),
        ));
    }
    if !matches!(request.method, TransformMethod::Affine)
        || !matches!(request.filter, ResampleFilter::Bilinear)
    {
        return Err(PilError::InternalError(
            "affine bilinear Transform kernel received a different request".into(),
        ));
    }
    let [a, b, c, d, e, f] = request.data else {
        return Err(PilError::InternalError(
            "affine bilinear Transform coefficients changed after admission".into(),
        ));
    };
    let source_layout = CheckedDims::new(source_dimensions.0, source_dimensions.1, CHANNELS as u8)?;
    if source.len() != source_layout.total_bytes() {
        return Err(PilError::InternalError(
            "Transform bilinear source buffer shape mismatch".into(),
        ));
    }
    let destination = CheckedDims::new(request.width, request.height, CHANNELS as u8)?;
    Ok((source_layout, destination, [*a, *b, *c, *d, *e, *f]))
}

fn normalize_fill<const CHANNELS: usize>(fill: Option<(u8, u8, u8, u8)>) -> (u8, u8, u8, u8) {
    let fill = fill.unwrap_or((0, 0, 0, 0));
    if CHANNELS == 2 {
        (fill.0, 0, 0, fill.1)
    } else {
        fill
    }
}

fn execute_pixels<const CHANNELS: usize, const ALPHA: bool>(
    source: &[u8],
    source_layout: CheckedDims,
    destination: CheckedDims,
    affine: [f64; 6],
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
) -> Result<(), PilError> {
    if output.len() != destination.total_bytes() {
        return Err(PilError::InternalError(
            "Transform bilinear destination buffer shape mismatch".into(),
        ));
    }
    let alpha_channel = ALPHA.then_some(CHANNELS - 1);
    let sampler = BilinearSampler {
        source,
        source_width: source_layout.width as usize,
        channels: CHANNELS,
        alpha_channel,
        fill,
    };
    let width = destination.width as usize;
    let height = destination.height as usize;
    let source_width = source_layout.width as usize;
    let source_height = source_layout.height as usize;
    let vector_width = if width < LANES {
        width
    } else {
        width / LANES * LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for y in 0..height {
        let row_start = y * width * CHANNELS;
        let mut x = 0usize;
        while x < vector_width {
            let count = (width - x).min(LANES);
            let samples: [RotateSample; LANES] = std::array::from_fn(|lane| {
                if lane < count {
                    vector_interpolation::sample_coordinates(
                        affine,
                        source_width,
                        source_height,
                        x + lane,
                        y,
                    )
                } else {
                    RotateSample::default()
                }
            });
            let fx = f64x8::new(samples.map(|sample| sample.fx));
            let fy = f64x8::new(samples.map(|sample| sample.fy));
            let block = sampler.vector_block(&samples, fx, fy);
            let output_start = row_start + x * CHANNELS;
            let active_bytes = count * CHANNELS;
            output[output_start..output_start + active_bytes]
                .copy_from_slice(&block[..active_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += count;
        }
        while x < width {
            let sample =
                vector_interpolation::sample_coordinates(affine, source_width, source_height, x, y);
            let output_start = row_start + x * CHANNELS;
            sampler.scalar_pixel(&mut output[output_start..output_start + CHANNELS], sample);
            scalar_tail = scalar_tail.saturating_add(1);
            x += 1;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(())
}

struct BilinearSampler<'a> {
    source: &'a [u8],
    source_width: usize,
    channels: usize,
    alpha_channel: Option<usize>,
    fill: (u8, u8, u8, u8),
}

impl BilinearSampler<'_> {
    #[inline(always)]
    fn vector_block(
        &self,
        samples: &[RotateSample; LANES],
        fx: f64x8,
        fy: f64x8,
    ) -> [u8; LANES * 4] {
        let mut block = [0u8; LANES * 4];
        let mut alpha = [0u8; LANES];
        if let Some(alpha_channel) = self.alpha_channel {
            let neighbors = vector_interpolation::gather_neighborhood(
                self.source,
                self.source_width,
                self.channels,
                alpha_channel,
                self.fill,
                samples,
                None,
            );
            alpha = vector_interpolation::bilinear_vector(neighbors, fx, fy)
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
            let neighbors = vector_interpolation::gather_neighborhood(
                self.source,
                self.source_width,
                self.channels,
                channel,
                self.fill,
                samples,
                self.alpha_channel,
            );
            let interpolated = vector_interpolation::bilinear_vector(neighbors, fx, fy);
            let values = if self.alpha_channel.is_some() {
                vector_interpolation::unpremultiply_vector(interpolated, alpha)
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
    fn scalar_pixel(&self, output: &mut [u8], sample: RotateSample) {
        let mut alpha = 0u8;
        if let Some(alpha_channel) = self.alpha_channel {
            let alpha_value = if sample.valid {
                bilinear_scalar(
                    self.read(sample.x0, sample.y0, alpha_channel),
                    self.read(sample.x1, sample.y0, alpha_channel),
                    self.read(sample.x0, sample.y1, alpha_channel),
                    self.read(sample.x1, sample.y1, alpha_channel),
                    sample.fx,
                    sample.fy,
                )
            } else {
                self.fill_sample(alpha_channel) as f64
            };
            alpha = alpha_value as u8;
            output[alpha_channel] = alpha;
        }
        for channel in 0..self.channels {
            if Some(channel) == self.alpha_channel {
                continue;
            }
            let value = (if sample.valid {
                let mut p00 = self.read(sample.x0, sample.y0, channel);
                let mut p10 = self.read(sample.x1, sample.y0, channel);
                let mut p01 = self.read(sample.x0, sample.y1, channel);
                let mut p11 = self.read(sample.x1, sample.y1, channel);
                if let Some(alpha_channel) = self.alpha_channel {
                    p00 = premultiply(p00, self.read(sample.x0, sample.y0, alpha_channel));
                    p10 = premultiply(p10, self.read(sample.x1, sample.y0, alpha_channel));
                    p01 = premultiply(p01, self.read(sample.x0, sample.y1, alpha_channel));
                    p11 = premultiply(p11, self.read(sample.x1, sample.y1, alpha_channel));
                }
                bilinear_scalar(p00, p10, p01, p11, sample.fx, sample.fy)
            } else {
                self.fill_sample(channel) as f64
            }) as u8;
            output[channel] = if self.alpha_channel.is_some() && alpha != 0 {
                (f64::from(value) * 255.0 / f64::from(alpha)) as u8
            } else {
                value
            };
        }
    }

    #[inline(always)]
    fn read(&self, x: usize, y: usize, channel: usize) -> f64 {
        self.source[(y * self.source_width + x) * self.channels + channel] as f64
    }

    #[inline(always)]
    fn fill_sample(&self, channel: usize) -> u8 {
        if self.channels == 2 && channel == 1 {
            self.fill.3
        } else {
            match channel {
                0 => self.fill.0,
                1 => self.fill.1,
                2 => self.fill.2,
                _ => self.fill.3,
            }
        }
    }
}

#[inline(always)]
fn premultiply(value: f64, alpha: f64) -> f64 {
    (value * alpha / 255.0 + 0.5) as u8 as f64
}

#[inline(always)]
fn bilinear_scalar(p00: f64, p10: f64, p01: f64, p11: f64, fx: f64, fy: f64) -> f64 {
    let top = (p10 - p00).mul_add(fx, p00);
    let bottom = (p11 - p01).mul_add(fx, p01);
    (bottom - top).mul_add(fy, top)
}

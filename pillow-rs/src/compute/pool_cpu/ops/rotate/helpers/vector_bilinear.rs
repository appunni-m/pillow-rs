//! Native-byte bilinear Rotate kernel selected by exact mode strategies.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::rotate::helpers::geometry::rotate_geometry;
use crate::compute::pool_cpu::ops::rotate::helpers::vector_interpolation::{
    self, RotateSample, LANES,
};
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};
use wide::f64x8;

pub(crate) fn execute<P, const CHANNELS: usize, const ALPHA: bool>(
    image: &ImageBuffer<P, Vec<u8>>,
    request: RotateRequest,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let geometry = rotate_geometry(image.width(), image.height(), request);
    let dimensions = CheckedDims::new(geometry.width, geometry.height, CHANNELS as u8)?;
    let source_width = usize::try_from(image.width())
        .map_err(|_| PilError::ValueError("Rotate source width overflow".into()))?;
    let source_height = usize::try_from(image.height())
        .map_err(|_| PilError::ValueError("Rotate source height overflow".into()))?;
    let expected_source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .ok_or_else(|| PilError::ValueError("Rotate source dimensions overflow".into()))?;
    if source_width == 0
        || source_height == 0
        || geometry.width == 0
        || geometry.height == 0
        || image.as_raw().len() != expected_source_len
    {
        return Err(PilError::InternalError(
            "Rotate bilinear source buffer shape mismatch".into(),
        ));
    }

    let mut output = dimensions.alloc_buffer();
    let fill = request.fill.unwrap_or((0, 0, 0, 0));
    let alpha_channel = if ALPHA { Some(CHANNELS - 1) } else { None };
    let sampler = BilinearSampler {
        source: image.as_raw(),
        width: source_width,
        channels: CHANNELS,
        alpha_channel,
        fill,
    };
    let destination_width = geometry.width as usize;
    let mut vector_blocks = 0_u64;
    for y in 0..geometry.height as usize {
        let row_start = y * destination_width * CHANNELS;
        let mut x = 0_usize;
        while x < destination_width {
            let active = (destination_width - x).min(LANES);
            let samples: [RotateSample; LANES] = std::array::from_fn(|lane| {
                if lane < active {
                    vector_interpolation::sample_coordinates(
                        geometry.affine,
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
            let active_bytes = active * CHANNELS;
            output[output_start..output_start + active_bytes]
                .copy_from_slice(&block[..active_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += active;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::typed_bytes_to_buffer::<P>(dimensions, output)
}

struct BilinearSampler<'a> {
    source: &'a [u8],
    width: usize,
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
        let mut block = [0_u8; LANES * 4];
        let mut alpha = [0_u8; LANES];
        if let Some(alpha_channel) = self.alpha_channel {
            let neighbors = vector_interpolation::gather_neighborhood(
                self.source,
                self.width,
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
                self.width,
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
}

//! SIMD EffectNoise and EffectSpread family adapters.

use super::{
    SimdImageShape, SimdLayout, f64x8, has_empty_native_bytes, has_valid_byte_data,
    native_copy_layout, shape_has_empty_native_bytes, shape_has_nonempty_byte_data,
    shape_native_identity_copy_channels,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView, GrayImage};
use wide::u8x16;

/// EffectNoise writes a native L image.
pub(super) fn output_layout() -> SimdLayout {
    SimdLayout::Luma8
}

/// EffectNoise changes the logical mode to L.
pub(super) fn output_mode() -> &'static str {
    "L"
}

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::EffectNoise { sigma } => {
            sigma.is_finite()
                && img.height() != 0
                && (img.width() as usize).saturating_mul(img.height() as usize) != 0
        }
        PipelineOp::EffectSpread { .. } => native_copy_layout(img, mode).is_some_and(|channels| {
            has_empty_native_bytes(img, channels) || has_valid_byte_data(img, channels)
        }),
        _ => false,
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match op {
        PipelineOp::EffectNoise { sigma } => {
            sigma.is_finite()
                && shape.height != 0
                && (shape.width as usize).saturating_mul(shape.height as usize) != 0
        }
        PipelineOp::EffectSpread { .. } => shape_native_identity_copy_channels(shape, mode)
            .is_some_and(|channels| {
                shape_has_empty_native_bytes(shape, channels)
                    || shape_has_nonempty_byte_data(shape, channels)
            }),
        _ => false,
    }
}

/// Generate Pillow's deterministic effect-noise stream with scalar RNG and
/// Box-Muller control, then vectorize the per-sample affine/clamp arithmetic.
/// The RNG sequence and rejection order remain scalar and process-global;
/// every complete eight-pixel group uses the SIMD data plane for
/// `128 + sigma * deviate` before the final byte conversion.
pub(crate) fn simd_effect_noise(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::EffectNoise { sigma } = op else {
        return Err(PilError::ValueError("expected EffectNoise op".into()));
    };
    let (width, height) = img.dimensions();
    if height == 0 || !sigma.is_finite() {
        return Err(super::simd_unsupported("EffectNoise"));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD EffectNoise pixel count overflow".into()))?;
    if pixels == 0 {
        return Err(super::simd_unsupported("EffectNoise"));
    }
    let sigma = f64::from(*sigma as f32);
    const RAND_MAX_F64: f64 = 2_147_483_647.0;
    let mut output = vec![0u8; pixels];

    crate::compute::pool_cpu::ops::effects::with_process_rng(|rng| {
        let mut pixel = 0usize;
        while pixel < pixels {
            let active = (pixels - pixel).min(8);
            let mut deviates = [0.0; 8];
            for deviate in deviates.iter_mut().take(active) {
                let (v1, radius) = loop {
                    let v1 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
                    let v2 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
                    let radius = v1 * v1 + v2 * v2;
                    if radius < 1.0 {
                        break (v1, radius);
                    }
                };
                let factor = (-2.0 * radius.ln() / radius).sqrt();
                *deviate = factor * v1;
            }
            let values = f64x8::new(deviates) * f64x8::splat(sigma) + f64x8::splat(128.0);
            for (destination, value) in output[pixel..pixel + active]
                .iter_mut()
                .zip(values.to_array().into_iter().take(active))
            {
                *destination = if value <= 0.0 {
                    0
                } else if value >= 255.0 {
                    255
                } else {
                    value as u8
                };
            }
            // The final partial batch still uses the vector arithmetic path;
            // inactive lanes are zero-filled and are never written. This is
            // important for effect_noise because every accepted pixel must
            // consume its RNG pair even when the image has fewer than eight
            // pixels, otherwise later cases observe a shifted process stream.
            pixel += active;
        }
    })?;

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(pixels.div_ceil(8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    GrayImage::from_raw(width, height, output)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD EffectNoise buffer shape mismatch".into()))
}

/// Execute Pillow's EffectSpread with scalar RNG/coordinate control and a
/// native vector load/store data plane.
///
/// The destination mapping is intentionally processed in source order: the
/// C implementation reads both pixels from the immutable input and performs
/// ordered writes, so a parallel scatter would change collision semantics.
/// Each native pixel group is loaded/stored through `u8x16`; the scalar part
/// is limited to RNG and bounds calculation, never a CPU adapter delegation.
fn simd_effect_spread_native(
    img: &DynamicImage,
    distance: u32,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = super::native_copy_layout(img, mode) else {
        return Ok(None);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD EffectSpread image length overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != expected_len {
        return Ok(None);
    }
    if expected_len == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return crate::image_utils::raw_bytes_to_image_allow_empty(
            img.width(),
            img.height(),
            Vec::new(),
            channels,
        )
        .map(|result| Some(super::preserve_mode(img, result)));
    }
    if distance == 0 {
        let mut output = vec![0u8; expected_len];
        let (vector_blocks, scalar_tail) = super::copy_native_bytes(source, &mut output)
            .ok_or_else(|| {
                PilError::InternalError("SIMD EffectSpread copy shape mismatch".into())
            })?;
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
            .map(|result| Some(super::preserve_mode(img, result)));
    }
    if distance == 1 {
        crate::compute::pool_cpu::ops::effects::effect_spread_advance_identity_rng(
            img.width(),
            img.height(),
        )?;
        crate::compute::record_pipeline_operation_path("native-identity");
        crate::compute::record_pipeline_operation_vector_blocks(0);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        return Ok(Some(super::preserve_mode(img, img.clone())));
    }

    let output = simd_effect_spread_vectorized(img, source, channels, width, height, distance)?;
    Ok(Some(output))
}

fn simd_effect_spread_vectorized(
    img: &DynamicImage,
    source: &[u8],
    channels: usize,
    width: usize,
    height: usize,
    distance: u32,
) -> Result<DynamicImage, PilError> {
    let width_i64 = i64::try_from(width)
        .map_err(|_| PilError::ValueError("SIMD EffectSpread width overflow".into()))?;
    let height_i64 = i64::try_from(height)
        .map_err(|_| PilError::ValueError("SIMD EffectSpread height overflow".into()))?;
    let d = i64::from(distance);
    let half_d = d / 2;
    let mut output = source.to_vec();
    let (vector_blocks, scalar_tail) =
        crate::compute::pool_cpu::ops::effects::with_process_rng(|rng| {
            let mut vector_blocks = 0u64;
            for y in 0..height_i64 {
                for x in 0..width_i64 {
                    let source_pixel = (y * width_i64 + x) as usize;
                    let source_start = source_pixel * channels;
                    let xx = x + i64::from(rng.next() % distance) - half_d;
                    let yy = y + i64::from(rng.next() % distance) - half_d;
                    let source_block = {
                        let mut block = [0u8; 16];
                        block[..channels]
                            .copy_from_slice(&source[source_start..source_start + channels]);
                        block
                    };
                    if xx >= 0 && xx < width_i64 && yy >= 0 && yy < height_i64 {
                        let destination_pixel = (yy * width_i64 + xx) as usize;
                        let destination_start = destination_pixel * channels;
                        let destination_block = {
                            let mut block = [0u8; 16];
                            block[..channels].copy_from_slice(
                                &source[destination_start..destination_start + channels],
                            );
                            block
                        };
                        let source_vector = u8x16::new(source_block).to_array();
                        let destination_vector = u8x16::new(destination_block).to_array();
                        output[destination_start..destination_start + channels]
                            .copy_from_slice(&source_vector[..channels]);
                        output[source_start..source_start + channels]
                            .copy_from_slice(&destination_vector[..channels]);
                    } else {
                        let source_vector = u8x16::new(source_block).to_array();
                        output[source_start..source_start + channels]
                            .copy_from_slice(&source_vector[..channels]);
                    }
                    vector_blocks = vector_blocks.saturating_add(1);
                }
            }
            (vector_blocks, 0u64)
        })?;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .map(|result| super::preserve_mode(img, result))
}

pub(crate) fn simd_effect_spread(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::EffectSpread { distance } = op else {
        return Err(PilError::ValueError("expected EffectSpread op".into()));
    };
    simd_effect_spread_native(img, *distance, mode)?
        .ok_or_else(|| super::simd_unsupported("EffectSpread"))
}

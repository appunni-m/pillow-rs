//! Native I/F bilinear Rotate kernel.

use super::super::super::super::SIMD_F64_LANES;
use super::super::super::{
    SimdRotateSample, rotate_fused_mul_add, simd_rotate_geometry, simd_rotate_sample_coordinates,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::DynamicImage;
use wide::{f32x8, f64x8, i32x8};

#[inline]
fn typed_bilinear_rotate_vector(
    p00: [f64; SIMD_F64_LANES],
    p10: [f64; SIMD_F64_LANES],
    p01: [f64; SIMD_F64_LANES],
    p11: [f64; SIMD_F64_LANES],
    fx: f64x8,
    fy: f64x8,
    is_float: bool,
) -> f64x8 {
    // Geometry.c subtracts source samples in their native type before
    // widening for interpolation. F rounds that subtraction to f32; I wraps
    // the i32 difference and evaluates the subsequent multiply/add separately.
    let difference = |right: [f64; SIMD_F64_LANES], left: [f64; SIMD_F64_LANES]| {
        if is_float {
            let difference = f32x8::new(right.map(|value| value as f32))
                - f32x8::new(left.map(|value| value as f32));
            f64x8::new(difference.to_array().map(f64::from))
        } else {
            let difference = i32x8::new(right.map(|value| value as i32))
                - i32x8::new(left.map(|value| value as i32));
            f64x8::new(difference.to_array().map(f64::from))
        }
    };
    let top_delta = difference(p10, p00);
    let bottom_delta = difference(p11, p01);
    if is_float {
        let top = rotate_fused_mul_add(top_delta, fx, f64x8::new(p00));
        let bottom = rotate_fused_mul_add(bottom_delta, fx, f64x8::new(p01));
        rotate_fused_mul_add(bottom - top, fy, top)
    } else {
        let top = top_delta * fx + f64x8::new(p00);
        let bottom = bottom_delta * fx + f64x8::new(p01);
        (bottom - top) * fy + top
    }
}

struct TypedBilinearRotateSamples {
    p00: [f64; SIMD_F64_LANES],
    p10: [f64; SIMD_F64_LANES],
    p01: [f64; SIMD_F64_LANES],
    p11: [f64; SIMD_F64_LANES],
    fx: [f64; SIMD_F64_LANES],
    fy: [f64; SIMD_F64_LANES],
}

#[inline(always)]
fn typed_bilinear_rotate_samples(
    source: &[u8],
    width: usize,
    height: usize,
    affine: [f64; 6],
    destination_x: usize,
    destination_y: usize,
    active: usize,
    scalar_fill: f64,
    is_float: bool,
) -> TypedBilinearRotateSamples {
    let mut samples = TypedBilinearRotateSamples {
        p00: [scalar_fill; SIMD_F64_LANES],
        p10: [scalar_fill; SIMD_F64_LANES],
        p01: [scalar_fill; SIMD_F64_LANES],
        p11: [scalar_fill; SIMD_F64_LANES],
        fx: [0.0; SIMD_F64_LANES],
        fy: [0.0; SIMD_F64_LANES],
    };

    for lane in 0..active {
        let x = destination_x + lane;
        let sample = simd_rotate_sample_coordinates(affine, width, height, x, destination_y);
        if sample.valid {
            let SimdRotateSample { x0, x1, y0, y1, .. } = sample;
            samples.fx[lane] = sample.fx;
            samples.fy[lane] = sample.fy;
            let read = |source_x: usize, source_y: usize| {
                let index = (source_y * width + source_x) * 4;
                if is_float {
                    f32::from_le_bytes([
                        source[index],
                        source[index + 1],
                        source[index + 2],
                        source[index + 3],
                    ]) as f64
                } else {
                    i32::from_le_bytes([
                        source[index],
                        source[index + 1],
                        source[index + 2],
                        source[index + 3],
                    ]) as f64
                }
            };
            samples.p00[lane] = read(x0, y0);
            samples.p10[lane] = read(x1, y0);
            samples.p01[lane] = read(x0, y1);
            samples.p11[lane] = read(x1, y1);
        }
    }
    samples
}

/// Rotate native four-byte `I`/`F` samples without treating their scalar
/// bytes as independent color channels.
///
/// Affine coordinate construction and gathers stay scalar control because a
/// portable gather is not exposed by `wide`; the bilinear sample arithmetic
/// is evaluated in eight `f64` lanes. Partial output blocks are padded and
/// only their active lanes are stored, so small public images still execute
/// the same vector data path rather than being rejected or sent to CPU.
pub(in crate::compute::pool_simd::ops::adapters::rotate) fn simd_typed_bilinear_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let is_float = match mode {
        Some("F") => true,
        Some("I") => false,
        _ => return Ok(None),
    };
    if !matches!(img, DynamicImage::ImageRgba8(_)) {
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
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let scalar_fill = if is_float {
        f32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    } else {
        i32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    };
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let active = (destination_width - destination_x).min(SIMD_F64_LANES);
            let samples = typed_bilinear_rotate_samples(
                source,
                width,
                height,
                geometry.affine,
                destination_x,
                destination_y,
                active,
                scalar_fill,
                is_float,
            );

            let values = typed_bilinear_rotate_vector(
                samples.p00,
                samples.p10,
                samples.p01,
                samples.p11,
                f64x8::new(samples.fx),
                f64x8::new(samples.fy),
                is_float,
            )
            .to_array();
            for lane in 0..active {
                let bytes = if is_float {
                    (values[lane] as f32).to_le_bytes()
                } else {
                    (values[lane] as i32).to_le_bytes()
                };
                let output_index = (destination_y * destination_width + destination_x + lane) * 4;
                output[output_index..output_index + 4].copy_from_slice(&bytes);
            }
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += active;
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(geometry.width, geometry.height, output, 4)
        .map(|result| Some(preserve_mode(img, result)))
}

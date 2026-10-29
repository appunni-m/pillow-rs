//! Typed vector bilinear Rotate for the native `I` and `F` scalar carriers.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::rotate::helpers::geometry::rotate_geometry;
use crate::compute::pool_cpu::ops::rotate::helpers::vector_interpolation::{
    self, LANES,
};
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::raster::{GenericImageView, Rgba, RgbaImage};
use wide::{f32x8, f64x8, i32x8};

pub(crate) fn execute<const FLOAT: bool>(
    image: &RgbaImage,
    request: RotateRequest,
) -> Result<RgbaImage, PilError> {
    let geometry = rotate_geometry(image.width(), image.height(), request);
    let dimensions = CheckedDims::new(geometry.width, geometry.height, 4)?;
    let width = usize::try_from(image.width())
        .map_err(|_| PilError::ValueError("Rotate source width overflow".into()))?;
    let height = usize::try_from(image.height())
        .map_err(|_| PilError::ValueError("Rotate source height overflow".into()))?;
    let source_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::ValueError("Rotate source dimensions overflow".into()))?;
    if width == 0
        || height == 0
        || geometry.width == 0
        || geometry.height == 0
        || image.as_raw().len() != source_len
    {
        return Err(PilError::InternalError(
            "Rotate typed bilinear source buffer shape mismatch".into(),
        ));
    }

    let fill = request.fill.unwrap_or((0, 0, 0, 0));
    let fill_bytes = [fill.0, fill.1, fill.2, fill.3];
    let scalar_fill = if FLOAT {
        f32::from_le_bytes(fill_bytes) as f64
    } else {
        i32::from_le_bytes(fill_bytes) as f64
    };
    let mut output = dimensions.alloc_buffer();
    let destination_width = geometry.width as usize;
    let destination_height = geometry.height as usize;
    let mut vector_blocks = 0_u64;
    for y in 0..destination_height {
        let mut x = 0_usize;
        while x < destination_width {
            let active = (destination_width - x).min(LANES);
            let samples = typed_samples::<FLOAT>(
                image.as_raw(), width, height, geometry.affine, x, y, active, scalar_fill,
            );
            let values = bilinear_vector::<FLOAT>(&samples).to_array();
            for lane in 0..active {
                let bytes = if FLOAT {
                    (values[lane] as f32).to_le_bytes()
                } else {
                    (values[lane] as i32).to_le_bytes()
                };
                let index = (y * destination_width + x + lane) * 4;
                output[index..index + 4].copy_from_slice(&bytes);
            }
            vector_blocks = vector_blocks.saturating_add(1);
            x += active;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::typed_bytes_to_buffer::<Rgba<u8>>(dimensions, output)
}

struct TypedSamples {
    p00: [f64; LANES],
    p10: [f64; LANES],
    p01: [f64; LANES],
    p11: [f64; LANES],
    fx: [f64; LANES],
    fy: [f64; LANES],
}

fn typed_samples<const FLOAT: bool>(
    source: &[u8],
    width: usize,
    height: usize,
    affine: [f64; 6],
    destination_x: usize,
    destination_y: usize,
    active: usize,
    scalar_fill: f64,
) -> TypedSamples {
    let mut samples = TypedSamples {
        p00: [scalar_fill; LANES],
        p10: [scalar_fill; LANES],
        p01: [scalar_fill; LANES],
        p11: [scalar_fill; LANES],
        fx: [0.0; LANES],
        fy: [0.0; LANES],
    };
    for lane in 0..active {
        let sample = vector_interpolation::sample_coordinates(
            affine, width, height, destination_x + lane, destination_y,
        );
        if !sample.valid {
            continue;
        }
        samples.fx[lane] = sample.fx;
        samples.fy[lane] = sample.fy;
        samples.p00[lane] = scalar_sample::<FLOAT>(source, width, sample.x0, sample.y0);
        samples.p10[lane] = scalar_sample::<FLOAT>(source, width, sample.x1, sample.y0);
        samples.p01[lane] = scalar_sample::<FLOAT>(source, width, sample.x0, sample.y1);
        samples.p11[lane] = scalar_sample::<FLOAT>(source, width, sample.x1, sample.y1);
    }
    samples
}

fn scalar_sample<const FLOAT: bool>(source: &[u8], width: usize, x: usize, y: usize) -> f64 {
    let index = (y * width + x) * 4;
    let bytes = [source[index], source[index + 1], source[index + 2], source[index + 3]];
    if FLOAT {
        f64::from(f32::from_le_bytes(bytes))
    } else {
        f64::from(i32::from_le_bytes(bytes))
    }
}

fn bilinear_vector<const FLOAT: bool>(samples: &TypedSamples) -> f64x8 {
    let difference = |right: [f64; LANES], left: [f64; LANES]| {
        if FLOAT {
            let delta = f32x8::new(right.map(|value| value as f32))
                - f32x8::new(left.map(|value| value as f32));
            f64x8::new(delta.to_array().map(f64::from))
        } else {
            let delta = i32x8::new(right.map(|value| value as i32))
                - i32x8::new(left.map(|value| value as i32));
            f64x8::new(delta.to_array().map(f64::from))
        }
    };
    let fx = f64x8::new(samples.fx);
    let fy = f64x8::new(samples.fy);
    let top_delta = difference(samples.p10, samples.p00);
    let bottom_delta = difference(samples.p11, samples.p01);
    if FLOAT {
        let top = vector_interpolation::fused_mul_add(top_delta, fx, f64x8::new(samples.p00));
        let bottom =
            vector_interpolation::fused_mul_add(bottom_delta, fx, f64x8::new(samples.p01));
        vector_interpolation::fused_mul_add(bottom - top, fy, top)
    } else {
        let top = top_delta * fx + f64x8::new(samples.p00);
        let bottom = bottom_delta * fx + f64x8::new(samples.p01);
        (bottom - top) * fy + top
    }
}

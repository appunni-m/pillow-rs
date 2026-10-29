//! SIMD data plane for deterministic EffectNoise sample generation.

use crate::compute::helpers::effects::with_process_rng;
use crate::error::PilError;
use crate::raster::GrayImage;
use wide::f64x8;

pub(crate) struct Kernel {
    width: u32,
    height: u32,
    sigma: f64,
}

pub(crate) fn prepare(width: u32, height: u32, sigma: f64) -> Kernel {
    Kernel { width, height, sigma }
}

pub(crate) fn execute_sse2(kernel: Kernel) -> Result<GrayImage, PilError> {
    execute_vector(kernel)
}

pub(crate) fn execute_neon(kernel: Kernel) -> Result<GrayImage, PilError> {
    execute_vector(kernel)
}

pub(crate) fn execute_wasm_simd128(kernel: Kernel) -> Result<GrayImage, PilError> {
    execute_vector(kernel)
}

fn execute_vector(kernel: Kernel) -> Result<GrayImage, PilError> {
    let pixels = usize::try_from(kernel.width)
        .ok()
        .and_then(|width| {
            usize::try_from(kernel.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("SIMD EffectNoise pixel count overflow".into()))?;
    let sigma = f64::from(kernel.sigma as f32);
    const RAND_MAX_F64: f64 = 2_147_483_647.0;
    let mut output = vec![0u8; pixels];

    with_process_rng(|rng| {
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
                *deviate = (-2.0 * radius.ln() / radius).sqrt() * v1;
            }
            let values = f64x8::new(deviates) * f64x8::splat(sigma) + f64x8::splat(128.0);
            for (destination, value) in output[pixel..pixel + active]
                .iter_mut()
                .zip(values.to_array().into_iter().take(active))
            {
                *destination = clamp_sample(value);
            }
            pixel += active;
        }
    })?;

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(pixels.div_ceil(8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    GrayImage::from_raw(kernel.width, kernel.height, output)
        .ok_or_else(|| PilError::InternalError("SIMD EffectNoise buffer shape mismatch".into()))
}

#[inline]
fn clamp_sample(value: f64) -> u8 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}

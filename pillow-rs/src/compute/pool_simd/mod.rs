//! SIMD worker pool — implements BackendImpl for SIMD-accelerated CPU compute.
//!
//! Uses the `wide` crate for portable SIMD (SSE, AVX, NEON) to process pixels
//! in vectorized chunks. Unsupported operations are routed to another backend
//! before execution; an explicitly locked SIMD pipeline reports an error.
//!
//! ## Architecture
//! - Same mode encoding as GPU: 0=L, 1=LA, 2=RGB, 3=RGBA
//! - Processes RGBA8 packed u32 as 4 independent u8 lanes
//! - Priority: 50 (above CPU=0, below GPU=100)
//! - Ops live in `ops/` mirroring `pool_cpu/ops/`

use crate::compute::registry;
use crate::compute::{Backend, BackendImpl};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(crate) mod ops;

// ─── SimdPool ──────────────────────────────────────────────────────────────

/// SIMD compute pool — CPU-accelerated via portable SIMD vectors.
///
/// Executes only operations with a registered SIMD implementation.
pub struct SimdPool;

fn normalize_palette_result(
    result: DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    match mode {
        Some("P") => {
            let rgba = result.to_rgba8();
            let (width, height) = rgba.dimensions();
            let indices = rgba.pixels().map(|pixel| pixel[0]).collect();
            crate::raster::GrayImage::from_raw(width, height, indices)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| {
                    PilError::InternalError("SIMD P-mode buffer shape mismatch".to_string())
                })
        }
        Some("PA") => {
            let rgba = result.to_rgba8();
            let (width, height) = rgba.dimensions();
            let samples = rgba
                .pixels()
                .flat_map(|pixel| [pixel[0], pixel[3]])
                .collect();
            crate::raster::GrayAlphaImage::from_raw(width, height, samples)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| {
                    PilError::InternalError("SIMD PA-mode buffer shape mismatch".to_string())
                })
        }
        _ => Ok(result),
    }
}

impl BackendImpl for SimdPool {
    fn name(&self) -> Backend {
        Backend::Simd
    }

    fn priority(&self) -> u8 {
        50 // Above CPU (0), below GPU (100)
    }

    fn supports(&self, op: &PipelineOp) -> Result<bool, PilError> {
        registry::simd_supports(op)
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let op_keys: Vec<&str> = ops.iter().map(|op| registry::variant_key(op)).collect();
        log::debug!(
            "[SIMD] {} op(s) {}x{}: {:?}",
            ops.len(),
            img.width(),
            img.height(),
            op_keys
        );

        let mut current = img.clone();
        for op in ops {
            let key = registry::variant_key(op);
            let entry = registry::registry()?
                .get(key)
                .ok_or_else(|| PilError::ValueError(format!("SIMD: unknown op {}", key)))?;
            let f = entry
                .simd_fn
                .ok_or_else(|| PilError::ValueError(format!("SIMD: no native impl for {}", key)))?;
            current = f(&current, op, mode)?;
        }
        normalize_palette_result(current, mode)
    }
}

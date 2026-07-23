//! SIMD worker pool — implements BackendImpl for SIMD-accelerated CPU compute.
//!
//! Uses the `wide` crate for portable SIMD (SSE, AVX, NEON) to process pixels
//! in vectorized chunks. Falls back to scalar CPU for unsupported ops.
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
use image_slash_star::DynamicImage;

pub mod ops;

// ─── SimdPool ──────────────────────────────────────────────────────────────

/// SIMD compute pool — CPU-accelerated via portable SIMD vectors.
///
/// Falls back to scalar CPU when SIMD instructions aren't available
/// or for ops that don't benefit from vectorization (Paste, Transform, etc.).
pub struct SimdPool;

impl BackendImpl for SimdPool {
    fn name(&self) -> Backend {
        Backend::Simd
    }

    fn priority(&self) -> u8 {
        50 // Above CPU (0), below GPU (100)
    }

    fn supports(&self, op: &PipelineOp) -> bool {
        // SIMD accelerates anything the CPU can do. As SIMD-specific functions
        // are added via simd_entry!, those become the preferred path.
        // For now, delegate to CPU functions with auto-vectorization doing the work.
        registry::cpu_supports(op)
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

        // Use SIMD-accelerated functions when available, CPU fallback otherwise.
        let mut current = img.clone();
        for op in ops {
            let key = registry::variant_key(op);
            let entry = registry::registry()
                .get(key)
                .ok_or_else(|| PilError::ValueError(format!("SIMD: unknown op {}", key)))?;
            // Prefer SIMD fn, fall back to CPU fn
            let f = entry
                .simd_fn
                .or(entry.cpu_fn)
                .ok_or_else(|| PilError::ValueError(format!("SIMD: no impl for {}", key)))?;
            current = f(&current, op, mode)?;
        }
        Ok(current)
    }
}

//! CPU worker pool — implements BackendImpl for CPU backend.

use crate::compute::registry;
use crate::compute::{Backend, BackendImpl};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use image_slash_star::DynamicImage;

pub(crate) mod ops;

/// CPU compute pool — processes all operations on the CPU.
/// This is the fallback pool that supports every PipelineOp.
pub struct CpuPool;

impl BackendImpl for CpuPool {
    fn name(&self) -> Backend {
        Backend::Cpu
    }

    fn priority(&self) -> u8 {
        0
    }

    fn supports(&self, op: &PipelineOp) -> Result<bool, PilError> {
        registry::cpu_supports(op)
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let mut result = img.clone();
        for op in ops {
            result = registry::execute_cpu(op, &result, mode)?;
        }
        Ok(result)
    }
}

use crate::compute::{Backend, ComputeBackend};
use crate::error::PilError;
use crate::image::execute_op;
use crate::pipeline::PipelineOp;
use image::DynamicImage;

pub struct CpuBackend;

impl ComputeBackend for CpuBackend {
    fn name(&self) -> Backend {
        Backend::Cpu
    }

    fn priority(&self) -> u8 {
        0
    }

    fn supports(&self, _op: &PipelineOp) -> bool {
        true
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        explicit_mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let mut img = img.clone();
        for op in ops {
            img = execute_op(&img, op, explicit_mode)?;
        }
        Ok(img)
    }
}

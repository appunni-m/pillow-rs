//! Maintained serial CPU backend.
//!
//! Every operation receives the execution context selected by the router and
//! prepares one concrete mode/architecture strategy. Parallel scheduling stays
//! outside this backend.

use crate::compute::{Backend, BackendImpl, ExecutionContext, registry};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

mod batch_execution;
pub(crate) mod helpers;
pub(crate) mod ops;

/// Serial CPU pool. It uses the exact mode carried by each `DynamicImage`.
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
        image: &DynamicImage,
        _mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let context = ExecutionContext::for_routed_backend(Backend::Cpu);
        batch_execution::execute_batch(ops, image, &context, Default::default())
    }

    fn execute_batch_with_context(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        _mode: Option<&str>,
        context: &ExecutionContext,
    ) -> Result<DynamicImage, PilError> {
        batch_execution::execute_batch(ops, image, context, Default::default())
    }

    fn execute_batch_with_context_and_palette(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        _mode: Option<&str>,
        context: &ExecutionContext,
        palette: registry::support::PaletteInput<'_>,
    ) -> Result<DynamicImage, PilError> {
        batch_execution::execute_batch(ops, image, context, palette)
    }
}

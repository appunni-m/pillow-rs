//! Execute nested Paste children with their parent's selected CPU context.

use std::sync::Arc;

use crate::compute::registry::support::OperationInputs;
use crate::compute::{Backend, CpuExecutionRoute, ExecutionContext};
use crate::error::PilError;
use crate::image::Image;
use crate::ops::paste::PasteSource;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

impl Image {
    /// Admits and executes a masked Paste child under its parent's context.
    ///
    /// The ordinary Paste path retains ownership of input validation and
    /// operation construction. This method materializes the existing
    /// destination, then prepares the admitted child without selecting another
    /// backend or architecture.
    // TODO(pillow-rs#3ptb): verify nested Draw Text Paste output, errors, and
    // context receipts against the frozen CPU mode/architecture matrix.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn execute_paste_at_with_context(
        &self,
        source: PasteSource,
        position: (i32, i32),
        mask: &Image,
        requested_backend: Option<Backend>,
        context: ExecutionContext,
    ) -> Result<Option<DynamicImage>, PilError> {
        let mut queued = self.clone();
        queued.paste_at_with_execution_route(
            source,
            position,
            Some(mask),
            CpuExecutionRoute::new(requested_backend, context),
        )?;
        self.execute_queued_paste_with_context(queued, requested_backend, context)
    }

    /// Admits and executes a compact solid-mask Bitmap child in its parent's context.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn execute_bitmap_paste_at_with_context(
        &self,
        source: PasteSource,
        position: (i32, i32),
        mask: &Image,
        requested_backend: Option<Backend>,
        context: ExecutionContext,
    ) -> Result<Option<DynamicImage>, PilError> {
        let mut queued = self.clone();
        queued.paste_bitmap_solid_masked_at_with_execution_route(
            source,
            position,
            mask,
            CpuExecutionRoute::new(requested_backend, context),
        )?;
        self.execute_queued_paste_with_context(queued, requested_backend, context)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn execute_queued_paste_with_context(
        &self,
        queued: Image,
        requested_backend: Option<Backend>,
        context: ExecutionContext,
    ) -> Result<Option<DynamicImage>, PilError> {
        if !paste_pipeline_was_added(self, &queued) {
            return Ok(None);
        }
        let op = last_pipeline_op(&queued)
            .ok_or_else(|| PilError::InternalError("Paste operation was not queued".into()))?;
        if !matches!(&op, PipelineOp::Paste { .. }) {
            return Err(PilError::InternalError(
                "queued Draw child was not a Paste operation".into(),
            ));
        }
        let input_image = self.materialized_shared_for_ops()?;
        let input = OperationInputs {
            image: input_image.as_ref(),
            palette: self.palette_input(),
        };
        crate::compute::execute_eager_cpu_arch_image_operation_with_context(
            "Paste",
            input.image,
            context,
            requested_backend,
            None,
            |context| {
                let operation = crate::compute::registry::prepare_cpu(&op, input, context)?;
                let selection = *operation.selection();
                operation.execute().map(|image| (image, selection))
            },
        )
        .map(Some)
    }
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn paste_pipeline_was_added(before: &Image, after: &Image) -> bool {
    match (before, after) {
        (
            Image::Pipeline {
                materialized: before,
                ..
            },
            Image::Pipeline {
                materialized: after,
                ..
            },
        ) => !Arc::ptr_eq(before, after),
        (_, Image::Pipeline { .. }) => true,
        _ => false,
    }
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn last_pipeline_op(image: &Image) -> Option<PipelineOp> {
    match image {
        Image::Pipeline { ops, .. } => ops.as_slice().last().cloned(),
        _ => None,
    }
}

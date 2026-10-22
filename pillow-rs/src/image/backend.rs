//! Explicit backend selection and recursive pipeline locking for [`Image`].

use std::sync::Arc;

use crate::pipeline::PipelineOp;

use super::{Image, PipelineOps, materialization_cache};

impl Image {
    /// Locks this image pipeline to one compute backend.
    ///
    /// The backend choice is applied when the image is materialized. Non-pipeline
    /// images are returned unchanged because there is no deferred work to route.
    pub fn use_backend(mut self, b: crate::compute::Backend) -> Image {
        self.lock_backend_recursive(b);
        self
    }

    fn lock_backend_recursive(&mut self, b: crate::compute::Backend) {
        let Image::Pipeline {
            source,
            ops,
            backend,
            materialized,
            ..
        } = self
        else {
            return;
        };

        Arc::make_mut(source).lock_backend_recursive(b);
        let mut flattened_ops = ops.to_vec();
        for op in &mut flattened_ops {
            Self::lock_op_backend_recursive(op, b);
        }
        *ops = PipelineOps::from_vec(flattened_ops);
        *backend = Some(b);
        // A previously inspected/materialized pipeline must execute again
        // under the newly forced backend, including nested pipeline nodes.
        *materialized = materialization_cache();
    }

    fn lock_op_backend_recursive(op: &mut PipelineOp, b: crate::compute::Backend) {
        match op {
            PipelineOp::Add { other, .. }
            | PipelineOp::Subtract { other, .. }
            | PipelineOp::Multiply { other }
            | PipelineOp::Screen { other }
            | PipelineOp::Darker { other }
            | PipelineOp::Lighter { other }
            | PipelineOp::Difference { other }
            | PipelineOp::Overlay { other }
            | PipelineOp::HardLight { other }
            | PipelineOp::SoftLight { other }
            | PipelineOp::AddModulo { other }
            | PipelineOp::SubtractModulo { other }
            | PipelineOp::LogicalAnd { other }
            | PipelineOp::LogicalOr { other }
            | PipelineOp::LogicalXor { other }
            | PipelineOp::BlendModule { other, .. } => {
                Arc::make_mut(other).lock_backend_recursive(b);
            }
            PipelineOp::CompositeModule { other, mask, .. } => {
                Arc::make_mut(other).lock_backend_recursive(b);
                Arc::make_mut(mask).lock_backend_recursive(b);
            }
            PipelineOp::Paste { source, mask, .. } => {
                Arc::make_mut(source).lock_backend_recursive(b);
                if let Some(mask) = mask {
                    Arc::make_mut(mask).lock_backend_recursive(b);
                }
            }
            PipelineOp::AlphaComposite { source, .. } => {
                Arc::make_mut(source).lock_backend_recursive(b);
            }
            PipelineOp::Merge { bands, .. } => {
                for band in Arc::make_mut(bands).iter_mut() {
                    band.lock_backend_recursive(b);
                }
            }
            PipelineOp::Autocontrast {
                mask: Some(mask), ..
            }
            | PipelineOp::EqualizeMasked { mask } => {
                Arc::make_mut(mask).lock_backend_recursive(b);
            }
            _ => {}
        }
    }

    /// Returns the backend explicitly locked on this pipeline, if any.
    pub fn backend(&self) -> Option<crate::compute::Backend> {
        match self {
            Image::Pipeline { backend, .. } => *backend,
            _ => None,
        }
    }
}

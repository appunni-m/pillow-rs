//! Explicit backend selection and recursive pipeline locking for [`Image`].

use std::sync::Arc;

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

use super::{Image, PipelineOps, image_mode_name, materialization_cache};

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

impl Image {
    /// Try applying ImageOps.grayscale immediately for the bounded native-RGB
    /// CPU/SIMD path. Other inputs keep the ordinary lazy pipeline semantics.
    pub(crate) fn try_grayscale_eager_rgb(&self) -> Result<Option<Image>, PilError> {
        if cfg!(feature = "parallel") {
            return Ok(None);
        }
        let Image::Loaded(data) = self else {
            return Ok(None);
        };
        let logical_mode = data
            .explicit_mode
            .as_deref()
            .unwrap_or_else(|| image_mode_name(data.decoded_mode));
        if logical_mode != "RGB" {
            return Ok(None);
        }

        let image = self.materialized_shared()?;
        if !matches!(image.as_ref(), DynamicImage::ImageRgb8(_)) {
            return Ok(None);
        }
        let dims =
            crate::checked_dims::CheckedDims::new_allow_empty(image.width(), image.height(), 3)?;
        let pixels = dims.total_pixels();
        if pixels == 0 || pixels > 256 || image.as_bytes().len() != dims.total_bytes() {
            return Ok(None);
        }

        let Some((backend, _)) = crate::compute::preferred_active_backend()? else {
            return Ok(None);
        };
        let operation = PipelineOp::Grayscale;
        let grayscale = match backend {
            crate::compute::Backend::Cpu => crate::compute::execute_eager_cpu_image_operation(
                "Grayscale",
                image.as_ref(),
                false,
                || crate::compute::registry::execute_cpu(&operation, image.as_ref(), Some("RGB")),
            )?,
            crate::compute::Backend::Simd => crate::compute::execute_eager_simd_image_operation(
                "Grayscale",
                image.as_ref(),
                || {
                    crate::compute::simd_grayscale_with_evidence(
                        image.as_ref(),
                        &operation,
                        Some("RGB"),
                    )
                },
            )?,
            crate::compute::Backend::Gpu => return Ok(None),
        };

        let mut result = Image::from_dynamic(grayscale, None);
        if let Image::Loaded(data) = &mut result {
            data.info = self.image_info();
            data.exif = self.exif_metadata();
        }
        Ok(Some(result))
    }
}

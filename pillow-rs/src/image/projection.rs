//! Pillow-compatible horizontal and vertical image projections.

use crate::compute::{Backend, PipelineOperationTelemetryCapture};
use crate::error::PilError;
use crate::image::PipelineOps;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

use super::Image;

type ProjectionAxes = (Vec<u32>, Vec<u32>);

mod putpixel_pipeline;

impl Image {
    /// Returns horizontal and vertical non-zero pixel projections.
    ///
    /// The first vector has one entry per column and the second has one entry
    /// per row. Entries are `1` when any source sample on that axis is
    /// non-zero, otherwise `0`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or mode-specific projection
    /// fails.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub fn getprojection(&self) -> Result<ProjectionAxes, PilError> {
        if let Some(projection) = self.try_getprojection_putpixel_pipeline()? {
            return Ok(projection);
        }
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
        operation_receipts.capture();
        if matches!(self, Image::Loaded(_) | Image::Paletted(_))
            && let Some(projection) = self.try_getprojection_eager_backend(image.as_ref())?
        {
            return Ok(projection);
        }
        let (selected_backend, only_active_backend, context) =
            self.cpu_arch_execution_route_advisory_with_policy();
        let requested_backend = only_active_backend.then_some(selected_backend).flatten();
        let fallback_reason = selected_backend
            .filter(|backend| *backend == Backend::Gpu)
            .map(|_| {
                format!(
                    "GetProjection has no GPU strategy for this materialized route; used CPU {:?}",
                    context.architecture()
                )
            });
        crate::compute::execute_eager_cpu_getprojection_with_receipts(
            image.as_ref(),
            &context,
            requested_backend,
            fallback_reason,
            operation_receipts,
        )
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn try_getprojection_eager_backend(
        &self,
        image: &DynamicImage,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        let (selected_backend, only_active_backend, cpu_context) =
            self.cpu_arch_execution_route_with_policy()?;
        let Some(backend) = selected_backend else {
            return Ok(None);
        };
        let requested = only_active_backend.then_some(backend);
        match backend {
            Backend::Cpu | Backend::Simd => crate::compute::execute_eager_cpu_getprojection(
                image,
                &cpu_context,
                requested,
                None,
            )
            .map(Some),
            #[cfg(feature = "gpu")]
            Backend::Gpu if image.mode() == Mode::ImageRgb8 => {
                if crate::compute::gpu_rgb_projection_supported(image) {
                    crate::compute::execute_eager_gpu_rgb_getprojection(image).map(Some)
                } else {
                    crate::compute::execute_eager_cpu_getprojection(
                        image,
                        &cpu_context,
                        requested,
                        Some(format!(
                            "GPU GetProjection is unsupported for this RGB image; used CPU {:?}",
                            cpu_context.architecture()
                        )),
                    )
                    .map(Some)
                }
            }
            Backend::Gpu => crate::compute::execute_eager_cpu_getprojection(
                image,
                &cpu_context,
                requested,
                Some(format!(
                    "GPU GetProjection is unsupported for mode {:?}; used CPU {:?}",
                    image.mode(),
                    cpu_context.architecture()
                )),
            )
            .map(Some),
        }
    }
}

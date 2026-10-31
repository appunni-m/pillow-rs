//! Pillow-compatible horizontal and vertical image projections.

use crate::compute::{Backend, ExecutionContext};
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
    pub fn getprojection(&self) -> Result<ProjectionAxes, PilError> {
        if let Some(projection) = self.try_getprojection_putpixel_pipeline()? {
            return Ok(projection);
        }
        let image = self.materialized_shared()?;
        if matches!(self, Image::Loaded(_) | Image::Paletted(_))
            && let Some(projection) = self.try_getprojection_eager_backend(image.as_ref())?
        {
            return Ok(projection);
        }
        let selected = crate::compute::preferred_active_backend().ok().flatten();
        let requested_backend = selected
            .filter(|(_, only_active_backend)| *only_active_backend)
            .map(|(backend, _)| backend);
        let fallback_reason = requested_backend
            .filter(|backend| *backend != Backend::Cpu)
            .map(|backend| {
                format!(
                    "GetProjection materialized pipeline used CPU Default while {backend:?} was requested"
                )
            });
        let context = ExecutionContext::for_routed_backend(Backend::Cpu);
        crate::compute::execute_eager_cpu_getprojection(
            image.as_ref(),
            &context,
            requested_backend,
            fallback_reason,
        )
    }

    fn try_getprojection_eager_backend(
        &self,
        image: &DynamicImage,
    ) -> Result<Option<ProjectionAxes>, PilError> {
        let Some((backend, only_active_backend)) = crate::compute::preferred_active_backend()?
        else {
            return Ok(None);
        };
        let requested = only_active_backend.then_some(backend);
        match backend {
            Backend::Cpu | Backend::Simd => {
                let context = ExecutionContext::for_routed_backend(backend);
                crate::compute::execute_eager_cpu_getprojection(
                    image,
                    &context,
                    requested,
                    None,
                )
                .map(Some)
            }
            #[cfg(feature = "gpu")]
            Backend::Gpu if image.mode() == Mode::ImageRgb8 => {
                if crate::compute::gpu_rgb_projection_supported(image) {
                    crate::compute::execute_eager_gpu_rgb_getprojection(image).map(Some)
                } else {
                    let context = ExecutionContext::for_routed_backend(Backend::Cpu);
                    crate::compute::execute_eager_cpu_getprojection(
                        image,
                        &context,
                        requested,
                        Some("GPU GetProjection is unsupported for this RGB image".into()),
                    )
                    .map(Some)
                }
            }
            Backend::Gpu => {
                let context = ExecutionContext::for_routed_backend(Backend::Cpu);
                crate::compute::execute_eager_cpu_getprojection(
                    image,
                    &context,
                    requested,
                    Some(format!(
                        "GPU GetProjection is unsupported for mode {:?}",
                        image.mode()
                    )),
                )
                .map(Some)
            }
        }
    }
}

#[cfg(test)]
#[path = "projection/getprojection_cmyk_tests.rs"]
mod getprojection_cmyk_tests;

#[cfg(test)]
#[path = "projection/getprojection_rgb_pipeline_tests.rs"]
mod getprojection_rgb_pipeline_tests;

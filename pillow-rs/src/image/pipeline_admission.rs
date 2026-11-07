//! Admission checks for operations that may fuse across lazy image pipelines.
//!
//! CPU mode/architecture contracts own PutPixel/thumbnail fusion admission.
//! The wrapper resolves its backend compatibility request and injects that
//! context before asking the operation family whether it can fuse.

use super::Image;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;

impl Image {
    /// Whether this CPU or compatibility-SIMD RGB pipeline contains only public,
    /// already validated pixel writes over an in-memory source. `thumbnail()`
    /// can keep those writes in the same backend batch when that backend has a
    /// matching fusion.
    pub(crate) fn has_deferable_rgb_putpixel_prefix(&self) -> bool {
        let Image::Pipeline {
            source,
            ops,
            backend,
            mode,
            ..
        } = self
        else {
            return false;
        };
        let backend_is_unambiguous = match backend {
            Some(crate::compute::Backend::Cpu | crate::compute::Backend::Simd) => true,
            None => {
                crate::compute::cpu_is_only_active_backend()
                    || crate::compute::cpu_architecture_alias_is_only_active_backend()
            }
            Some(crate::compute::Backend::Gpu) => false,
        };
        if !backend_is_unambiguous {
            return false;
        }
        if !matches!(
            super::declared_pipeline_mode(mode),
            None | Some(crate::raster::Mode::ImageRgb8)
        )
            || ops.is_empty()
        {
            return false;
        }
        let Image::Loaded(source) = source.as_ref() else {
            return false;
        };
        if !matches!(source.image.as_ref(), DynamicImage::ImageRgb8(_)) {
            return false;
        }
        ops.iter().all(|op| {
            matches!(
                op,
                PipelineOp::PutPixel {
                    palette_index: false,
                    ..
                }
            )
        })
    }

    /// Check the CPU architecture strategy's fusion admission for this lazy
    /// write prefix and requested thumbnail.
    pub(crate) fn can_defer_rgb_putpixel_thumbnail(
        &self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
        reducing_gap: Option<f64>,
    ) -> bool {
        if !self.has_deferable_rgb_putpixel_prefix() {
            return false;
        }
        let Image::Pipeline {
            source,
            ops,
            mode: _,
            ..
        } = self
        else {
            return false;
        };
        let (requested_backend, strict_request, context) =
            match self.cpu_arch_execution_route_with_policy() {
                Ok(route) => route,
                Err(_) => return false,
            };
        if !strict_request {
            return false;
        }
        let Some(selected_backend) = requested_backend else {
            return false;
        };
        if !matches!(
            selected_backend,
            crate::compute::Backend::Cpu | crate::compute::Backend::Simd
        ) {
            return false;
        }
        let Image::Loaded(source) = source.as_ref() else {
            return false;
        };
        let thumbnail = PipelineOp::Thumbnail {
            w: width,
            h: height,
            filter,
            reducing_gap,
        };
        match selected_backend {
            crate::compute::Backend::Cpu | crate::compute::Backend::Simd => {
                crate::compute::pool_cpu::ops::thumbnail::ops::rgb_putpixel_fusion_supported(
                    source.image.as_ref(),
                    ops.as_slice(),
                    &thumbnail,
                    &context,
                )
            }
            crate::compute::Backend::Gpu => false,
        }
    }
}

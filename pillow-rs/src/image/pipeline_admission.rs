//! Admission checks for operations that may fuse across lazy image pipelines.

use super::Image;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;

impl Image {
    /// Whether this CPU- or SIMD-locked RGB pipeline contains only public,
    /// already validated pixel writes over an in-memory source. `thumbnail()`
    /// can keep those writes in the same backend batch when that backend has a
    /// matching fusion.
    pub(crate) fn has_deferable_rgb_putpixel_prefix(&self) -> bool {
        let Image::Pipeline {
            source,
            ops,
            backend,
            explicit_mode,
            ..
        } = self
        else {
            return false;
        };
        let backend_is_unambiguous = match backend {
            Some(crate::compute::Backend::Cpu | crate::compute::Backend::Simd) => true,
            None => {
                crate::compute::cpu_is_only_active_backend()
                    || crate::compute::simd_is_only_active_backend()
            }
            Some(crate::compute::Backend::Gpu) => false,
        };
        if !backend_is_unambiguous {
            return false;
        }
        if !matches!(*explicit_mode, None | Some(crate::raster::Mode::ImageRgb8))
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

    /// Check the concrete CPU or SIMD fusion admission for this lazy write
    /// prefix and requested thumbnail.
    pub(crate) fn can_defer_rgb_putpixel_thumbnail(
        &self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
    ) -> bool {
        if !self.has_deferable_rgb_putpixel_prefix() {
            return false;
        }
        let Image::Pipeline {
            source,
            ops,
            backend,
            explicit_mode,
            ..
        } = self
        else {
            return false;
        };
        let selected_backend = match backend {
            Some(crate::compute::Backend::Cpu) => Some(crate::compute::Backend::Cpu),
            Some(crate::compute::Backend::Simd) => Some(crate::compute::Backend::Simd),
            Some(crate::compute::Backend::Gpu) => None,
            None if crate::compute::cpu_is_only_active_backend() => {
                Some(crate::compute::Backend::Cpu)
            }
            None if crate::compute::simd_is_only_active_backend() => {
                Some(crate::compute::Backend::Simd)
            }
            None => None,
        };
        let Some(selected_backend) = selected_backend else {
            return false;
        };
        let Image::Loaded(source) = source.as_ref() else {
            return false;
        };
        let thumbnail = PipelineOp::Thumbnail {
            w: width,
            h: height,
            filter,
        };
        match selected_backend {
            crate::compute::Backend::Cpu => {
                crate::compute::cpu_rgb_putpixel_thumbnail_fusion_supported(
                    source.image.as_ref(),
                    ops.as_slice(),
                    &thumbnail,
                    super::mode_boundary::pillow_mode_name_opt(*explicit_mode),
                )
            }
            crate::compute::Backend::Simd => {
                crate::compute::simd_rgb_putpixel_thumbnail_fusion_supported(
                    source.image.as_ref(),
                    ops.as_slice(),
                    &thumbnail,
                    super::mode_boundary::pillow_mode_name_opt(*explicit_mode),
                )
            }
            crate::compute::Backend::Gpu => false,
        }
    }
}

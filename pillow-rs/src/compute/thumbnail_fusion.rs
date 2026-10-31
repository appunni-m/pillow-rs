//! Backend-specific admission for fused PutPixel and thumbnail operations.
//!
//! TODO(pillow-rs#3ptb): replace the separate SIMD adapter admission below
//! with the CPU thumbnail family's prepared mode/architecture fusion contract.

use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use crate::compute::ExecutionContext;

/// Ask the CPU execution path whether an RGB PutPixel prefix can be folded
/// into a non-nearest 2×2 thumbnail reduction without materializing the prefix.
pub(crate) fn cpu_rgb_putpixel_thumbnail_fusion_supported(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<Mode>,
    context: &ExecutionContext,
) -> bool {
    super::pool_cpu::ops::thumbnail::ops::rgb_putpixel_fusion_supported(
        img,
        putpixel_ops,
        thumbnail,
        mode,
        context,
    )
}

/// Ask the SIMD execution path whether an RGB PutPixel prefix can be folded
/// into a non-nearest 2×2 thumbnail reduction without materializing the prefix.
#[inline(always)]
pub(crate) fn simd_rgb_putpixel_thumbnail_fusion_supported(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    super::pool_simd::ops::adapters::simd_rgb_putpixel_thumbnail_fusion_supported(
        img,
        putpixel_ops,
        thumbnail,
        mode,
    )
}

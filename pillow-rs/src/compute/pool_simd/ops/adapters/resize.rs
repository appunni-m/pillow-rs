//! SIMD Resize family dispatch and mode-specific kernels.

#[path = "resize/kernels/i32.rs"]
mod i32_mode;

pub(super) use self::i32_mode::{
    build_resize_i_horizontal_plan, resize_i_horizontal_values, simd_resize_i32,
    simd_resize_i32_boxed,
};

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

/// Execute a native SIMD resize through the shared family adapter.
pub(crate) fn simd_resize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    super::simd_resize_impl(img, op, mode, true, 0, false)
}

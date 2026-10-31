//! Scalar FMA implementation for ImageF32 Resize on x86 AVX/FMA targets.

use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::ops::pil_resize::X86FmaToken;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

pub(crate) fn execute(
    context: &ExecutionContext,
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
    architecture: Architecture,
) -> Result<RgbaImage, PilError> {
    let fma = X86FmaToken::from_context(context, architecture).ok_or_else(|| {
        PilError::InternalError(
            "ImageF32 AVX FMA strategy received an ineligible execution context".into(),
        )
    })?;
    super::super::default::resize_impl::execute_with_fma(
        input, width, height, filter, bounds, &fma,
    )
}

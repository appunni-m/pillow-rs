//! x86 FMA streamed 2x2 ImageI32 reducing-gap implementation.

use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::ops::pil_resize::X86FmaToken;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

pub(crate) fn execute(
    context: &ExecutionContext,
    input: &RgbaImage,
    output: (u32, u32),
    filter: ResampleFilter,
    architecture: Architecture,
) -> Result<RgbaImage, PilError> {
    let fma = X86FmaToken::from_context(context, architecture).ok_or_else(|| {
        PilError::InternalError(
            "ImageI32 Thumbnail AVX FMA received an ineligible execution context".into(),
        )
    })?;
    super::super::helpers::streaming::execute(input, output, filter, &fma)
}

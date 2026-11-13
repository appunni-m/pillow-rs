//! x86 FMA streamed 2x2 ImageI32 reducing-gap implementation.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::ops::pil_resize::X86FmaToken;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    context: &ExecutionContext,
    input: &RgbaImage,
    output: (u32, u32),
    filter: ResampleFilter,
    selection: &StrategySelection,
) -> Result<RgbaImage, PilError> {
    let architecture = selection.implementation.architecture;
    let fma = X86FmaToken::from_context(context, architecture).ok_or_else(|| {
        PilError::InternalError(
            "ImageI32 Thumbnail AVX FMA received an ineligible execution context".into(),
        )
    })?;
    super::super::helpers::streaming::execute(input, output, filter, &fma)
}

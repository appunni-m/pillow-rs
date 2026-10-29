//! Request assessment for Effects mode strategies.

use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, PaletteKnowledge, RequestFacts,
    StrategyGuard, StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn facts<P: Pixel>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    request: RequestFacts,
) -> StrategyInputs {
    StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: image.as_raw().len().checked_mul(std::mem::size_of::<P::Subpixel>()),
        palette: PaletteKnowledge::Absent,
        request,
    }
}

pub(crate) fn select(
    support: ModeSupport,
    input: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.assess(ctx, input)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Effects strategy assessment remained unresolved after preparation".into(),
        )),
    }
}

pub(crate) const fn vector_architectures_for(
    operation: OperationId,
    mode: crate::raster::Mode,
    guard: StrategyGuard,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
    ]
}

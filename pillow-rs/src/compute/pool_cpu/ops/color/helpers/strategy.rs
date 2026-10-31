//! Shared mode-strategy assessment for native palette remapping.

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OperationInputs, StrategyGuard,
    StrategySelection, SupportAssessment,
};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) const fn palette_remap_architectures(
    mode: Mode,
) -> [ImplementationSupport; 3] {
    let operation = OperationId::RemapPalette;
    let guard = StrategyGuard::NativePointwiseBytes { channels: 1 };
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

pub(crate) fn select(
    support: ModeSupport,
    input: OperationInputs<'_>,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.assess(ctx, input.facts())? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "RemapPalette strategy remained unresolved after raster materialization".into(),
        )),
    }
}

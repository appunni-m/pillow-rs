//! Shared assessment for the mode-owned Eval strategy declarations.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, PaletteKnowledge, RequestFacts,
    StrategyGuard, StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::compute::Architecture;
use crate::raster::Mode;
use crate::error::PilError;

pub(crate) fn facts(
    width: u32,
    height: u32,
    byte_len: usize,
    lut_entries: usize,
) -> StrategyInputs {
    StrategyInputs {
        width,
        height,
        byte_len: Some(byte_len),
        palette: PaletteKnowledge::Absent,
        request: RequestFacts::Unspecified,
    }
    .with_eval_lut_entries(lut_entries)
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
            "Eval strategy assessment remained unresolved after materialization".into(),
        )),
    }
}

pub(crate) const fn cpu_architectures_for(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Eval,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
            StrategyGuard::NativePointwiseBytes { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Eval,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::NativePointwiseBytes { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Eval,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::NativePointwiseBytes { channels },
        ),
    ]
}

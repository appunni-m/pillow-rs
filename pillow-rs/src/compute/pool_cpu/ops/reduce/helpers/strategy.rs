//! Shared selection mechanics for Reduce mode-owned architecture strategies.

use crate::compute::{
    Architecture,
    ExecutionContext,
    registry::support::{
        ImplementationSupport, ModeSupport, OperationId, StrategyGuard, StrategyInputs,
        StrategySelection, SupportAssessment,
    },
};
use crate::error::PilError;

pub(crate) fn select_mode(
    support: ModeSupport,
    input: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.assess(ctx, input)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Reduce strategy selection requires materialized source facts".into(),
        )),
    }
}

/// Declares portable wide-lane and AVX2 reducers with one request contract.
pub(crate) const fn native_byte_architectures(
    operation: OperationId,
    mode: crate::raster::Mode,
    channels: u8,
) -> [ImplementationSupport; 4] {
    let guard = StrategyGuard::ReduceNativeByteVector { channels };
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx2,
            cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            &[Architecture::Avx2],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2",
                not(target_feature = "avx2")
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

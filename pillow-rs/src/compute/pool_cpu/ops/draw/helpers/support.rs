//! Shared architecture declarations for exact-mode Draw strategies.

use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, RejectionKind, StrategyGuard,
};
use crate::raster::Mode;

pub(crate) type VectorArchitectures = [ImplementationSupport; 3];

pub(in crate::compute::pool_cpu::ops::draw) const fn vector_architectures(
    mode: Mode,
) -> VectorArchitectures {
    [
        ImplementationSupport::cpu(
            OperationId::Draw,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
            StrategyGuard::DrawNativeSpans,
        ),
        ImplementationSupport::cpu(
            OperationId::Draw,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::DrawNativeSpans,
        ),
        ImplementationSupport::cpu(
            OperationId::Draw,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::DrawNativeSpans,
        ),
    ]
}

pub(crate) const fn vector_support(
    mode: Mode,
    architectures: &'static VectorArchitectures,
) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::Draw,
        mode,
        OutputModeRule::Preserve,
        crate::compute::pool_cpu::ops::draw::ops::DRAW_INPUTS,
        architectures,
    )
}

pub(crate) const fn serial_support(mode: Mode) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::Draw,
        mode,
        OutputModeRule::Preserve,
        crate::compute::pool_cpu::ops::draw::ops::DRAW_INPUTS,
        &[],
    )
}

pub(crate) const fn unsupported_support(mode: Mode) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason: "unsupported image mode",
        kind: RejectionKind::ValueError,
    }
}

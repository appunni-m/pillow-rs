//! Shared architecture declarations for exact-mode Draw strategies.

use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, RejectionKind, StrategyGuard,
};
use crate::raster::Mode;

pub(crate) type VectorArchitectures = [ImplementationSupport; 4];

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::draw) const fn vector_architectures(
    mode: Mode,
) -> VectorArchitectures {
    let rgb_mode = matches!(mode, Mode::ImageRgb8);
    [
        ImplementationSupport::cpu(
            OperationId::Draw,
            mode,
            Architecture::Avx2,
            rgb_mode
                && cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "avx2"
                )),
            &[Architecture::Avx2],
            StrategyGuard::DrawNativeSpans,
        ),
        ImplementationSupport::cpu(
            OperationId::Draw,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )) && !(rgb_mode && cfg!(target_feature = "avx2")),
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) const fn serial_support(mode: Mode) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::Draw,
        mode,
        OutputModeRule::Preserve,
        crate::compute::pool_cpu::ops::draw::ops::DRAW_INPUTS,
        &[],
    )
}

// TODO(pillow-rs#3ptb): define native ink semantics and expected outputs before
// adding Draw strategies for the five internal high-depth modes; these explicit
// unsupported routes are not Pillow parity failures.
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) const fn unsupported_support(mode: Mode) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason: "unsupported image mode",
        kind: RejectionKind::ValueError,
    }
}

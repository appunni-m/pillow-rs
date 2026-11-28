//! Shared selection mechanics for Reduce mode-owned architecture strategies.

use crate::compute::{
    Architecture, ExecutionContext,
    registry::support::{
        ImplementationSupport, ModeSupport, OperationId, StrategyGuard, StrategyInputs,
        StrategySelection, SupportAssessment,
    },
};
use crate::error::PilError;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
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

/// Declares the AVX-512F signed-I32 reducer when compiled for x86.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) const fn native_i32_architectures(
    operation: OperationId,
    mode: crate::raster::Mode,
) -> [ImplementationSupport; 1] {
    [ImplementationSupport::cpu(
        operation,
        mode,
        Architecture::Avx512F,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        &[Architecture::Avx512F],
        StrategyGuard::ReduceI32Vector { lanes: 16 },
    )]
}

/// Declares exact 2x2 AVX-512F byte reduction and native-byte lower strategies.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) const fn native_byte_avx512f_architectures(
    operation: OperationId,
    mode: crate::raster::Mode,
    channels: u8,
) -> [ImplementationSupport; 5] {
    let avx512 = StrategyGuard::ReduceAvx512Byte2x2 { channels };
    let lower = StrategyGuard::ReduceNativeByteVector { channels };
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx512F,
            cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            &[Architecture::Avx512F],
            avx512,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx2,
            cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            &[Architecture::Avx2],
            lower,
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
            lower,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            lower,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            lower,
        ),
    ]
}

/// Declares one exact 2x2 AVX-512F four-byte reduction strategy.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) const fn native_byte_avx512f_architecture(
    operation: OperationId,
    mode: crate::raster::Mode,
    channels: u8,
) -> ImplementationSupport {
    ImplementationSupport::cpu(
        operation,
        mode,
        Architecture::Avx512F,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        &[Architecture::Avx512F],
        StrategyGuard::ReduceAvx512Byte2x2 { channels },
    )
}

/// Declares F32 reducers for architectures with the native float kernels.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) const fn native_f32_architectures(
    operation: OperationId,
    mode: crate::raster::Mode,
) -> [ImplementationSupport; 5] {
    let guard = StrategyGuard::ReduceF32Vector { lanes: 8 };
    let avx512_guard = StrategyGuard::ReduceF32Vector { lanes: 16 };
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx512F,
            cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            &[Architecture::Avx512F],
            avx512_guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx"
            )),
            &[Architecture::Avx],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2",
                not(target_feature = "avx")
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

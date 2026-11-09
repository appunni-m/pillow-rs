//! Shared preparation for exact-mode native enhancement strategies.

use crate::compute::Architecture;
use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, StrategyGuard, StrategyInputs,
    StrategySelection, SupportAssessment,
};
use crate::error::PilError;

pub(crate) const fn byte_architectures(
    operation: OperationId,
    mode: crate::raster::Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    let guard = StrategyGuard::BrightnessVector { channels };
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

pub(crate) const fn contrast_architectures(
    operation: OperationId,
    mode: crate::raster::Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    let guard = StrategyGuard::ContrastNativeBytesVectorizable { channels };
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

pub(crate) const fn color_saturation_architectures(
    operation: OperationId,
    mode: crate::raster::Mode,
    channels: u8,
) -> [ImplementationSupport; 2] {
    let guard = StrategyGuard::ColorSaturationVector { channels };
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::AvxFma,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx",
                target_feature = "fma"
            )),
            &[Architecture::AvxFma],
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
    ]
}

pub(crate) const fn sharpness_architectures(
    operation: OperationId,
    mode: crate::raster::Mode,
    channels: u8,
) -> [ImplementationSupport; 7] {
    let guard = StrategyGuard::SharpnessNativeBytesVectorizable { channels };
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx2Fma,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx2",
                target_feature = "fma"
            )),
            &[Architecture::Avx2Fma],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx2",
                not(target_feature = "fma")
            )),
            &[Architecture::Avx2],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::AvxFma,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx",
                target_feature = "fma",
                not(target_feature = "avx2")
            )),
            &[Architecture::AvxFma],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx",
                not(target_feature = "avx2"),
                not(target_feature = "fma")
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

pub(crate) fn select(
    support: ModeSupport,
    facts: StrategyInputs,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.assess(context, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "enhancement support remained unresolved after native input preparation".into(),
        )),
    }
}

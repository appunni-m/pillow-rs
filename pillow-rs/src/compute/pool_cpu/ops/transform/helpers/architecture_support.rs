//! Architecture declarations shared by exact-mode Transform strategies.

use crate::compute::Architecture;
use crate::compute::registry::support::{ImplementationSupport, OperationId, StrategyGuard};
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) const fn architectures(mode: Mode, channels: u8) -> [ImplementationSupport; 5] {
    for_mode(mode, channels, false)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) const fn affine_bilinear_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 7] {
    let x86_fma_target = cfg!(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "avx",
        target_feature = "fma"
    ));
    let standard = for_mode(mode, channels, !x86_fma_target);
    let fma_guard = StrategyGuard::TransformByteKernels {
        channels,
        affine_nearest: true,
        affine_bilinear: true,
        projective_nearest: true,
    };
    [
        standard[0],
        standard[1],
        standard[2],
        standard[3],
        standard[4],
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::AvxFma,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx",
                target_feature = "fma"
            )),
            &[Architecture::AvxFma],
            fma_guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::Avx2Fma,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx2",
                target_feature = "fma"
            )),
            &[Architecture::Avx2Fma],
            fma_guard,
        ),
    ]
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) const fn i16_affine_architectures(mode: Mode) -> [ImplementationSupport; 7] {
    let guard = StrategyGuard::TransformI16Affine {
        nearest: true,
        bilinear: true,
    };
    [
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::Avx2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx2",
                not(all(target_feature = "avx", target_feature = "fma"))
            )),
            &[Architecture::Avx2],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::Avx,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx",
                not(target_feature = "fma")
            )),
            &[Architecture::Avx],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2",
                not(target_feature = "avx"),
                not(all(target_feature = "avx", target_feature = "fma"))
            )),
            &[Architecture::Sse2],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Transform,
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
            OperationId::Transform,
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
    ]
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
const fn for_mode(mode: Mode, channels: u8, affine_bilinear: bool) -> [ImplementationSupport; 5] {
    let guard = StrategyGuard::TransformByteKernels {
        channels,
        affine_nearest: true,
        affine_bilinear,
        projective_nearest: true,
    };
    [
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::Avx2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx2"
            )),
            &[Architecture::Avx2],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Transform,
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
            OperationId::Transform,
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
            OperationId::Transform,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Transform,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
    ]
}

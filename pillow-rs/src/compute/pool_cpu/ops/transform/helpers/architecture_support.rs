//! Architecture declarations shared by exact-mode Transform strategies.

use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, OperationId, StrategyGuard,
};
use crate::raster::Mode;

pub(crate) const fn architectures(mode: Mode, channels: u8) -> [ImplementationSupport; 5] {
    for_mode(mode, channels, false)
}

pub(crate) const fn affine_bilinear_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 5] {
    for_mode(mode, channels, true)
}

pub(crate) const fn i16_affine_architectures(mode: Mode) -> [ImplementationSupport; 5] {
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

const fn for_mode(
    mode: Mode,
    channels: u8,
    affine_bilinear: bool,
) -> [ImplementationSupport; 5] {
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

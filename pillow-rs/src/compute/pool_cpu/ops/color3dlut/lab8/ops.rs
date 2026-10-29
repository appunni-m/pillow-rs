//! Exact LAB Color3DLut support, including its core-only native strategy.

use crate::compute::{
    Architecture, ExecutionContext,
    registry::support::{
        ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

// NO_PILLOW_SUPPORT: Color3DLut applies the table to native LAB channel values;
// Pillow defines source-mode behavior only for RGB, RGBA, and CMYK.
const SOURCE_MODE: Mode = Mode::ImageLab8;
#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "avx2")))]
const SSE2_SUPPORT: ImplementationSupport = ImplementationSupport::cpu(
    OperationId::Color3DLut,
    SOURCE_MODE,
    Architecture::Sse2,
    true,
    &[Architecture::Sse2],
    StrategyGuard::Color3DLutVector { source_channels: 3 },
);

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2"))]
const AVX2_SUPPORT: ImplementationSupport = ImplementationSupport::cpu(
    OperationId::Color3DLut,
    SOURCE_MODE,
    Architecture::Avx2,
    true,
    &[Architecture::Avx2],
    StrategyGuard::Color3DLutVector { source_channels: 3 },
);

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
const NEON_SUPPORT: ImplementationSupport = ImplementationSupport::cpu(
    OperationId::Color3DLut,
    SOURCE_MODE,
    Architecture::Neon,
    true,
    &[Architecture::Neon],
    StrategyGuard::Color3DLutVector { source_channels: 3 },
);

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
const WASM_SIMD128_SUPPORT: ImplementationSupport = ImplementationSupport::cpu(
    OperationId::Color3DLut,
    SOURCE_MODE,
    Architecture::WasmSimd128,
    true,
    &[Architecture::WasmSimd128],
    StrategyGuard::Color3DLutVector { source_channels: 3 },
);

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "avx2")))]
const ARCHITECTURES: &[ImplementationSupport] = &[SSE2_SUPPORT];
#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2"))]
const ARCHITECTURES: &[ImplementationSupport] = &[AVX2_SUPPORT];
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
const ARCHITECTURES: &[ImplementationSupport] = &[NEON_SUPPORT];
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
const ARCHITECTURES: &[ImplementationSupport] = &[WASM_SIMD128_SUPPORT];
#[cfg(not(any(
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "avx2")),
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2"),
    all(target_arch = "aarch64", target_feature = "neon"),
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
const ARCHITECTURES: &[ImplementationSupport] = &[];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Color3DLut,
    SOURCE_MODE,
    OutputModeRule::ByTargetMode,
    &super::super::ops::TARGET_INPUTS,
    ARCHITECTURES,
    "the core applies the lookup table to native LAB channel values without a mode conversion",
);

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    size: (u32, u32, u32),
    table: &'a [f64],
    channels: u32,
    target_mode: Mode,
    ctx: &ExecutionContext,
) -> Result<super::super::helpers::strategy::Strategy<'a>, PilError> {
    super::super::helpers::strategy::Strategy::prepare(
        image, SOURCE_MODE, size, table, channels, target_mode, SUPPORT, ctx,
    )
}

pub(crate) fn execute(
    prepared: super::super::helpers::strategy::Strategy<'_>,
) -> Result<DynamicImage, PilError> {
    let (image, size, table, channels, target_mode, selection) = prepared.into_parts();
    match selection.implementation.architecture {
        Architecture::Default => super::default::color3dlut_impl::execute(
            image, size, table, channels, target_mode,
        ),
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "avx2")))]
        Architecture::Sse2 => super::sse2::color3dlut_impl::execute(
            image, size, table, channels, target_mode,
        ),
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2"))]
        Architecture::Avx2 => super::avx2::color3dlut_impl::execute(
            image, size, table, channels, target_mode,
        ),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => super::neon::color3dlut_impl::execute(
            image, size, table, channels, target_mode,
        ),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => super::wasm_simd128::color3dlut_impl::execute(
            image, size, table, channels, target_mode,
        ),
        _ => Err(PilError::InternalError(
            "selected Color3DLut architecture has no LAB implementation".into(),
        )),
    }
}


//! Luma source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageLuma8
);

use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
        StrategyGuard, StrategySelection,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));
const WASM_SIMD128_AVAILABLE: bool = cfg!(all(target_arch = "wasm32", target_feature = "simd128"));

static ARCHITECTURES: [ImplementationSupport; 39] = [
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgba8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgba8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageCmyk8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageCmyk8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageYCbCr8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageYCbCr8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageHsv8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageHsv8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageI32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageI32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageF32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageF32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageLumaA8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageLumaA8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16L),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16L),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16B),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16B),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16N),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16N),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgba8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageCmyk8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageYCbCr8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageHsv8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageI32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageF32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageLumaA8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16L),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16B),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageI16N),
    ),
];

// LAB reads native L samples and writes final LAB output in this mode strategy.
const CORE_ONLY_TARGETS: &[Mode] = &[];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: super::super::ops::LUMA_TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageLuma8,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn execute(
    source: &GrayImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        #[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "sse2")))]
        Architecture::Sse2 => super::sse2::convert_impl::execute(source, target),
        #[cfg(target_arch = "aarch64")]
        Architecture::Neon => super::neon::convert_impl::execute(source, target),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => super::wasm_simd128::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared Luma8 architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &GrayImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute_palette_target(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared Luma8 palette architecture".into(),
        )),
    }
}

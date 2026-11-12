//! Luma-alpha source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::helpers::convert::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageLumaA8
);

use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, InputContract, ModeSupport, OperationId, StrategyGuard,
        StrategySelection,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));
const WASM_SIMD128_AVAILABLE: bool = cfg!(all(target_arch = "wasm32", target_feature = "simd128"));

const fn sse2(guard: StrategyGuard) -> ImplementationSupport {
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLumaA8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        guard,
    )
}

const fn neon(guard: StrategyGuard) -> ImplementationSupport {
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLumaA8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        guard,
    )
}

const fn wasm_simd128(guard: StrategyGuard) -> ImplementationSupport {
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLumaA8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        guard,
    )
}

static ARCHITECTURES: [ImplementationSupport; 30] = [
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageLuma8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageLuma8)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(
        Mode::ImageLaPremultiplied8,
    )),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(
        Mode::ImageLaPremultiplied8,
    )),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageRgb8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageRgb8)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageRgba8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageRgba8)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageRgbx8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageRgbx8)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageCmyk8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageCmyk8)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageYCbCr8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageYCbCr8)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageHsv8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageHsv8)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageI32)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageI32)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageF32)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageF32)),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageLuma8)),
    wasm_simd128(StrategyGuard::ConvertTargetModeNonEmpty(
        Mode::ImageLaPremultiplied8,
    )),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageRgb8)),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageRgba8)),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageRgbx8)),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageCmyk8)),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageYCbCr8)),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageHsv8)),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageI32)),
    wasm_simd128(StrategyGuard::ConvertTargetMode(Mode::ImageF32)),
];

// Pillow's wrapper routes these missing direct LA converters through L.
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: super::super::ops::LA_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageLumaA8,
    crate::compute::registry::support::OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn execute(
    source: &GrayAlphaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    let mut output = match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        Architecture::Sse2 => super::sse2::convert_impl::execute(source, target),
        Architecture::Neon => super::neon::convert_impl::execute(source, target),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => super::wasm_simd128::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LA architecture".into(),
        )),
    }?;
    super::super::helpers::convert::physical_lanes::preserve_luma_alpha_lane(source, target, &mut output)?;
    Ok(output)
}

pub(crate) fn execute_palette_target(
    source: &GrayAlphaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LA palette architecture".into(),
        )),
    }
}

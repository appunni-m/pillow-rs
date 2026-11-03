//! CMYK8 source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageCmyk8
);

use crate::compute::{Architecture, registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategyGuard, StrategySelection,
}};
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, Mode, RgbaImage};

const TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageLab8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
// TODO(pillow-rs#3ptb): Pillow's wrapper also converts CMYK to LAB through
// RGBA and ImageCms; reconcile all CoreOnly targets with wrapper fallbacks.
const CORE_ONLY_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageYCbCr8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: Some("listed CMYK target routes need Pillow-wrapper parity review"),
    rejection_reason: "image has wrong mode",
}];

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));
const WASM_SIMD128_AVAILABLE: bool = cfg!(all(target_arch = "wasm32", target_feature = "simd128"));

static ARCHITECTURES: [ImplementationSupport; 4] = [
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageCmyk8,
        Architecture::Ssse3,
        cfg!(target_arch = "x86_64"),
        &[Architecture::Sse2, Architecture::Ssse3],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageCmyk8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageCmyk8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageCmyk8,
        Architecture::WasmSimd128,
        WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageCmyk8,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &ARCHITECTURES,
);

/// Executes the CMYK8 strategy selected from its exact support declaration.
///
/// # Safety
///
/// `selection` must come from [`SUPPORT`] for this source and target, with the
/// selected architecture's requirements checked by the same execution context.
pub(crate) unsafe fn execute(
    source: &RgbaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match (selection.implementation.architecture, target) {
        (Architecture::Default, _) => super::default::convert_impl::execute(source, target),
        #[cfg(target_arch = "x86_64")]
        (Architecture::Ssse3, Mode::ImageRgb8) => super::ssse3::convert_impl::execute(source),
        #[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "sse2")))]
        (Architecture::Sse2, Mode::ImageRgb8) => super::sse2::convert_impl::execute(source),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        (Architecture::Neon, Mode::ImageRgb8) => super::neon::convert_impl::execute(source),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        (Architecture::WasmSimd128, Mode::ImageRgb8) => {
            super::wasm_simd128::convert_impl::execute(source)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared CMYK8 architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &RgbaImage,
    target: Mode,
    dither: Option<DitherMethod>,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target, dither)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared CMYK palette architecture".into(),
        )),
    }
}

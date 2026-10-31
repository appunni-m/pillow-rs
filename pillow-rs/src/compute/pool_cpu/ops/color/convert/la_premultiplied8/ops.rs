//! La source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageLaPremultiplied8
);

use crate::compute::{Architecture, registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));

static ARCHITECTURES: [ImplementationSupport; 3] = [
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLaPremultiplied8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLumaA8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLaPremultiplied8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLumaA8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLaPremultiplied8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLumaA8),
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageLaPremultiplied8,
    OutputModeRule::ByTargetMode,
    &super::super::ops::LA_PREMULTIPLIED_INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn execute(
    source: &GrayAlphaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        #[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "sse2")))]
        Architecture::Sse2 => super::sse2::convert_impl::execute(source, target),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => super::neon::convert_impl::execute(source, target),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => super::wasm_simd128::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared La architecture".into(),
        )),
    }
}

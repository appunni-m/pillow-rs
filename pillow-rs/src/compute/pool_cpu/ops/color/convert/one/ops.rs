//! Bilevel source-mode support and Default Convert strategy.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::One
);

use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
        StrategySelection,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));

macro_rules! architecture_targets {
    ($architecture:ident, $available:expr, $requirements:expr; $($target:ident),+ $(,)?) => {
        $(ImplementationSupport::cpu(
            OperationId::Convert,
            Mode::One,
            Architecture::$architecture,
            $available,
            $requirements,
            StrategyGuard::ConvertTargetMode(Mode::$target),
        ),)+
    };
}

static ARCHITECTURES: [ImplementationSupport; 12] = [
    architecture_targets!(
        Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2];
        ImageI16,
        ImageI16L,
        ImageI16B,
        ImageI16N,
    ),
    architecture_targets!(
        Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon];
        ImageI16,
        ImageI16L,
        ImageI16B,
        ImageI16N,
    ),
    architecture_targets!(
        WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128];
        ImageI16,
        ImageI16L,
        ImageI16B,
        ImageI16N,
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::One,
    OutputModeRule::ByTargetModeWithPalette,
    &super::super::ops::ONE_INPUTS,
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
        Architecture::Sse2 if is_luma16_target(target) => {
            super::sse2::convert_impl::execute(source, target)
        }
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon if is_luma16_target(target) => {
            super::neon::convert_impl::execute(source, target)
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 if is_luma16_target(target) => {
            super::wasm_simd128::convert_impl::execute(source, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared One architecture".into(),
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
            "Convert selected an undeclared One palette architecture".into(),
        )),
    }
}

fn is_luma16_target(target: Mode) -> bool {
    matches!(
        target,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    )
}

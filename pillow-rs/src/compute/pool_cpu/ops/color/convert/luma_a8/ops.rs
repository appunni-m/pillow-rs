//! Luma-alpha source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageLumaA8
);

use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, ModeSupport, OperationId, StrategyGuard, StrategySelection,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));

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

static ARCHITECTURES: [ImplementationSupport; 22] = [
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLuma8)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLuma8)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLumaA8)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLumaA8)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageLaPremultiplied8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageLaPremultiplied8)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgb8)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgb8)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgba8)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgba8)),
    sse2(StrategyGuard::ConvertTargetMode(Mode::ImageRgbx8)),
    neon(StrategyGuard::ConvertTargetMode(Mode::ImageRgbx8)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageCmyk8)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageCmyk8)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageYCbCr8)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageYCbCr8)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageHsv8)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageHsv8)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageI32)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageI32)),
    sse2(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageF32)),
    neon(StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageF32)),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageLumaA8,
    crate::compute::registry::support::OutputModeRule::ByTargetModeWithPalette,
    &super::super::ops::LA_INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn execute(
    source: &GrayAlphaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        Architecture::Sse2 => super::sse2::convert_impl::execute(source, target),
        Architecture::Neon => super::neon::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LA architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &GrayAlphaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute_palette_target(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LA palette architecture".into(),
        )),
    }
}


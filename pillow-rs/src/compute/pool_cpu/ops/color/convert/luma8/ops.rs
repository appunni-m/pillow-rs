//! Luma source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageLuma8
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

static ARCHITECTURES: [ImplementationSupport; 24] = [
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgba8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgba8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageCmyk8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageCmyk8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageYCbCr8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageYCbCr8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageHsv8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageHsv8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageI32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageI32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageF32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageF32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLumaA8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLumaA8),
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
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageLuma8,
    OutputModeRule::ByTargetModeWithPalette,
    &super::super::ops::LUMA_INPUTS,
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

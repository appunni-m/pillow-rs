//! CMYK8 source-mode support and architecture selection for Convert.

use crate::compute::{Architecture, registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategyGuard, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

const TARGETS: &[Mode] = &[
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));

static ARCHITECTURES: [ImplementationSupport; 3] = [
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
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageCmyk8,
    OutputModeRule::ByTargetMode,
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
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared CMYK8 architecture".into(),
        )),
    }
}

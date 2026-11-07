//! I;16L source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageI16L
);

use crate::compute::{Architecture, registry::support::{
    InputContract, ModeSupport, OperationId, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

const CORE_ONLY_TARGETS: &[Mode] = &[];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: super::super::ops::I16_TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageI16L,
    crate::compute::registry::support::OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &ARCHITECTURES,
);

static ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 24] =
    super::super::helpers::luma16::architecture_support(Mode::ImageI16L);

pub(crate) fn execute(
    source: &Samples,
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
            "Convert selected an undeclared I;16L architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &Samples,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::super::helpers::luma16::execute_palette_target(source, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared I;16L palette architecture".into(),
        )),
    }
}

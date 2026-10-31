//! I;16N source-mode support and architecture selection for Convert.

use crate::compute::{Architecture, registry::support::{
    ModeSupport, OperationId, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageI16N,
    crate::compute::registry::support::OutputModeRule::ByTargetMode,
    &super::super::ops::OTHER_INPUTS,
    &ARCHITECTURES,
);

static ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 21] =
    super::super::helpers::luma16::architecture_support(Mode::ImageI16N);

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
            "Convert selected an undeclared I;16N architecture".into(),
        )),
    }
}

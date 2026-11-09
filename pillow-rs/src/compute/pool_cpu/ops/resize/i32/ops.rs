//! Exact ImageI32 mode dispatch for Resize.
//!
//! Filtered requests use declared AVX/FMA or NEON strategies when eligible.
//! Ordinary unboxed nearest can use NEON; boxed nearest remains scalar.

use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode};

static RESIZE_ARCHITECTURES: [ImplementationSupport; 2] = [
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageI32,
        Architecture::AvxFma,
        cfg!(target_arch = "x86_64"),
        &[Architecture::AvxFma],
        StrategyGuard::ResizeFilteredFma,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageI32,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ResizeI32Neon,
    ),
];
static RESIZE_BOXED_ARCHITECTURES: [ImplementationSupport; 1] = [ImplementationSupport::cpu(
    OperationId::ResizeBoxed,
    Mode::ImageI32,
    Architecture::AvxFma,
    cfg!(target_arch = "x86_64"),
    &[Architecture::AvxFma],
    StrategyGuard::ResizeFilteredFma,
)];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Resize,
    Mode::ImageI32,
    OutputModeRule::Preserve,
    super::super::helpers::support::INPUTS,
    &RESIZE_ARCHITECTURES,
);
pub(crate) const BOXED_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::ResizeBoxed,
    Mode::ImageI32,
    OutputModeRule::Preserve,
    super::super::helpers::support::INPUTS,
    &RESIZE_BOXED_ARCHITECTURES,
);

pub(crate) fn resize(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let architecture = selection.implementation.architecture;
    let DynamicImage::ImageI32(input) = image else {
        return Err(PilError::InternalError(
            "ImageI32 Resize strategy received another raster mode".into(),
        ));
    };
    if let Some(box_bounds) = bounds
        && matches!(filter, ResampleFilter::Nearest)
    {
        return Ok(DynamicImage::ImageI32(
            super::default::nearest::execute_boxed(input, (width, height), box_bounds)?,
        ));
    }
    let output = match architecture {
        Architecture::Default => {
            super::default::resize_impl::execute(input, width, height, filter, bounds)?
        }
        #[cfg(target_arch = "x86_64")]
        Architecture::AvxFma => super::avx::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        )?,
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => super::neon::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        )?,
        _ => {
            return Err(PilError::InternalError(format!(
                "ImageI32 Resize received non-I32 implementation selection {architecture:?}"
            )));
        }
    };
    Ok(DynamicImage::ImageI32(output))
}

//! Exact ImageI32 Thumbnail support and architecture dispatch.

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode};

const ARCHITECTURES: &[ImplementationSupport] = &[ImplementationSupport::cpu(
    OperationId::Thumbnail,
    Mode::ImageI32,
    Architecture::AvxFma,
    cfg!(target_arch = "x86_64"),
    &[Architecture::AvxFma],
    StrategyGuard::ThumbnailI32Stream2x2,
)];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Thumbnail,
    Mode::ImageI32,
    OutputModeRule::Preserve,
    super::super::ops::INPUTS,
    ARCHITECTURES,
);

pub(in crate::compute::pool_cpu::ops::thumbnail) fn streams_two_by_two(
    source: (u32, u32),
    output: (u32, u32),
    filter: ResampleFilter,
) -> bool {
    crate::compute::helpers::resampling::thumbnail::uses_even_two_by_two_reduction(
        source, output, filter,
    )
}

pub(crate) fn execute(
    image: &DynamicImage,
    output: (u32, u32),
    filter: ResampleFilter,
    context: &ExecutionContext,
    architecture: Architecture,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI32(input) = image else {
        return Err(PilError::InternalError(
            "ImageI32 Thumbnail strategy received another raster mode".into(),
        ));
    };
    let output_image = match architecture {
        Architecture::Default => {
            super::default::thumbnail_impl::execute(input, output, filter)
        }
        #[cfg(target_arch = "x86_64")]
        Architecture::AvxFma => super::avx::thumbnail_impl::execute(
            context,
            input,
            output,
            filter,
            Architecture::AvxFma,
        ),
        _ => Err(PilError::InternalError(format!(
            "ImageI32 Thumbnail received non-I32 implementation selection {architecture:?}"
        ))),
    }?;
    Ok(DynamicImage::ImageI32(output_image))
}

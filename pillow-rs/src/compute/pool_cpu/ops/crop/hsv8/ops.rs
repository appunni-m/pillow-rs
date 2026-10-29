//! Exact hsv8 crop semantics and its declared architecture strategy.

use crate::compute::{ExecutionContext, registry::support::{ModeSupport, OperationId, StrategyInputs, PaletteKnowledge, SupportAssessment}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::default;
use super::super::helpers::crop::CropRegion;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;

pub(crate) const SUPPORT: ModeSupport =
    ModeSupport::serial(OperationId::Crop, Mode::ImageHsv8, default::crop_impl::INPUTS);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(default::crop_impl::Crop<'a>),
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn assess(region: CropRegion, ctx: &ExecutionContext) -> Result<SupportAssessment, PilError> {
    SUPPORT.assess(ctx, StrategyInputs { width: region.width, height: region.height, byte_len: None, palette: PaletteKnowledge::Unknown, request: Default::default() })
}

pub(crate) fn prepare<'a>(
    image: &'a default::crop_impl::Input,
    region: CropRegion,
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let implementation = default::crop_impl::prepare(image, region)?;
    let selection = SUPPORT.select_serial(ctx)?;
    Ok(Prepared::new(ArchitectureStrategy::Default(implementation), selection))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
            ArchitectureStrategy::Default(implementation) => default::crop_impl::execute(implementation),
    }
}

//! Exact ycbcr8 crop semantics and its declared architecture strategy.

use crate::compute::{ExecutionContext, registry::support::{
    ModeSupport,
    OperationId,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::super::helpers::crop::CropRegion;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;

pub(crate) const SUPPORT: ModeSupport =
    ModeSupport::serial(OperationId::Crop, Mode::ImageYCbCr8, super::default::crop_impl::INPUTS);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(super::default::crop_impl::Crop<'a>),
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;


pub(crate) fn prepare<'a>(
    image: &'a super::default::crop_impl::Input,
    region: CropRegion,
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let implementation = super::default::crop_impl::prepare(image, region)?;
    let selection = SUPPORT.select_serial(ctx)?;
    Ok(Prepared::new(ArchitectureStrategy::Default(implementation), selection))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
            ArchitectureStrategy::Default(implementation) => super::default::crop_impl::execute(implementation),
    }
}

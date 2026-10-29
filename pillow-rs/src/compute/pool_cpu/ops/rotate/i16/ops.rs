//! Exact ImageI16 Rotate support and serial strategy.

use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::pool_cpu::ops::rotate::helpers::strategy;
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OutputModeRule,
};
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>;

pub(crate) struct Rotate<'a> {
    input: &'a Input,
    request: RotateRequest,
}

pub(crate) type Strategy<'a> = Prepared<Rotate<'a>>;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Rotate,
    Mode::ImageI16,
    OutputModeRule::Preserve,
    super::super::ops::ROTATE_INPUTS,
    &[],
);

pub(crate) fn prepare<'a>(
    input: &'a Input,
    request: RotateRequest,
    context: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let facts = strategy::facts(input, request);
    let selection = strategy::select(SUPPORT, facts, context)?;
    Ok(Prepared::new(Rotate { input, request }, selection))
}

pub(crate) fn execute(plan: Strategy<'_>) -> Result<DynamicImage, PilError> {
    let image = super::default::rotate_impl::execute(plan.implementation.input, plan.implementation.request)?;
    Ok(DynamicImage::ImageI16(image))
}


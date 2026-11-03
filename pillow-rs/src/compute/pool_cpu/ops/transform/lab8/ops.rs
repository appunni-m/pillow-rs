//! Exact ImageLab8 Transform support and Default strategy.

use crate::compute::pool_cpu::ops::transform::helpers::strategy::{self, Strategy};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::compute::registry::support::{ModeSupport, OperationId, OutputModeRule};
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Transform,
    Mode::ImageLab8,
    OutputModeRule::Preserve,
    super::super::ops::TRANSFORM_INPUTS,
    &[],
);

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    request: TransformRequest<'a>,
    context: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    strategy::prepare(SUPPORT, image, request, context)
}

pub(crate) fn execute(plan: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match plan.selection().implementation.architecture {
        crate::compute::Architecture::Default => super::default::transform_impl::execute(
            plan.implementation.image,
            plan.implementation.request,
        ),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Transform ImageLab8"
        ))),
    }
}

//! Exact ImageI16 Transform support and typed affine strategy.

use crate::compute::pool_cpu::ops::transform::helpers::{
    architecture_support, strategy::{self, Strategy},
};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::compute::registry::support::{ModeSupport, OperationId, OutputModeRule};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

static ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] =
    architecture_support::i16_affine_architectures(Mode::ImageI16);

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Transform,
    Mode::ImageI16,
    OutputModeRule::Preserve,
    super::super::ops::TRANSFORM_INPUTS,
    &ARCHITECTURES,
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
        Architecture::Default => super::default::transform_impl::execute(
            plan.implementation.image,
            plan.implementation.request,
        ),
        Architecture::Avx2 => avx2::transform_impl::execute(
            plan.implementation.image,
            plan.implementation.request,
        ),
        Architecture::Avx => avx::transform_impl::execute(
            plan.implementation.image,
            plan.implementation.request,
        ),
        Architecture::Sse2 => sse2::transform_impl::execute(
            plan.implementation.image,
            plan.implementation.request,
        ),
        Architecture::Neon => neon::transform_impl::execute(
            plan.implementation.image,
            plan.implementation.request,
        ),
        Architecture::WasmSimd128 => wasm_simd128::transform_impl::execute(
            plan.implementation.image,
            plan.implementation.request,
        ),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Transform ImageI16"
        ))),
    }
}

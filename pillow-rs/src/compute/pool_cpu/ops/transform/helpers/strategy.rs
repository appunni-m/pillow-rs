//! Shared preparation for mode-owned Transform strategies.

use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    ModeSupport, PaletteKnowledge, StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::DynamicImage;

use super::super::ops::TransformRequest;

pub(crate) struct TransformPlan<'a> {
    pub(crate) image: &'a DynamicImage,
    pub(crate) request: TransformRequest<'a>,
}

pub(crate) type Strategy<'a> = Prepared<TransformPlan<'a>>;

pub(crate) fn prepare<'a>(
    support: ModeSupport,
    image: &'a DynamicImage,
    request: TransformRequest<'a>,
    context: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let selection = match support.assess(context, facts(image, request))? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "Transform preparation requires materialized input facts".into(),
            ));
        }
    };
    Ok(Prepared::new(TransformPlan { image, request }, selection))
}

fn facts(image: &DynamicImage, request: TransformRequest<'_>) -> StrategyInputs {
    StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_bytes().len()),
        palette: PaletteKnowledge::Absent,
        request: request.facts(),
    }
}

pub(crate) fn selection<'a>(plan: &'a Strategy<'_>) -> &'a StrategySelection {
    plan.selection()
}

pub(crate) type ExecuteInto = for<'a> fn(
    &DynamicImage,
    TransformRequest<'a>,
    &mut DynamicImage,
) -> Result<(), PilError>;

pub(crate) fn execute_into(
    plan: Strategy<'_>,
    output: &mut DynamicImage,
    default: ExecuteInto,
    architecture_specific: Option<ExecuteInto>,
) -> Result<(), PilError> {
    let architecture = plan.selection().implementation.architecture;
    let TransformPlan { image, request } = plan.implementation;
    match architecture {
        Architecture::Default => default(image, request, output),
        Architecture::Avx2
        | Architecture::Avx
        | Architecture::Sse2
        | Architecture::Neon
        | Architecture::WasmSimd128 => architecture_specific
            .ok_or_else(|| {
                PilError::InternalError(format!(
                    "Transform {architecture:?} has no direct destination strategy"
                ))
            })?(image, request, output),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Transform"
        ))),
    }
}

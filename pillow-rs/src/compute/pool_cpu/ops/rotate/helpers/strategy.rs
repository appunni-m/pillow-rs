//! Input facts and selection shared by Rotate mode declarations.

use crate::compute::registry::support::{
    ModeSupport, PaletteKnowledge, RequestFacts, StrategyInputs, StrategySelection,
    SupportAssessment,
};
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};

use super::super::ops::RotateRequest;

pub(crate) fn facts<P: Pixel>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    request: RotateRequest,
) -> StrategyInputs {
    StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: image.as_raw().len().checked_mul(std::mem::size_of::<P::Subpixel>()),
        palette: PaletteKnowledge::Absent,
        request: request.facts(),
    }
}

pub(crate) fn select(
    support: ModeSupport,
    input: StrategyInputs,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.assess(context, input)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Rotate strategy assessment remained unresolved after preparation".into(),
        )),
    }
}

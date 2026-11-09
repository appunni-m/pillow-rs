use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::compute::{Architecture, Backend};
use crate::draw::{FloodFillAccess, FloodFillError};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) fn execute<A: FloodFillAccess>(
    mode: Mode,
    selection: &StrategySelection,
    access: &mut A,
    seed: A::Point,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    threshold: &A::Threshold,
) -> Result<(), FloodFillError<A::Error>> {
    validate_selection(mode, selection).map_err(FloodFillError::Strategy)?;
    crate::draw::floodfill::run(access, seed, value, border, threshold)
        .map_err(FloodFillError::Callback)
}

fn validate_selection(mode: Mode, selection: &StrategySelection) -> Result<(), PilError> {
    let implementation = selection.implementation;
    if implementation.operation == OperationId::FloodFill
        && implementation.mode == mode
        && implementation.backend == Backend::Cpu
        && implementation.architecture == Architecture::Default
        && selection.requested_backend == Backend::Cpu
    {
        return Ok(());
    }
    Err(PilError::InternalError(
        "FloodFill received a mismatched CPU Default strategy selection".into(),
    ))
}

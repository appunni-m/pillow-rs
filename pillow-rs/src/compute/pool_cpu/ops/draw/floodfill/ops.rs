//! FloodFill support declaration, selection, and Default strategy orchestration.

use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationSupport, OutputModeRule,
};
use crate::compute::ExecutionContext;
use crate::draw::{FloodFillAccess, FloodFillError, PreparedFloodFill};
use crate::error::PilError;
use crate::raster::Mode;

const FLOODFILL_INPUTS: &[crate::compute::registry::support::InputContract] = &[
    crate::compute::registry::support::InputContract::FloodFillInputs,
];

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => pillow_strategy(mode),
        Mode::ImageLuma8 => pillow_strategy(mode),
        Mode::ImageLumaA8 => pillow_strategy(mode),
        Mode::ImageLaPremultiplied8 => pillow_strategy(mode),
        Mode::ImagePalette8 => pillow_strategy(mode),
        Mode::ImagePaletteAlpha8 => pillow_strategy(mode),
        Mode::ImageRgb8 => pillow_strategy(mode),
        Mode::ImageRgba8 => pillow_strategy(mode),
        Mode::ImageRgbPremultiplied8 => pillow_strategy(mode),
        Mode::ImageRgbx8 => pillow_strategy(mode),
        Mode::ImageCmyk8 => pillow_strategy(mode),
        Mode::ImageYCbCr8 => pillow_strategy(mode),
        Mode::ImageLab8 => pillow_strategy(mode),
        Mode::ImageHsv8 => pillow_strategy(mode),
        Mode::ImageI32 => pillow_strategy(mode),
        Mode::ImageF32 => pillow_strategy(mode),
        Mode::ImageI16 => pillow_strategy(mode),
        Mode::ImageI16L => pillow_strategy(mode),
        Mode::ImageI16B => pillow_strategy(mode),
        Mode::ImageI16N => pillow_strategy(mode),
        // NO_PILLOW_SUPPORT: these internal high-depth identities are native
        // raster extensions without Pillow mode strings or a Python contract.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::FloodFill,
            mode,
            OutputModeRule::NoRasterResult,
            FLOODFILL_INPUTS,
            &[],
            "internal high-depth raster identity has no Pillow floodfill contract",
        ),
    }
}

fn pillow_strategy(mode: Mode) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::FloodFill,
        mode,
        OutputModeRule::NoRasterResult,
        FLOODFILL_INPUTS,
        &[],
    )
}

pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::FloodFill, FLOODFILL_INPUTS, classify_mode);

pub(crate) fn prepare(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<PreparedFloodFill, PilError> {
    let selection = SUPPORT.for_mode(mode).select_serial(context)?;
    Ok(PreparedFloodFill::new(mode, selection))
}

pub(crate) fn execute<A: FloodFillAccess>(
    prepared: &PreparedFloodFill,
    access: &mut A,
    seed: A::Point,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    threshold: &A::Threshold,
) -> Result<(), FloodFillError<A::Error>> {
    crate::compute::pool_cpu::ops::draw::floodfill::default::execute(
        prepared.mode(),
        prepared.selection(),
        access,
        seed,
        value,
        border,
        threshold,
    )
}


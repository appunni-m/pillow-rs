//! Color3DLut support declaration and prepared mode-strategy composition.
//!
//! TODO(pillow-rs#3ptb): retire the separate SIMD Color3DLut adapter after the
//! remaining SIMD operation families have moved to CPU architecture owners.
//! The public Image method enters this prepared plan through the registry; keep
//! that route as the only CPU execution entry point.

use crate::compute::{ExecutionContext, registry::support::{
    InputContract, ModeChannelCapacity, ModeSupport, OperationId, OperationInputs,
    OperationSupport, RejectionKind,
}};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

const SOURCE_MODE_ERROR: &str = "image has wrong mode";
pub(super) const TARGET_MODES: &[ModeChannelCapacity] = &[
    ModeChannelCapacity { mode: Mode::ImageRgb8, channels: 3 },
    ModeChannelCapacity { mode: Mode::ImageRgba8, channels: 4 },
    ModeChannelCapacity { mode: Mode::ImageCmyk8, channels: 4 },
];
pub(super) const TARGET_INPUTS: [InputContract; 1] = [InputContract::Color3DLutTarget {
    targets: TARGET_MODES,
}];

pub(crate) type PreparedColor3DLut<'a> = super::helpers::strategy::Strategy<'a>;

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::SUPPORT,
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::Rejected {
            mode,
            reason: SOURCE_MODE_ERROR,
            kind: RejectionKind::ValueError,
        },
    }
}

pub(crate) static SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Color3DLut,
    &[],
    classify_mode,
);

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    size: (u32, u32, u32),
    table: &'a [f64],
    channels: u32,
    target_mode: Mode,
    ctx: &ExecutionContext,
) -> Result<PreparedColor3DLut<'a>, PilError> {
    match image.mode() {
        Mode::ImageRgb8 => super::rgb8::ops::prepare(
            image, size, table, channels, target_mode, ctx,
        ),
        Mode::ImageRgba8 => super::rgba8::ops::prepare(
            image, size, table, channels, target_mode, ctx,
        ),
        Mode::ImageCmyk8 => super::cmyk8::ops::prepare(
            image, size, table, channels, target_mode, ctx,
        ),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::prepare(
            image, size, table, channels, target_mode, ctx,
        ),
        Mode::ImageLab8 => super::lab8::ops::prepare(
            image, size, table, channels, target_mode, ctx,
        ),
        Mode::ImageHsv8 => super::hsv8::ops::prepare(
            image, size, table, channels, target_mode, ctx,
        ),
        mode => match classify_mode(mode) {
            ModeSupport::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
            ModeSupport::Supported { .. } => unreachable!("supported Color3DLut mode missed dispatch"),
        },
    }
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Color3DLut {
        size,
        table,
        channels,
        source_mode,
        target_mode,
    } = op else {
        return Err(PilError::ValueError("expected Color3DLut op".into()));
    };
    if *source_mode != input.image.mode() {
        return Err(PilError::InternalError(
            "Color3DLut source mode did not match the materialized image".into(),
        ));
    }
    let plan = prepare(input.image, *size, table, *channels, *target_mode, ctx)?;
    Ok(crate::compute::registry::PreparedOperation::from_color3dlut(plan))
}

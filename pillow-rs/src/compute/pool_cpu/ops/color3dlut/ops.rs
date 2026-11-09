//! Color3DLut support declaration and prepared mode-strategy composition.
//!
//! TODO(pillow-rs#3ptb): expand parity over every declared source/target Mode,
//! channel count, and available Architecture, including the eight-pixel
//! Default fallback, core-only native source modes, and rejected LUT shapes.
//! Source comparison with the archived CPU/SIMD adapters is complete. The
//! public Image method enters this plan through the registry as its CPU
//! execution entry point.
//! Pillow's three-channel LUT copies the fourth physical byte through when the
//! target is RGB; Default and architecture paths preserve it on compact output.

use crate::compute::{
    ExecutionContext,
    registry::support::{
        InputContract, ModeChannelCapacity, ModeSupport, OperationId, OperationInputs,
        OperationSupport, OutputModeRule, RejectionKind,
    },
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

const SOURCE_MODE_ERROR: &str = "image has wrong mode";
const PALETTE_MODE_ERROR: &str = "cannot filter palette images";
pub(super) const TARGET_MODES: &[ModeChannelCapacity] = &[
    ModeChannelCapacity {
        mode: Mode::ImageRgb8,
        channels: 3,
    },
    ModeChannelCapacity {
        mode: Mode::ImageRgba8,
        channels: 4,
    },
    ModeChannelCapacity {
        mode: Mode::ImageCmyk8,
        channels: 4,
    },
];
pub(super) const TARGET_INPUTS: [InputContract; 1] = [InputContract::Color3DLutTarget {
    targets: TARGET_MODES,
}];

pub(crate) type PreparedColor3DLut<'a> = super::helpers::strategy::Strategy<'a>;

#[derive(Clone, Copy)]
enum Color3DLutModeStrategy {
    Rgb8,
    Rgba8,
    Cmyk8,
    Ycbcr8,
    Lab8,
    Hsv8,
}

impl Color3DLutModeStrategy {
    fn for_mode(mode: Mode) -> Option<Self> {
        match mode {
            Mode::ImageRgb8 => Some(Self::Rgb8),
            Mode::ImageRgba8 => Some(Self::Rgba8),
            Mode::ImageCmyk8 => Some(Self::Cmyk8),
            Mode::ImageYCbCr8 => Some(Self::Ycbcr8),
            Mode::ImageLab8 => Some(Self::Lab8),
            Mode::ImageHsv8 => Some(Self::Hsv8),
            _ => None,
        }
    }

    fn support(self) -> ModeSupport {
        match self {
            Self::Rgb8 => super::rgb8::ops::SUPPORT,
            Self::Rgba8 => super::rgba8::ops::SUPPORT,
            Self::Cmyk8 => super::cmyk8::ops::SUPPORT,
            Self::Ycbcr8 => super::ycbcr8::ops::SUPPORT,
            Self::Lab8 => super::lab8::ops::SUPPORT,
            Self::Hsv8 => super::hsv8::ops::SUPPORT,
        }
    }

    fn prepare<'a>(
        self,
        image: &'a DynamicImage,
        size: (u32, u32, u32),
        table: &'a [f64],
        channels: u32,
        target_mode: Mode,
        ctx: &ExecutionContext,
    ) -> Result<PreparedColor3DLut<'a>, PilError> {
        match self {
            Self::Rgb8 => super::rgb8::ops::prepare(image, size, table, channels, target_mode, ctx),
            Self::Rgba8 => {
                super::rgba8::ops::prepare(image, size, table, channels, target_mode, ctx)
            }
            Self::Cmyk8 => {
                super::cmyk8::ops::prepare(image, size, table, channels, target_mode, ctx)
            }
            Self::Ycbcr8 => {
                super::ycbcr8::ops::prepare(image, size, table, channels, target_mode, ctx)
            }
            Self::Lab8 => super::lab8::ops::prepare(image, size, table, channels, target_mode, ctx),
            Self::Hsv8 => super::hsv8::ops::prepare(image, size, table, channels, target_mode, ctx),
        }
    }
}

fn classify_mode(mode: Mode) -> ModeSupport {
    match Color3DLutModeStrategy::for_mode(mode) {
        Some(strategy) => strategy.support(),
        None if mode == Mode::ImagePalette8 => ModeSupport::Rejected {
            mode,
            reason: PALETTE_MODE_ERROR,
            kind: RejectionKind::ValueError,
        },
        None => ModeSupport::Rejected {
            mode,
            reason: SOURCE_MODE_ERROR,
            kind: RejectionKind::ValueError,
        },
    }
}

pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Color3DLut, &TARGET_INPUTS, classify_mode)
        .with_rejected_output(OutputModeRule::ByTargetMode);

pub(crate) fn prepare<'a>(
    mode: Mode,
    image: &'a DynamicImage,
    size: (u32, u32, u32),
    table: &'a [f64],
    channels: u32,
    target_mode: Mode,
    ctx: &ExecutionContext,
) -> Result<PreparedColor3DLut<'a>, PilError> {
    match Color3DLutModeStrategy::for_mode(mode) {
        Some(strategy) => strategy.prepare(image, size, table, channels, target_mode, ctx),
        None => match classify_mode(mode) {
            ModeSupport::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
            ModeSupport::Supported { .. } => {
                unreachable!("supported Color3DLut mode missed dispatch")
            }
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
    } = op
    else {
        return Err(PilError::ValueError("expected Color3DLut op".into()));
    };
    let mode = input.mode();
    if *source_mode != mode {
        return Err(PilError::InternalError(
            "Color3DLut source mode did not match the materialized image".into(),
        ));
    }
    let plan = prepare(
        mode,
        input.image,
        *size,
        table,
        *channels,
        *target_mode,
        ctx,
    )?;
    Ok(crate::compute::registry::PreparedOperation::from_color3dlut(plan))
}

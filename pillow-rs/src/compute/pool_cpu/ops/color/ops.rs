//! Color operation support declarations and prepared orchestration.

use crate::compute::{ExecutionContext, registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    OutputModeRule, RejectionKind, StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode, RgbImage};

pub(super) const RGB_INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 3,
    overflow_reason: "LAB image size overflow",
    mismatch_reason: "LAB conversion requires tightly packed RGB8 input",
}];

fn classify_convert_lab(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
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
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Complete source-mode contract for the normalized RGB-to-LAB pipeline op.
pub(crate) static SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ConvertLab,
    &[],
    classify_convert_lab,
);

/// Prepared Convert plan exported through the color-family registry facade.
pub(crate) type PreparedConvert<'a> = super::convert::ops::Prepared<'a>;

fn classify_remap_palette(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::SUPPORT,
        Mode::One
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
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
            reason: "illegal image mode",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Exact-mode contracts for the native L and P index remapping operation.
pub(crate) static REMAP_PALETTE_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::RemapPalette,
    &[],
    classify_remap_palette,
);

/// Prepared RGB-to-LAB execution with the exact mode and backend selection.
pub(crate) struct PreparedConvertLab<'a> {
    source: &'a RgbImage,
    selection: StrategySelection,
}

impl PreparedConvertLab<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        super::rgb8::default::convert_lab_impl::execute(self.source)
    }
}

/// Prepares the selected source-mode strategy using the image's variant tag.
pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    ctx: &ExecutionContext,
) -> Result<PreparedConvertLab<'a>, PilError> {
    let mode = input.mode();
    let assessment = SUPPORT.for_mode(mode).assess(ctx, input.facts())?;
    let selection = match assessment {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "ConvertLab support remained unresolved after RGB materialization".into(),
            ));
        }
    };
    let DynamicImage::ImageRgb8(source) = input.image else {
        return Err(PilError::InternalError(
            "ConvertLab support accepted a non-RGB8 image".into(),
        ));
    };
    Ok(PreparedConvertLab { source, selection })
}

/// Registry entry for ConvertLab; it preserves typed source and output modes.
pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    if !matches!(op, PipelineOp::ConvertLab { .. }) {
        return Err(PilError::ValueError("expected ConvertLab op".into()));
    }
    prepare(input, ctx).map(crate::compute::registry::PreparedOperation::from_convert_lab)
}

/// Prepared L/P palette-index remapping with the selected native strategy.
pub(crate) enum PreparedRemapPalette<'a> {
    Luma8(super::luma8::ops::Prepared<'a>),
    Palette8(super::palette8::ops::Prepared<'a>),
}

impl PreparedRemapPalette<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma8(plan) => plan.selection(),
            Self::Palette8(plan) => plan.selection(),
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self {
            Self::Luma8(plan) => plan.execute(),
            Self::Palette8(plan) => plan.execute(),
        }
    }
}

/// Selects the native L/P mode strategy for a typed remap request.
pub(crate) fn prepare_remap_palette<'a>(
    input: OperationInputs<'a>,
    dest_map: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<PreparedRemapPalette<'a>, PilError> {
    match input.image {
        DynamicImage::ImageLuma8(_) => super::luma8::ops::prepare(input, dest_map, ctx)
            .map(PreparedRemapPalette::Luma8),
        DynamicImage::ImagePalette8(_) => super::palette8::ops::prepare(input, dest_map, ctx)
            .map(PreparedRemapPalette::Palette8),
        _ => {
            let mode = input.mode();
            match REMAP_PALETTE_SUPPORT.for_mode(mode) {
                ModeSupport::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
                ModeSupport::Supported { .. } => Err(PilError::InternalError(
                    "RemapPalette support accepted a non-L/P image".into(),
                )),
            }
        }
    }
}

/// Registry entry for RemapPalette, retaining wrapper palette ownership.
pub(crate) fn prepare_remap_palette_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::RemapPalette { dest_map } = op else {
        return Err(PilError::ValueError("expected RemapPalette op".into()));
    };
    let map = &dest_map[..dest_map.len().min(256)];
    prepare_remap_palette(input, map, ctx)
        .map(crate::compute::registry::PreparedOperation::from_remap_palette)
}

//! Convert support contract and mode/architecture orchestration.

use crate::compute::{ExecutionContext, registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

const LUMA_TARGETS: &[Mode] = &[
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const LA_TARGETS: &[Mode] = &[
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const RGBA_TARGETS: &[Mode] = &[
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const RGB_PREMULTIPLIED_TARGETS: &[Mode] = &[
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const LA_PREMULTIPLIED_TARGETS: &[Mode] = &[Mode::ImageLumaA8];
const OTHER_TARGETS: &[Mode] = &[
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];

pub(crate) const LUMA_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: LUMA_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];
pub(crate) const LA_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: LA_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];
pub(crate) const RGBA_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: RGBA_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];
pub(crate) const RGB_PREMULTIPLIED_INPUTS: &[InputContract] =
    &[InputContract::ConvertTarget {
        targets: RGB_PREMULTIPLIED_TARGETS,
        core_only_targets: &[],
        core_only_reason: None,
        rejection_reason: "image has wrong mode",
    }];
pub(crate) const LA_PREMULTIPLIED_INPUTS: &[InputContract] =
    &[InputContract::ConvertTarget {
        targets: LA_PREMULTIPLIED_TARGETS,
        core_only_targets: &[],
        core_only_reason: None,
        rejection_reason: "image has wrong mode",
    }];
pub(crate) const OTHER_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: OTHER_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::SUPPORT,
        Mode::ImageI32 => super::i32::ops::SUPPORT,
        Mode::ImageF32 => super::f32::ops::SUPPORT,
        Mode::ImageI16 => super::i16::ops::SUPPORT,
        Mode::ImageI16L => super::i16l::ops::SUPPORT,
        Mode::ImageI16B => super::i16b::ops::SUPPORT,
        Mode::ImageI16N => super::i16n::ops::SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
    }
}

/// Complete source-mode and typed-target contract for Convert.
// TODO(pillow-rs#3ptb): migrate the remaining archived SIMD Convert kernels
// into their exact source-mode Architecture strategies and reconcile every
// source/target pair with the archived reachability matrix.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Convert, &[], classify_mode);

/// Compatibility name consumed by the prepared-operation registry.
pub(crate) type PreparedConvert<'a> = Prepared<'a>;

pub(crate) struct Prepared<'a> {
    input: OperationInputs<'a>,
    target: Mode,
    selection: StrategySelection,
}

impl Prepared<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.input.image {
            DynamicImage::One(source) => {
                return super::one::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageLuma8(source) => {
                return super::luma8::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageLumaA8(source) => {
                return super::luma_a8::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageRgb8(source) => {
                // SAFETY: prepare retained the exact source, target, and
                // context used to select this declared mode strategy.
                return unsafe {
                    super::rgb8::ops::execute(source, self.target, self.selection)
                };
            }
            DynamicImage::ImageRgba8(source) => {
                // SAFETY: `prepare` computed this selection from the same
                // source mode, target, and injected context retained here.
                return unsafe {
                    super::rgba8::ops::execute(source, self.target, self.selection)
                };
            }
            DynamicImage::ImageRgbPremultiplied8(source) => {
                return super::rgb_premultiplied8::ops::execute(
                    source,
                    self.target,
                    self.selection,
                );
            }
            DynamicImage::ImageRgbx8(source) => {
                // SAFETY: prepare selected this strategy from the RGBX
                // contract using the same source, target, and context retained here.
                return unsafe {
                    super::rgbx8::ops::execute(source, self.target, self.selection)
                };
            }
            DynamicImage::ImageI32(source) => {
                return super::i32::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageF32(source) => {
                return super::f32::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageI16(source) => {
                return super::i16::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageI16L(source) => {
                return super::i16l::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageI16B(source) => {
                return super::i16b::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageI16N(source) => {
                return super::i16n::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageLaPremultiplied8(source) => {
                return super::la_premultiplied8::ops::execute(
                    source,
                    self.target,
                    self.selection,
                );
            }
            DynamicImage::ImageCmyk8(source) => {
                // SAFETY: prepare selected this strategy from the CMYK
                // contract using the same source, target, and context retained here.
                return unsafe {
                    super::cmyk8::ops::execute(source, self.target, self.selection)
                };
            }
            DynamicImage::ImageYCbCr8(source) => {
                return super::ycbcr8::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageHsv8(source) => {
                return super::hsv8::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImagePalette8(source) => {
                return super::palette8::ops::execute(
                    source,
                    self.input.palette,
                    self.target,
                    self.selection,
                );
            }
            DynamicImage::ImagePaletteAlpha8(source) => {
                return super::palette_alpha8::ops::execute(
                    source,
                    self.input.palette,
                    self.target,
                    self.selection,
                );
            }
            DynamicImage::ImageLab8(source) => {
                return super::lab8::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageLumaA16(source) => {
                return super::luma_a16::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageRgb16(source) => {
                return super::rgb16::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageRgba16(source) => {
                return super::rgba16::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageRgb32F(source) => {
                return super::rgb32f::ops::execute(source, self.target, self.selection);
            }
            DynamicImage::ImageRgba32F(source) => {
                return super::rgba32f::ops::execute(source, self.target, self.selection);
            }
        }
    }
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    target: Mode,
    ctx: &ExecutionContext,
) -> Result<Prepared<'a>, PilError> {
    let support = SUPPORT.for_mode(input.mode());
    let assessment = support.assess(ctx, input.facts().with_convert_target(target))?;
    let selection = match assessment {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "Convert target support remained unresolved after typed admission".into(),
            ));
        }
    };
    Ok(Prepared { input, target, selection })
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Convert { mode, .. } = op else {
        return Err(PilError::ValueError("expected Convert op".into()));
    };
    prepare(input, *mode, ctx).map(crate::compute::registry::PreparedOperation::from_convert)
}


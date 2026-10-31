//! Convert support contract and mode/architecture orchestration.

use crate::compute::{ExecutionContext, registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::{DitherMethod, PipelineOp};
use crate::raster::{DynamicImage, Mode};

const LUMA_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
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
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
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
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
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
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
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
const I16_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
pub(crate) const RGBX_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
pub(crate) const RGBX_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: RGBX_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];
const SCALAR_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
];
const MATRIX_TARGETS: &[Mode] = &[Mode::ImageLuma8, Mode::ImageRgb8];
const MATRIX_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: MATRIX_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

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
pub(crate) const I16_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: I16_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];
pub(crate) const SCALAR_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: SCALAR_TARGETS,
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
// TODO(pillow-rs#3ptb): compare each archived SIMD Convert source/target path
// and edge guard with these mode/Architecture declarations; migrate any gaps
// and relocate the archived SIMD cases beside their maintained strategies.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Convert, &[], classify_mode);

fn classify_matrix_mode(mode: Mode) -> ModeSupport {
    if mode == Mode::ImageRgb8 {
        return ModeSupport::cpu(
            OperationId::ConvertMatrix,
            mode,
            crate::compute::registry::support::OutputModeRule::ByTargetMode,
            MATRIX_INPUTS,
            &[],
        );
    }

    // NO_PILLOW_SUPPORT: native samples preserve the existing RGB projection
    // semantics for these exact source identities.
    ModeSupport::cpu_core_only(
        OperationId::ConvertMatrix,
        mode,
        crate::compute::registry::support::OutputModeRule::ByTargetMode,
        MATRIX_INPUTS,
        &[],
        "matrix conversion from this exact source identity extends Pillow's RGB matrix contract",
    )
}

/// Matrix conversion support, with one Default strategy for each exact source mode.
pub(crate) static MATRIX_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ConvertMatrix,
    &[],
    classify_matrix_mode,
);

/// Compatibility name consumed by the prepared-operation registry.
pub(crate) type PreparedConvert<'a> = Prepared<'a>;

/// Raster output plus wrapper-owned palette data for eager palette conversion.
pub(crate) struct PreparedConvertOutput {
    pub(crate) image: DynamicImage,
    pub(crate) palette: Option<Vec<u8>>,
    pub(crate) palette_alpha: Option<Vec<u8>>,
    pub(crate) wrapper_metadata: Option<ConvertWrapperMetadata>,
}

#[derive(Clone, Copy)]
pub(crate) enum ConvertWrapperMetadata {
    PreserveSource,
    PreserveSourceLoaded,
    ExifOnly,
    Drop,
}

impl PreparedConvertOutput {
    fn raster(image: DynamicImage) -> Self {
        Self {
            image,
            palette: None,
            palette_alpha: None,
            wrapper_metadata: None,
        }
    }
}

pub(crate) struct Prepared<'a> {
    input: OperationInputs<'a>,
    target: Mode,
    matrix: Option<&'a [f64]>,
    dither: Option<DitherMethod>,
    selection: StrategySelection,
}

impl Prepared<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let output = self.execute_with_output()?;
        if output.palette.is_some()
            || output.palette_alpha.is_some()
            || output.wrapper_metadata.is_some()
        {
            return Err(PilError::InternalError(
                "palette-target Convert requires the Image wrapper output boundary".into(),
            ));
        }
        Ok(output.image)
    }

    pub(crate) fn execute_with_output(self) -> Result<PreparedConvertOutput, PilError> {
        if let Some(matrix) = self.matrix {
            return super::helpers::matrix::convert_with_matrix(
                self.input.image,
                self.target,
                matrix,
            )
            .map(PreparedConvertOutput::raster);
        }
        if self.target == Mode::One {
            return self.execute_binary().map(PreparedConvertOutput::raster);
        }
        if matches!(self.target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
            return self.execute_palette_target();
        }
        self.execute_mode().map(PreparedConvertOutput::raster)
    }

    fn execute_palette_target(self) -> Result<PreparedConvertOutput, PilError> {
        let input = self.input;
        let target = self.target;
        let dither = self.dither;
        let selection = self.selection;
        match input.image {
            DynamicImage::One(source) => {
                super::one::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLuma8(source) => {
                super::luma8::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLumaA8(source) => {
                super::luma_a8::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImagePalette8(source) => {
                super::palette8::ops::execute_palette_target(
                    source,
                    input.palette,
                    target,
                    selection,
                )
            }
            DynamicImage::ImagePaletteAlpha8(source) => {
                super::palette_alpha8::ops::execute_palette_target(
                    source,
                    target,
                    selection,
                )
            }
            DynamicImage::ImageRgb8(source) => {
                super::rgb8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageRgba8(source) => {
                super::rgba8::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageRgbPremultiplied8(source) => {
                super::rgb_premultiplied8::ops::execute_palette_target(
                    source,
                    target,
                    dither,
                    selection,
                )
            }
            DynamicImage::ImageRgbx8(source) => {
                super::rgbx8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageCmyk8(source) => {
                super::cmyk8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageYCbCr8(source) => {
                super::ycbcr8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageHsv8(source) => {
                super::hsv8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageLab8(source) => {
                super::lab8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageI32(source) => {
                super::i32::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageF32(source) => {
                super::f32::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16(source) => {
                super::i16::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16L(source) => {
                super::i16l::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16B(source) => {
                super::i16b::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16N(source) => {
                super::i16n::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLumaA16(source) => super::luma_a16::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            DynamicImage::ImageRgb16(source) => super::rgb16::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            DynamicImage::ImageRgba16(source) => super::rgba16::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            DynamicImage::ImageRgb32F(source) => super::rgb32f::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            DynamicImage::ImageRgba32F(source) => super::rgba32f::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            _ => Err(PilError::InternalError(
                "Convert selected an undeclared palette target strategy".into(),
            )),
        }
    }

    fn execute_mode(self) -> Result<DynamicImage, PilError> {
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

    fn execute_binary(self) -> Result<DynamicImage, PilError> {
        let input = self.input;
        let dither = self.dither;
        let selection = self.selection;
        match input.image {
            DynamicImage::One(_) => {
                super::one::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLuma8(_) => {
                super::luma8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLumaA8(_) => {
                super::luma_a8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLaPremultiplied8(_) => super::la_premultiplied8::ops::execute_binary(
                input,
                dither.as_ref(),
                selection,
            ),
            DynamicImage::ImagePalette8(_) => {
                super::palette8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImagePaletteAlpha8(_) => {
                super::palette_alpha8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb8(_) => {
                super::rgb8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba8(_) => {
                super::rgba8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgbPremultiplied8(_) => super::rgb_premultiplied8::ops::execute_binary(
                input,
                dither.as_ref(),
                selection,
            ),
            DynamicImage::ImageRgbx8(_) => {
                super::rgbx8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageCmyk8(_) => {
                super::cmyk8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageYCbCr8(_) => {
                super::ycbcr8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLab8(_) => {
                super::lab8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageHsv8(_) => {
                super::hsv8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI32(_) => {
                super::i32::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageF32(_) => {
                super::f32::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16(_) => {
                super::i16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16L(_) => {
                super::i16l::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16B(_) => {
                super::i16b::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16N(_) => {
                super::i16n::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLumaA16(_) => {
                super::luma_a16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb16(_) => {
                super::rgb16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba16(_) => {
                super::rgba16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb32F(_) => {
                super::rgb32f::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba32F(_) => {
                super::rgba32f::ops::execute_binary(input, dither.as_ref(), selection)
            }
        }
    }
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    target: Mode,
    dither: Option<DitherMethod>,
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
    Ok(Prepared { input, target, matrix: None, dither, selection })
}

pub(crate) fn prepare_matrix<'a>(
    input: OperationInputs<'a>,
    target: Mode,
    matrix: &'a [f64],
    ctx: &ExecutionContext,
) -> Result<Prepared<'a>, PilError> {
    let support = MATRIX_SUPPORT.for_mode(input.mode());
    let assessment = support.assess(ctx, input.facts().with_convert_target(target))?;
    let selection = match assessment {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "matrix Convert strategy required unresolved input facts".into(),
            ));
        }
    };
    Ok(Prepared { input, target, matrix: Some(matrix), dither: None, selection })
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Convert { mode, matrix, dither } = op else {
        return Err(PilError::ValueError("expected Convert op".into()));
    };
    // TODO(pillow-rs#3ptb): carry prepared palette metadata through the lazy
    // pipeline terminal before admitting palette-target Convert descriptors.
    if matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return Err(PilError::NotImplementedError(
            "palette-target Convert requires the Image wrapper output boundary".into(),
        ));
    }
    if let Some(matrix) = matrix {
        return prepare_matrix(input, *mode, matrix, ctx)
            .map(crate::compute::registry::PreparedOperation::from_convert);
    }
    prepare(input, *mode, dither.clone(), ctx)
        .map(crate::compute::registry::PreparedOperation::from_convert)
}


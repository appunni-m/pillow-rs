//! Prepared Convert execution and typed child-strategy selection.

use crate::compute::{
    ExecutionContext,
    registry::support::{
        OperationInputs, OperationRouteOutcome, StrategySelection, SupportAssessment,
    },
};
use crate::error::PilError;
use crate::pipeline::{DitherMethod, PipelineOp};
use crate::raster::{DynamicImage, Mode};

use super::super::super as color_ops;
use super::super::super::ops::{CONVERT_SUPPORT, MATRIX_SUPPORT};

/// Raster output plus wrapper-owned palette data for eager palette conversion.
pub(crate) struct PreparedConvertOutput {
    pub(crate) image: DynamicImage,
    pub(crate) palette: Option<Vec<u8>>,
    pub(crate) palette_alpha: Option<Vec<u8>>,
    pub(crate) wrapper_metadata: Option<ConvertWrapperMetadata>,
}

#[derive(Debug, Clone, Copy)]
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

    pub(crate) fn composed_selections(&self) -> &[StrategySelection] {
        &[]
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let identity = self.matrix.is_none() && self.input.mode() == self.target;
        if matches!(self.target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            && self.target != self.input.mode()
        {
            // The raster-only registry result cannot carry wrapper palette
            // state. Image-facing P/PA conversion uses execute_with_output.
            return Err(PilError::NotImplementedError(
                "palette-target Convert requires the Image wrapper output boundary".into(),
            ));
        }
        let output = self.execute_with_output()?;
        if !identity
            && (output.palette.is_some()
                || output.palette_alpha.is_some()
                || output.wrapper_metadata.is_some())
        {
            return Err(PilError::InternalError(
                "palette-target Convert requires the Image wrapper output boundary".into(),
            ));
        }
        Ok(output.image)
    }

    pub(crate) fn execute_with_output(self) -> Result<PreparedConvertOutput, PilError> {
        if self.matrix.is_none() && self.input.mode() == self.target {
            return Ok(self.identity_output());
        }
        if let Some(matrix) = self.matrix {
            return super::matrix::convert_with_matrix(self.input.image, self.target, matrix)
                .map(PreparedConvertOutput::raster);
        }
        if self.target == Mode::One {
            return self.execute_binary().map(PreparedConvertOutput::raster);
        }
        if matches!(self.target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
            return self.execute_palette_target();
        }
        Self::execute_mode(self.input, self.target, self.selection)
            .map(PreparedConvertOutput::raster)
    }

    fn identity_output(self) -> PreparedConvertOutput {
        let input = self.input;
        let image = input.image.clone();
        // NO_PILLOW_SUPPORT: this same-mode identity also admits the five
        // internal high-depth raster identities as a native core extension.
        match self.target {
            Mode::ImagePalette8 => PreparedConvertOutput {
                image,
                palette: None,
                palette_alpha: None,
                wrapper_metadata: Some(ConvertWrapperMetadata::PreserveSourceLoaded),
            },
            Mode::ImagePaletteAlpha8 => PreparedConvertOutput {
                image,
                palette: input.palette.rgb.map(|entries| entries.to_vec()),
                palette_alpha: (!input.palette.alpha.is_empty())
                    .then(|| input.palette.alpha.to_vec()),
                wrapper_metadata: Some(ConvertWrapperMetadata::PreserveSource),
            },
            _ => PreparedConvertOutput::raster(image),
        }
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn execute_palette_target(self) -> Result<PreparedConvertOutput, PilError> {
        let input = self.input;
        let target = self.target;
        let dither = self.dither;
        let selection = self.selection;
        match input.image {
            DynamicImage::One(source) => {
                color_ops::one::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLuma8(source) => {
                color_ops::luma8::convert::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLumaA8(source) => {
                color_ops::luma_a8::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImagePalette8(source) => {
                color_ops::palette8::convert::execute_palette_target(
                    source,
                    input.palette,
                    target,
                    selection,
                )
            }
            DynamicImage::ImagePaletteAlpha8(source) => {
                color_ops::palette_alpha8::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageRgb8(source) => {
                color_ops::rgb8::convert::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageRgba8(source) => {
                color_ops::rgba8::convert::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageRgbPremultiplied8(source) => {
                color_ops::rgb_premultiplied8::ops::execute_palette_target(
                    source, target, dither, selection,
                )
            }
            DynamicImage::ImageRgbx8(source) => {
                color_ops::rgbx8::convert::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageCmyk8(source) => {
                color_ops::cmyk8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageYCbCr8(source) => {
                color_ops::ycbcr8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageHsv8(source) => {
                color_ops::hsv8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageLab8(source) => {
                color_ops::lab8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageI32(source) => {
                color_ops::i32::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageF32(source) => {
                color_ops::f32::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16(source) => {
                color_ops::i16::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16L(source) => {
                color_ops::i16l::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16B(source) => {
                color_ops::i16b::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16N(source) => {
                color_ops::i16n::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLumaA16(source) => {
                color_ops::luma_a16::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageRgb16(source) => {
                color_ops::rgb16::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageRgba16(source) => {
                color_ops::rgba16::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageRgb32F(source) => {
                color_ops::rgb32f::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageRgba32F(source) => {
                color_ops::rgba32f::ops::execute_palette_target(source, target, dither, selection)
            }
            _ => Err(PilError::InternalError(
                "Convert selected an undeclared palette target strategy".into(),
            )),
        }
    }

    #[allow(
        unsafe_code,
        reason = "the retained strategy selection proves the architecture-specific Convert preconditions"
    )]
    fn execute_mode(
        input: OperationInputs<'_>,
        target: Mode,
        selection: StrategySelection,
    ) -> Result<DynamicImage, PilError> {
        match input.image {
            DynamicImage::One(source) => {
                return color_ops::one::ops::execute(source, target, selection);
            }
            DynamicImage::ImageLuma8(source) => {
                return color_ops::luma8::convert::execute(source, target, selection);
            }
            DynamicImage::ImageLumaA8(source) => {
                return color_ops::luma_a8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgb8(source) => {
                // SAFETY: prepare retained the exact source, target, and
                // context used to select this declared mode strategy.
                return unsafe { color_ops::rgb8::convert::execute(source, target, selection) };
            }
            DynamicImage::ImageRgba8(source) => {
                // SAFETY: `prepare` computed this selection from the same
                // source mode, target, and injected context retained here.
                return unsafe { color_ops::rgba8::convert::execute(source, target, selection) };
            }
            DynamicImage::ImageRgbPremultiplied8(source) => {
                return color_ops::rgb_premultiplied8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgbx8(source) => {
                // SAFETY: prepare selected this strategy from the RGBX
                // contract using the same source, target, and context retained here.
                return unsafe { color_ops::rgbx8::convert::execute(source, target, selection) };
            }
            DynamicImage::ImageI32(source) => {
                return color_ops::i32::ops::execute(source, target, selection);
            }
            DynamicImage::ImageF32(source) => {
                return color_ops::f32::ops::execute(source, target, selection);
            }
            DynamicImage::ImageI16(source) => {
                return color_ops::i16::ops::execute(source, target, selection);
            }
            DynamicImage::ImageI16L(source) => {
                return color_ops::i16l::ops::execute(source, target, selection);
            }
            DynamicImage::ImageI16B(source) => {
                return color_ops::i16b::ops::execute(source, target, selection);
            }
            DynamicImage::ImageI16N(source) => {
                return color_ops::i16n::ops::execute(source, target, selection);
            }
            DynamicImage::ImageLaPremultiplied8(source) => {
                return color_ops::la_premultiplied8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageCmyk8(source) => {
                // SAFETY: prepare selected this strategy from the CMYK
                // contract using the same source, target, and context retained here.
                return unsafe { color_ops::cmyk8::ops::execute(source, target, selection) };
            }
            DynamicImage::ImageYCbCr8(source) => {
                return color_ops::ycbcr8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageHsv8(source) => {
                return color_ops::hsv8::ops::execute(source, target, selection);
            }
            DynamicImage::ImagePalette8(source) => {
                return color_ops::palette8::convert::execute(
                    source,
                    input.palette,
                    target,
                    selection,
                );
            }
            DynamicImage::ImagePaletteAlpha8(source) => {
                return color_ops::palette_alpha8::ops::execute(
                    source,
                    input.palette,
                    target,
                    selection,
                );
            }
            DynamicImage::ImageLab8(source) => {
                return color_ops::lab8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageLumaA16(source) => {
                return color_ops::luma_a16::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgb16(source) => {
                return color_ops::rgb16::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgba16(source) => {
                return color_ops::rgba16::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgb32F(source) => {
                return color_ops::rgb32f::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgba32F(source) => {
                return color_ops::rgba32f::ops::execute(source, target, selection);
            }
        }
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn execute_binary(self) -> Result<DynamicImage, PilError> {
        let input = self.input;
        let dither = self.dither;
        let selection = self.selection;
        match input.image {
            DynamicImage::One(_) => {
                color_ops::one::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLuma8(_) => {
                color_ops::luma8::convert::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLumaA8(_) => {
                color_ops::luma_a8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLaPremultiplied8(_) => Err(PilError::ValueError(
                "conversion from La to L not supported".into(),
            )),
            DynamicImage::ImagePalette8(_) => {
                color_ops::palette8::convert::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImagePaletteAlpha8(_) => {
                color_ops::palette_alpha8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb8(_) => {
                color_ops::rgb8::convert::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba8(_) => {
                color_ops::rgba8::convert::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgbPremultiplied8(_) => {
                color_ops::rgb_premultiplied8::ops::execute_binary(
                    input,
                    dither.as_ref(),
                    selection,
                )
            }
            DynamicImage::ImageRgbx8(_) => {
                color_ops::rgbx8::convert::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageCmyk8(_) => {
                color_ops::cmyk8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageYCbCr8(_) => {
                color_ops::ycbcr8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLab8(_) => {
                color_ops::lab8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageHsv8(_) => {
                color_ops::hsv8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI32(_) => {
                color_ops::i32::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageF32(_) => {
                color_ops::f32::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16(_) => {
                color_ops::i16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16L(_) => {
                color_ops::i16l::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16B(_) => {
                color_ops::i16b::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16N(_) => {
                color_ops::i16n::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLumaA16(_) => {
                color_ops::luma_a16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb16(_) => {
                color_ops::rgb16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba16(_) => {
                color_ops::rgba16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb32F(_) => {
                color_ops::rgb32f::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba32F(_) => {
                color_ops::rgba32f::ops::execute_binary(input, dither.as_ref(), selection)
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
    if let Some(composition) = CONVERT_SUPPORT.target_composition_for(input.mode(), target) {
        if let OperationRouteOutcome::Rejects { reason, kind, .. } = composition.outcome {
            return Err(kind.into_error(reason));
        }
    }
    let support = CONVERT_SUPPORT.for_mode(input.mode());
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
    Ok(Prepared {
        input,
        target,
        matrix: None,
        dither,
        selection,
    })
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
    Ok(Prepared {
        input,
        target,
        matrix: Some(matrix),
        dither: None,
        selection,
    })
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Convert {
        mode,
        matrix,
        dither,
    } = op
    else {
        return Err(PilError::ValueError("expected Convert op".into()));
    };
    // Image-facing cross-mode P/PA conversion consumes this selected plan via
    // `execute_convert_output`, which carries the wrapper palette sidecar.
    // Generic raster-only execution rejects this output before running pixels.
    if let Some(matrix) = matrix {
        return prepare_matrix(input, *mode, matrix, ctx)
            .map(crate::compute::registry::PreparedOperation::from_convert);
    }
    prepare(input, *mode, dither.clone(), ctx)
        .map(crate::compute::registry::PreparedOperation::from_convert)
}

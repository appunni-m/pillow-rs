//! Prepared Convert execution and typed child-strategy selection.

use crate::compute::{ExecutionContext, registry::support::{
    ModeSupport, OperationId, OperationInputs, OperationRouteOutcome, PaletteKnowledge,
    RequestFacts, StrategyInputs, StrategySelection, SupportAssessment,
}};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::{DitherMethod, PipelineOp};
use crate::raster::{DynamicImage, Mode};

use super::super::ops::{MATRIX_SUPPORT, SUPPORT};
use super::target_compositions::{self, LabNormalization};

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
    lab: Option<LabConvertSelections>,
}

struct LabConvertSelections {
    entries: Vec<StrategySelection>,
    normalization: LabNormalization,
}

impl LabConvertSelections {
    fn as_slice(&self) -> &[StrategySelection] {
        &self.entries
    }
}

impl Prepared<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn composed_selections(&self) -> &[StrategySelection] {
        self.lab
            .as_ref()
            .map_or(&[], LabConvertSelections::as_slice)
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
            return super::matrix::convert_with_matrix(
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
        if self.target == Mode::ImageLab8 {
            return self.execute_lab().map(PreparedConvertOutput::raster);
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

    fn execute_lab(self) -> Result<DynamicImage, PilError> {
        if self.input.mode() == Mode::ImageLab8 {
            return Ok(self.input.image.clone());
        }
        let route = self.lab.ok_or_else(|| {
            PilError::InternalError("Convert-to-LAB preparation omitted its child strategies".into())
        })?;
        match route.normalization {
            LabNormalization::Direct => {
                let convert_lab = route.strategy(0)?;
                execute_lab_strategy(self.input.image, convert_lab)
            }
            LabNormalization::ToRgba => {
                let rgba = execute_lab_convert_step(
                    self.input,
                    Mode::ImageRgba8,
                    route.strategy(0)?,
                )?;
                require_lab_mode(rgba, Mode::ImageRgba8)?;
                execute_lab_strategy(&rgba, route.strategy(1)?)
            }
            LabNormalization::ViaLuma => {
                let luma = execute_lab_convert_step(
                    self.input,
                    Mode::ImageLuma8,
                    route.strategy(0)?,
                )?;
                require_lab_mode(luma, Mode::ImageLuma8)?;
                let luma_input = lab_child_input(&luma);
                let rgba = execute_lab_convert_step(
                    luma_input,
                    Mode::ImageRgba8,
                    route.strategy(1)?,
                )?;
                require_lab_mode(rgba, Mode::ImageRgba8)?;
                execute_lab_strategy(&rgba, route.strategy(2)?)
            }
            LabNormalization::ViaRgb => {
                let rgb = execute_lab_convert_step(
                    self.input,
                    Mode::ImageRgb8,
                    route.strategy(0)?,
                )?;
                require_lab_mode(rgb, Mode::ImageRgb8)?;
                let rgb_input = lab_child_input(&rgb);
                let rgba = execute_lab_convert_step(
                    rgb_input,
                    Mode::ImageRgba8,
                    route.strategy(1)?,
                )?;
                require_lab_mode(rgba, Mode::ImageRgba8)?;
                execute_lab_strategy(&rgba, route.strategy(2)?)
            }
        }
    }

    fn execute_palette_target(self) -> Result<PreparedConvertOutput, PilError> {
        let input = self.input;
        let target = self.target;
        let dither = self.dither;
        let selection = self.selection;
        match input.image {
            DynamicImage::One(source) => {
                super::super::one::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLuma8(source) => {
                super::super::luma8::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLumaA8(source) => {
                super::super::luma_a8::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImagePalette8(source) => {
                super::super::palette8::ops::execute_palette_target(
                    source,
                    input.palette,
                    target,
                    selection,
                )
            }
            DynamicImage::ImagePaletteAlpha8(source) => {
                super::super::palette_alpha8::ops::execute_palette_target(
                    source,
                    target,
                    selection,
                )
            }
            DynamicImage::ImageRgb8(source) => {
                super::super::rgb8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageRgba8(source) => {
                super::super::rgba8::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageRgbPremultiplied8(source) => {
                super::super::rgb_premultiplied8::ops::execute_palette_target(
                    source,
                    target,
                    dither,
                    selection,
                )
            }
            DynamicImage::ImageRgbx8(source) => {
                super::super::rgbx8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageCmyk8(source) => {
                super::super::cmyk8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageYCbCr8(source) => {
                super::super::ycbcr8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageHsv8(source) => {
                super::super::hsv8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageLab8(source) => {
                super::super::lab8::ops::execute_palette_target(source, target, dither, selection)
            }
            DynamicImage::ImageI32(source) => {
                super::super::i32::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageF32(source) => {
                super::super::f32::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16(source) => {
                super::super::i16::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16L(source) => {
                super::super::i16l::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16B(source) => {
                super::super::i16b::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageI16N(source) => {
                super::super::i16n::ops::execute_palette_target(source, target, selection)
            }
            DynamicImage::ImageLumaA16(source) => super::super::luma_a16::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            DynamicImage::ImageRgb16(source) => super::super::rgb16::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            DynamicImage::ImageRgba16(source) => super::super::rgba16::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            DynamicImage::ImageRgb32F(source) => super::super::rgb32f::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            DynamicImage::ImageRgba32F(source) => super::super::rgba32f::ops::execute_palette_target(
                source, target, dither, selection,
            ),
            _ => Err(PilError::InternalError(
                "Convert selected an undeclared palette target strategy".into(),
            )),
        }
    }

    fn execute_mode(
        input: OperationInputs<'_>,
        target: Mode,
        selection: StrategySelection,
    ) -> Result<DynamicImage, PilError> {
        match input.image {
            DynamicImage::One(source) => {
                return super::super::one::ops::execute(source, target, selection);
            }
            DynamicImage::ImageLuma8(source) => {
                return super::super::luma8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageLumaA8(source) => {
                return super::super::luma_a8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgb8(source) => {
                // SAFETY: prepare retained the exact source, target, and
                // context used to select this declared mode strategy.
                return unsafe { super::super::rgb8::ops::execute(source, target, selection) };
            }
            DynamicImage::ImageRgba8(source) => {
                // SAFETY: `prepare` computed this selection from the same
                // source mode, target, and injected context retained here.
                return unsafe { super::super::rgba8::ops::execute(source, target, selection) };
            }
            DynamicImage::ImageRgbPremultiplied8(source) => {
                return super::super::rgb_premultiplied8::ops::execute(
                    source,
                    target,
                    selection,
                );
            }
            DynamicImage::ImageRgbx8(source) => {
                // SAFETY: prepare selected this strategy from the RGBX
                // contract using the same source, target, and context retained here.
                return unsafe { super::super::rgbx8::ops::execute(source, target, selection) };
            }
            DynamicImage::ImageI32(source) => {
                return super::super::i32::ops::execute(source, target, selection);
            }
            DynamicImage::ImageF32(source) => {
                return super::super::f32::ops::execute(source, target, selection);
            }
            DynamicImage::ImageI16(source) => {
                return super::super::i16::ops::execute(source, target, selection);
            }
            DynamicImage::ImageI16L(source) => {
                return super::super::i16l::ops::execute(source, target, selection);
            }
            DynamicImage::ImageI16B(source) => {
                return super::super::i16b::ops::execute(source, target, selection);
            }
            DynamicImage::ImageI16N(source) => {
                return super::super::i16n::ops::execute(source, target, selection);
            }
            DynamicImage::ImageLaPremultiplied8(source) => {
                return super::super::la_premultiplied8::ops::execute(
                    source,
                    target,
                    selection,
                );
            }
            DynamicImage::ImageCmyk8(source) => {
                // SAFETY: prepare selected this strategy from the CMYK
                // contract using the same source, target, and context retained here.
                return unsafe { super::super::cmyk8::ops::execute(source, target, selection) };
            }
            DynamicImage::ImageYCbCr8(source) => {
                return super::super::ycbcr8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageHsv8(source) => {
                return super::super::hsv8::ops::execute(source, target, selection);
            }
            DynamicImage::ImagePalette8(source) => {
                return super::super::palette8::ops::execute(
                    source,
                    input.palette,
                    target,
                    selection,
                );
            }
            DynamicImage::ImagePaletteAlpha8(source) => {
                return super::super::palette_alpha8::ops::execute(
                    source,
                    input.palette,
                    target,
                    selection,
                );
            }
            DynamicImage::ImageLab8(source) => {
                return super::super::lab8::ops::execute(source, target, selection);
            }
            DynamicImage::ImageLumaA16(source) => {
                return super::super::luma_a16::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgb16(source) => {
                return super::super::rgb16::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgba16(source) => {
                return super::super::rgba16::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgb32F(source) => {
                return super::super::rgb32f::ops::execute(source, target, selection);
            }
            DynamicImage::ImageRgba32F(source) => {
                return super::super::rgba32f::ops::execute(source, target, selection);
            }
        }
    }

    fn execute_binary(self) -> Result<DynamicImage, PilError> {
        let input = self.input;
        let dither = self.dither;
        let selection = self.selection;
        match input.image {
            DynamicImage::One(_) => {
                super::super::one::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLuma8(_) => {
                super::super::luma8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLumaA8(_) => {
                super::super::luma_a8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLaPremultiplied8(_) => super::super::la_premultiplied8::ops::execute_binary(
                input,
                dither.as_ref(),
                selection,
            ),
            DynamicImage::ImagePalette8(_) => {
                super::super::palette8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImagePaletteAlpha8(_) => {
                super::super::palette_alpha8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb8(_) => {
                super::super::rgb8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba8(_) => {
                super::super::rgba8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgbPremultiplied8(_) => super::super::rgb_premultiplied8::ops::execute_binary(
                input,
                dither.as_ref(),
                selection,
            ),
            DynamicImage::ImageRgbx8(_) => {
                super::super::rgbx8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageCmyk8(_) => {
                super::super::cmyk8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageYCbCr8(_) => {
                super::super::ycbcr8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLab8(_) => {
                super::super::lab8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageHsv8(_) => {
                super::super::hsv8::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI32(_) => {
                super::super::i32::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageF32(_) => {
                super::super::f32::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16(_) => {
                super::super::i16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16L(_) => {
                super::super::i16l::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16B(_) => {
                super::super::i16b::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageI16N(_) => {
                super::super::i16n::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageLumaA16(_) => {
                super::super::luma_a16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb16(_) => {
                super::super::rgb16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba16(_) => {
                super::super::rgba16::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgb32F(_) => {
                super::super::rgb32f::ops::execute_binary(input, dither.as_ref(), selection)
            }
            DynamicImage::ImageRgba32F(_) => {
                super::super::rgba32f::ops::execute_binary(input, dither.as_ref(), selection)
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
    if let Some(composition) = SUPPORT.target_composition_for(input.mode(), target) {
        if let OperationRouteOutcome::Rejects { reason, kind, .. } = composition.outcome {
            return Err(kind.into_error(reason));
        }
    }
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
    let lab = prepare_lab_route(input, target, ctx)?;
    Ok(Prepared { input, target, matrix: None, dither, selection, lab })
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
    Ok(Prepared { input, target, matrix: Some(matrix), dither: None, selection, lab: None })
}

fn prepare_lab_route(
    input: OperationInputs<'_>,
    target: Mode,
    ctx: &ExecutionContext,
) -> Result<Option<LabConvertSelections>, PilError> {
    if target != Mode::ImageLab8 || input.mode() == Mode::ImageLab8 {
        return Ok(None);
    }

    let normalization = target_compositions::lab_normalization(input.mode());
    let width = input.image.width();
    let height = input.image.height();
    let mut entries = Vec::with_capacity(3);
    match normalization {
        LabNormalization::Direct => {
            entries.push(select_strategy(
                crate::compute::pool_cpu::ops::color::ops::SUPPORT.for_mode(input.mode()),
                input.facts(),
                ctx,
                "ConvertLab support remained unresolved for its native source mode",
            )?);
        }
        LabNormalization::ToRgba => {
            entries.push(select_convert_target(
                input.mode(),
                input.facts(),
                Mode::ImageRgba8,
                ctx,
            )?);
        }
        LabNormalization::ViaLuma => {
            entries.push(select_convert_target(
                input.mode(),
                input.facts(),
                Mode::ImageLuma8,
                ctx,
            )?);
            let luma_facts = facts_for_mode(width, height, 1)?;
            entries.push(select_convert_target(
                Mode::ImageLuma8,
                luma_facts,
                Mode::ImageRgba8,
                ctx,
            )?);
        }
        LabNormalization::ViaRgb => {
            entries.push(select_convert_target(
                input.mode(),
                input.facts(),
                Mode::ImageRgb8,
                ctx,
            )?);
            let rgb_facts = facts_for_mode(width, height, 3)?;
            entries.push(select_convert_target(
                Mode::ImageRgb8,
                rgb_facts,
                Mode::ImageRgba8,
                ctx,
            )?);
        }
    }
    if normalization != LabNormalization::Direct {
        let rgba_facts = facts_for_mode(width, height, 4)?;
        entries.push(select_strategy(
            crate::compute::pool_cpu::ops::color::ops::SUPPORT.for_mode(Mode::ImageRgba8),
            rgba_facts,
            ctx,
            "ConvertLab support remained unresolved after RGBA normalization",
        )?);
    }
    Ok(Some(LabConvertSelections {
        entries,
        normalization,
    }))
}

impl LabConvertSelections {
    fn strategy(&self, index: usize) -> Result<StrategySelection, PilError> {
        self.entries.get(index).copied().ok_or_else(|| {
            PilError::InternalError("Convert-to-LAB route omitted a child strategy".into())
        })
    }
}

fn execute_lab_convert_step(
    input: OperationInputs<'_>,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    crate::compute::record_pipeline_operation_selection(&selection);
    Prepared::execute_mode(input, target, selection)
}

fn lab_child_input(image: &DynamicImage) -> OperationInputs<'_> {
    OperationInputs {
        image,
        palette: crate::compute::registry::support::PaletteInput::default(),
    }
}

fn require_lab_mode(image: DynamicImage, expected: Mode) -> Result<DynamicImage, PilError> {
    if image.mode() == expected {
        Ok(image)
    } else {
        Err(PilError::InternalError(
            "Convert-to-LAB normalization returned another mode".into(),
        ))
    }
}

fn execute_lab_strategy(
    image: &DynamicImage,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    crate::compute::record_pipeline_operation_selection(&selection);
    crate::compute::pool_cpu::ops::color::ops::execute_selected_convert_lab(image, selection)
}

fn select_convert_target(
    source: Mode,
    facts: StrategyInputs,
    target: Mode,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    select_strategy(
        SUPPORT.for_mode(source),
        facts.with_convert_target(target),
        ctx,
        "Convert child support remained unresolved after source materialization",
    )
}

fn select_strategy(
    support: ModeSupport,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
    unresolved_message: &'static str,
) -> Result<StrategySelection, PilError> {
    match support.assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            Err(PilError::InternalError(unresolved_message.into()))
        }
    }
}

fn facts_for_mode(width: u32, height: u32, channels: u8) -> Result<StrategyInputs, PilError> {
    let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
    Ok(StrategyInputs {
        width,
        height,
        byte_len: Some(dimensions.total_bytes()),
        palette: PaletteKnowledge::Absent,
        request: RequestFacts::Unspecified,
    })
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Convert { mode, matrix, dither } = op else {
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


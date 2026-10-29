//! Mode admission and prepared execution for ImageOps Equalize.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, OutputModeRule,
    PaletteKnowledge, RejectionKind, RequestFacts, StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GrayImage, Mode, RgbImage};
use std::sync::Arc;

const INPUTS: &[InputContract] = &[
    InputContract::ImageOpsLumaMask,
    InputContract::PaletteMetadata,
];
const LUMA_INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];
const RGB_INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 3, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];
const PALETTE_INPUTS: &[InputContract] = &[
    InputContract::PaletteMetadata,
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];

const LUMA_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Equalize,
        Mode::ImageLuma8,
        1,
    );
const RGB_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Equalize,
        Mode::ImageRgb8,
        3,
    );
const PALETTE_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] =
    super::helpers::strategy::cpu_architectures_for(
        OperationId::Equalize,
        Mode::ImagePalette8,
        StrategyGuard::Always,
    );

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => ModeSupport::cpu(
            OperationId::Equalize,
            mode,
            OutputModeRule::Preserve,
            LUMA_INPUTS,
            &LUMA_ARCHITECTURES,
        ),
        Mode::ImageRgb8 => ModeSupport::cpu(
            OperationId::Equalize,
            mode,
            OutputModeRule::Preserve,
            RGB_INPUTS,
            &RGB_ARCHITECTURES,
        ),
        Mode::ImagePalette8 => ModeSupport::cpu(
            OperationId::Equalize,
            mode,
            OutputModeRule::Fixed(Mode::ImageRgb8),
            PALETTE_INPUTS,
            &PALETTE_ARCHITECTURES,
        ),
        _ => ModeSupport::Rejected {
            mode,
            reason: super::helpers::strategy::unsupported_mode_reason(mode),
            kind: RejectionKind::OsError,
        },
    }
}

/// Exhaustive ImageOps Equalize mode, mask, palette, and architecture contract.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Equalize, INPUTS, classify_mode);

enum Source<'a> {
    Luma(&'a GrayImage),
    Rgb(&'a RgbImage),
    Palette(RgbImage),
}

/// Equalize with its selected source-mode and architecture strategies.
pub(crate) struct PreparedEqualize<'a> {
    source: Source<'a>,
    lut: Vec<u8>,
    architecture: super::helpers::strategy::NativeByteArchitecture,
    selection: StrategySelection,
}

impl PreparedEqualize<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        use super::helpers::strategy::NativeByteArchitecture as Architecture;
        match (self.source, self.architecture) {
            (Source::Luma(input), Architecture::Default) => {
                super::luma8::default::equalize_impl::execute(input, &self.lut)
            }
            (Source::Luma(input), Architecture::Sse2) => {
                super::luma8::sse2::histogram_impl::equalize(input, &self.lut)
            }
            (Source::Luma(input), Architecture::Neon) => {
                super::luma8::neon::histogram_impl::equalize(input, &self.lut)
            }
            (Source::Luma(input), Architecture::WasmSimd128) => {
                super::luma8::wasm_simd128::histogram_impl::equalize(input, &self.lut)
            }
            (Source::Rgb(input), Architecture::Default) => {
                super::rgb8::default::equalize_impl::execute(input, &self.lut)
            }
            (Source::Rgb(input), Architecture::Sse2) => {
                super::rgb8::sse2::histogram_impl::equalize(input, &self.lut)
            }
            (Source::Rgb(input), Architecture::Neon) => {
                super::rgb8::neon::histogram_impl::equalize(input, &self.lut)
            }
            (Source::Rgb(input), Architecture::WasmSimd128) => {
                super::rgb8::wasm_simd128::histogram_impl::equalize(input, &self.lut)
            }
            (Source::Palette(input), Architecture::Default) => {
                super::palette8::default::equalize_impl::execute_rgb(&input, &self.lut)
            }
            (Source::Palette(input), Architecture::Sse2) => {
                super::palette8::sse2::equalize_impl::execute(&input, &self.lut)
            }
            (Source::Palette(input), Architecture::Neon) => {
                super::palette8::neon::equalize_impl::execute(&input, &self.lut)
            }
            (Source::Palette(input), Architecture::WasmSimd128) => {
                super::palette8::wasm_simd128::equalize_impl::execute(&input, &self.lut)
            }
        }
    }
}

fn equalize_lut(
    image: &DynamicImage,
    channels: usize,
    mask: Option<&DynamicImage>,
) -> Result<Vec<u8>, PilError> {
    super::helpers::histogram::equalize_lut(image, channels, mask)
        .ok_or_else(|| PilError::ValueError("images do not match".into()))
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    mask: Option<&Arc<crate::image::Image>>,
    ctx: &ExecutionContext,
) -> Result<PreparedEqualize<'a>, PilError> {
    let mask_facts = super::helpers::strategy::imageops_mask_facts(mask)?;
    let image = input.image;
    let selection = super::helpers::strategy::select_mode(
        SUPPORT.for_mode(image.mode()),
        image,
        RequestFacts::ImageOpsMask { mask: mask_facts },
        input.palette.knowledge(),
        ctx,
    )?;
    let architecture = super::helpers::strategy::native_byte_architecture(&selection)?;
    let materialized_mask = mask.map(|mask| mask.materialized_shared()).transpose()?;
    let mask = materialized_mask.as_deref();
    let (source, lut) = match image {
        DynamicImage::ImageLuma8(source) => {
            (Source::Luma(source), equalize_lut(image, 1, mask)?)
        }
        DynamicImage::ImageRgb8(source) => {
            (Source::Rgb(source), equalize_lut(image, 3, mask)?)
        }
        DynamicImage::ImagePalette8(indices) => {
            let rgb = super::palette8::default::equalize_impl::expand_to_rgb(
                indices,
                input.palette,
            );
            let lut = super::helpers::histogram::equalize_lut_for_bytes(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                3,
                mask,
            )
            .ok_or_else(|| PilError::ValueError("images do not match".into()))?;
            (Source::Palette(rgb), lut)
        }
        _ => {
            return Err(PilError::InternalError(
                "Equalize support and mode dispatch disagree".into(),
            ));
        }
    };
    Ok(PreparedEqualize { source, lut, architecture, selection })
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let mask = match op {
        PipelineOp::Equalize => None,
        PipelineOp::EqualizeMasked { mask } => Some(mask),
        _ => return Err(PilError::ValueError("expected Equalize op".into())),
    };
    prepare(input, mask, ctx).map(crate::compute::registry::PreparedOperation::from_equalize)
}

//! Mode admission and prepared execution for ImageOps Autocontrast.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, OutputModeRule,
    PaletteKnowledge, RejectionKind, RequestFacts, StrategySelection,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GrayImage, Mode, RgbImage};
use std::sync::Arc;

const INPUTS: &[InputContract] = &[InputContract::ImageOpsLumaMask];
const LUMA_INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];
const RGB_INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 3, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];

const LUMA_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Autocontrast,
        Mode::ImageLuma8,
        1,
    );
const RGB_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Autocontrast,
        Mode::ImageRgb8,
        3,
    );

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => ModeSupport::cpu(
            OperationId::Autocontrast,
            mode,
            OutputModeRule::Preserve,
            LUMA_INPUTS,
            &LUMA_ARCHITECTURES,
        ),
        Mode::ImageRgb8 => ModeSupport::cpu(
            OperationId::Autocontrast,
            mode,
            OutputModeRule::Preserve,
            RGB_INPUTS,
            &RGB_ARCHITECTURES,
        ),
        Mode::ImagePalette8 => ModeSupport::Rejected {
            mode,
            reason: "mode P support coming soon",
            kind: RejectionKind::NotImplementedError,
        },
        _ => ModeSupport::Rejected {
            mode,
            reason: super::helpers::strategy::unsupported_mode_reason(mode),
            kind: RejectionKind::OsError,
        },
    }
}

/// Exhaustive ImageOps Autocontrast mode, mask, and architecture contract.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Autocontrast, INPUTS, classify_mode);

enum Source<'a> {
    Luma(&'a GrayImage),
    Rgb(&'a RgbImage),
}

/// Autocontrast with a validated mode, histogram, and selected CPU kernel.
pub(crate) struct PreparedAutocontrast<'a> {
    source: Source<'a>,
    lut: Vec<u8>,
    architecture: super::helpers::strategy::NativeByteArchitecture,
    selection: StrategySelection,
}

impl PreparedAutocontrast<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        use super::helpers::strategy::NativeByteArchitecture as Architecture;
        match (self.source, self.architecture) {
            (Source::Luma(input), Architecture::Default) => {
                super::luma8::default::autocontrast_impl::execute(input, &self.lut)
            }
            (Source::Luma(input), Architecture::Sse2) => {
                super::luma8::sse2::histogram_impl::autocontrast(input, &self.lut)
            }
            (Source::Luma(input), Architecture::Neon) => {
                super::luma8::neon::histogram_impl::autocontrast(input, &self.lut)
            }
            (Source::Luma(input), Architecture::WasmSimd128) => {
                super::luma8::wasm_simd128::histogram_impl::autocontrast(input, &self.lut)
            }
            (Source::Rgb(input), Architecture::Default) => {
                super::rgb8::default::autocontrast_impl::execute(input, &self.lut)
            }
            (Source::Rgb(input), Architecture::Sse2) => {
                super::rgb8::sse2::histogram_impl::autocontrast(input, &self.lut)
            }
            (Source::Rgb(input), Architecture::Neon) => {
                super::rgb8::neon::histogram_impl::autocontrast(input, &self.lut)
            }
            (Source::Rgb(input), Architecture::WasmSimd128) => {
                super::rgb8::wasm_simd128::histogram_impl::autocontrast(input, &self.lut)
            }
        }
    }
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    cutoff: f64,
    mask: Option<&Arc<crate::image::Image>>,
    ctx: &ExecutionContext,
) -> Result<PreparedAutocontrast<'a>, PilError> {
    let mask_facts = super::helpers::strategy::imageops_mask_facts(mask)?;
    let selection = super::helpers::strategy::select_mode(
        SUPPORT.for_mode(image.mode()),
        image,
        RequestFacts::ImageOpsMask { mask: mask_facts },
        PaletteKnowledge::Unknown,
        ctx,
    )?;
    let architecture = super::helpers::strategy::native_byte_architecture(&selection)?;
    let lut = if image.width() == 0 || image.height() == 0 {
        Vec::new()
    } else {
        super::helpers::histogram::autocontrast_lut(image, cutoff, mask)?
    };
    let source = match image {
        DynamicImage::ImageLuma8(input) => Source::Luma(input),
        DynamicImage::ImageRgb8(input) => Source::Rgb(input),
        _ => {
            return Err(PilError::InternalError(
                "Autocontrast support and mode dispatch disagree".into(),
            ));
        }
    };
    Ok(PreparedAutocontrast { source, lut, architecture, selection })
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Autocontrast { cutoff, mask } = op else {
        return Err(PilError::ValueError("expected Autocontrast op".into()));
    };
    prepare(input.image, *cutoff, mask.as_ref(), ctx)
        .map(crate::compute::registry::PreparedOperation::from_autocontrast)
}

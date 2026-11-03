//! Mode admission and prepared execution for ImageOps Autocontrast.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, PaletteKnowledge, RejectionKind, RequestFacts, StrategySelection,
};
use crate::error::PilError;
use crate::pipeline::AutocontrastOptions;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(super) const INPUTS: &[InputContract] = &[
    InputContract::AutocontrastParameters,
    InputContract::ImageOpsLumaMask,
];
pub(super) fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::AUTOCONTRAST_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::AUTOCONTRAST_SUPPORT,
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

/// Prepared Autocontrast strategy routed to its exact source-mode owner.
pub(crate) enum PreparedAutocontrast<'a> {
    Luma(super::luma8::ops::PreparedAutocontrast<'a>),
    Rgb(super::rgb8::ops::PreparedAutocontrast<'a>),
}

impl PreparedAutocontrast<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma(prepared) => prepared.selection(),
            Self::Rgb(prepared) => prepared.selection(),
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let mut output = match self {
            Self::Luma(prepared) => prepared.execute()?,
            Self::Rgb(prepared) => prepared.execute()?,
        };
        super::helpers::hidden_lanes::initialize_point_output_lanes(&mut output)?;
        Ok(output)
    }
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    options: &AutocontrastOptions,
    mask: Option<&Arc<crate::image::Image>>,
    ctx: &ExecutionContext,
) -> Result<PreparedAutocontrast<'a>, PilError> {
    let mask_facts = super::helpers::strategy::imageops_mask_facts(mask)?;
    let selection = super::helpers::strategy::select_mode(
        super::ops::AUTOCONTRAST_SUPPORT.for_mode(image.mode()),
        image,
        RequestFacts::ImageOpsMask { mask: mask_facts },
        PaletteKnowledge::Unknown,
        ctx,
    )?;
    let lut = super::helpers::histogram::autocontrast_lut(image, options, mask)?;
    let source = match image {
        DynamicImage::ImageLuma8(input) => PreparedAutocontrast::Luma(
            super::luma8::ops::PreparedAutocontrast::new(input, lut, selection),
        ),
        DynamicImage::ImageRgb8(input) => PreparedAutocontrast::Rgb(
            super::rgb8::ops::PreparedAutocontrast::new(input, lut, selection),
        ),
        _ => {
            return Err(PilError::InternalError(
                "Autocontrast support and mode dispatch disagree".into(),
            ));
        }
    };
    Ok(source)
}

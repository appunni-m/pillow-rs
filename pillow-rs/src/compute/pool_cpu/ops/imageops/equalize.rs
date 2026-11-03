//! Mode admission and prepared execution for ImageOps Equalize.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationInputs, PaletteKnowledge, RejectionKind, RequestFacts,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(super) const INPUTS: &[InputContract] = &[
    InputContract::ImageOpsLumaMask,
    InputContract::PaletteMetadata,
];
pub(super) fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::EQUALIZE_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::EQUALIZE_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::EQUALIZE_SUPPORT,
        _ => ModeSupport::Rejected {
            mode,
            reason: super::helpers::strategy::unsupported_mode_reason(mode),
            kind: RejectionKind::OsError,
        },
    }
}

/// Prepared Equalize strategy routed to its exact source-mode owner.
pub(crate) enum PreparedEqualize<'a> {
    Luma(super::luma8::ops::PreparedEqualize<'a>),
    Rgb(super::rgb8::ops::PreparedEqualize<'a>),
    Palette(super::palette8::ops::PreparedEqualize),
}

impl PreparedEqualize<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma(prepared) => prepared.selection(),
            Self::Rgb(prepared) => prepared.selection(),
            Self::Palette(prepared) => prepared.selection(),
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let mut output = match self {
            Self::Luma(prepared) => prepared.execute()?,
            Self::Rgb(prepared) => prepared.execute()?,
            Self::Palette(prepared) => prepared.execute()?,
        };
        super::helpers::hidden_lanes::initialize_point_output_lanes(&mut output)?;
        Ok(output)
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
        super::ops::EQUALIZE_SUPPORT.for_mode(image.mode()),
        image,
        RequestFacts::ImageOpsMask { mask: mask_facts },
        input.palette.knowledge(),
        ctx,
    )?;
    let materialized_mask = mask.map(|mask| mask.materialized_shared()).transpose()?;
    let mask = materialized_mask.as_deref();
    match image {
        DynamicImage::ImageLuma8(source) => Ok(PreparedEqualize::Luma(
            super::luma8::ops::PreparedEqualize::new(source, equalize_lut(image, 1, mask)?, selection),
        )),
        DynamicImage::ImageRgb8(source) => Ok(PreparedEqualize::Rgb(
            super::rgb8::ops::PreparedEqualize::new(source, equalize_lut(image, 3, mask)?, selection),
        )),
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
            Ok(PreparedEqualize::Palette(
                super::palette8::ops::PreparedEqualize::new(rgb, lut, selection),
            ))
        }
        _ => Err(PilError::InternalError(
            "Equalize support and mode dispatch disagree".into(),
        )),
    }
}

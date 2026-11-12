//! Shared preparation for exact-mode ImageOps Autocontrast strategies.

use std::sync::Arc;

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    OperationSupport, PaletteKnowledge, RequestFacts, StrategySelection,
};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::AutocontrastOptions;
use crate::raster::DynamicImage;

/// Prepared Autocontrast strategy routed to its exact source-mode owner.
pub(crate) enum PreparedAutocontrast<'a> {
    Luma(super::super::luma8::ops::PreparedAutocontrast<'a>),
    Palette(super::super::palette8::ops::PreparedAutocontrast<'a>),
    Rgb(super::super::rgb8::ops::PreparedAutocontrast<'a>),
}

impl PreparedAutocontrast<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma(prepared) => prepared.selection(),
            Self::Palette(prepared) => prepared.selection(),
            Self::Rgb(prepared) => prepared.selection(),
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let mut output = match self {
            Self::Luma(prepared) => prepared.execute()?,
            Self::Palette(prepared) => prepared.execute()?,
            Self::Rgb(prepared) => prepared.execute()?,
        };
        super::hidden_lanes::initialize_point_output_lanes(&mut output)?;
        Ok(output)
    }
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    options: &AutocontrastOptions,
    mask: Option<&Arc<Image>>,
    support: &'static OperationSupport,
    ctx: &ExecutionContext,
) -> Result<PreparedAutocontrast<'a>, PilError> {
    let mask_facts = super::strategy::imageops_mask_facts(mask)?;
    let selection = super::strategy::select_mode(
        support.for_mode(image.mode()),
        image,
        RequestFacts::ImageOpsMask { mask: mask_facts },
        PaletteKnowledge::Unknown,
        ctx,
    )?;
    let lut = super::histogram::autocontrast_lut(image, options, mask)?;
    match image {
        DynamicImage::ImageLuma8(input) => Ok(PreparedAutocontrast::Luma(
            super::super::luma8::ops::PreparedAutocontrast::new(input, lut, selection),
        )),
        DynamicImage::ImagePalette8(input) => Ok(PreparedAutocontrast::Palette(
            super::super::palette8::ops::PreparedAutocontrast::new(input, lut, selection),
        )),
        DynamicImage::ImageRgb8(input) => Ok(PreparedAutocontrast::Rgb(
            super::super::rgb8::ops::PreparedAutocontrast::new(input, lut, selection),
        )),
        _ => Err(PilError::InternalError(
            "Autocontrast support and mode dispatch disagree".into(),
        )),
    }
}

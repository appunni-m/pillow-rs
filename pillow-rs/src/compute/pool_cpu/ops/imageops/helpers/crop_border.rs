//! Prepared CropBorder plan composed from the exact-mode crop strategy.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::ops::crop::ops::{CropRequest, PreparedCrop};
use crate::compute::registry::support::{
    PaletteKnowledge, RequestFacts, StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView};

pub(crate) enum PreparedCropBorder<'a> {
    Crop(PreparedCrop<'a>, StrategySelection),
    Empty(&'a DynamicImage, u32, StrategySelection),
}

impl PreparedCropBorder<'_> {
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Crop(_, selection) | Self::Empty(_, _, selection) => selection,
        }
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self {
            Self::Crop(crop, _) => crop.execute(),
            Self::Empty(image, border, _) => {
                let (width, height) = image.dimensions();
                Ok(image.crop_imm(border, border, width - 2 * border, height - 2 * border))
            }
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::imageops) fn prepare_crop_border<'a>(
    image: &'a DynamicImage,
    border: u32,
    ctx: &ExecutionContext,
) -> Result<PreparedCropBorder<'a>, PilError> {
    let (width, height) = image.dimensions();
    let assessment = super::super::ops::CROP_BORDER_SUPPORT
        .for_mode(image.mode())
        .assess(
            ctx,
            StrategyInputs {
                width,
                height,
                byte_len: Some(image.as_bytes().len()),
                palette: PaletteKnowledge::Unknown,
                request: RequestFacts::CropBorder { border },
            },
        )?;
    let selection = match assessment {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "CropBorder preparation has unresolved input facts".into(),
            ));
        }
    };
    let crop_width = width - 2 * border;
    let crop_height = height - 2 * border;
    if crop_width == 0 || crop_height == 0 {
        return Ok(PreparedCropBorder::Empty(image, border, selection));
    }
    let crop = crate::compute::pool_cpu::ops::crop::ops::prepare(
        image,
        CropRequest {
            left: border,
            top: border,
            right: width - border,
            bottom: height - border,
        },
        ctx,
    )?;
    // CropBorder owns a matching I;16 architecture declaration; Crop owns the
    // actual prepared row-copy kernel selected under its own operation id.
    Ok(PreparedCropBorder::Crop(crop, selection))
}

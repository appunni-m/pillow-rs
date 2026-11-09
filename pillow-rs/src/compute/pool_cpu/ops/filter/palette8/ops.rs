use crate::compute::registry::support::{
    OperationId, PaletteKnowledge, RequestFacts, StrategyInputs, SupportAssessment,
};
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::ImagePalette8;

// Pillow's private ImagingCore.rankfilter accepts P's single-byte indices,
// while the public scalar RankFilter contract rejects palette images. Keep
// this callback route separately visible and reuse the native byte kernel.
const IMAGING_RANK_FILTER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::registry::support::ModeSupport::serial(
        OperationId::ImagingCorePaletteRankFilter,
        MODE,
        super::super::ops::inputs_for(OperationId::ImagingCorePaletteRankFilter),
    );

pub(crate) struct PreparedImagingRankFilter<'a> {
    input: &'a DynamicImage,
    size: u32,
    rank: u32,
    selection: crate::compute::registry::support::StrategySelection,
}

impl PreparedImagingRankFilter<'_> {
    pub(crate) fn selection(&self) -> &crate::compute::registry::support::StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, crate::error::PilError> {
        validate_imaging_rank_selection(&self.selection)?;
        execute_imaging_rank_filter(self.input, self.size, self.rank)
    }
}

pub(crate) fn prepare_imaging_rank_filter<'a>(
    input: &'a DynamicImage,
    size: u32,
    rank: u32,
    context: &crate::compute::ExecutionContext,
) -> Result<PreparedImagingRankFilter<'a>, crate::error::PilError> {
    if input.mode() != MODE {
        return Err(crate::error::PilError::InternalError(
            "palette rank filter received a non-P raster".into(),
        ));
    }
    let selection = match IMAGING_RANK_FILTER_SUPPORT.assess(
        context,
        StrategyInputs {
            width: input.width(),
            height: input.height(),
            byte_len: Some(input.as_bytes().len()),
            palette: PaletteKnowledge::Unknown,
            request: RequestFacts::FilterWindow {
                size,
                rank: Some(rank),
                f32_rank_compatibility: None,
            },
        },
    )? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(crate::error::PilError::InternalError(
                "palette rank filter requires materialized input facts".into(),
            ));
        }
    };
    Ok(PreparedImagingRankFilter {
        input,
        size,
        rank,
        selection,
    })
}

pub(in crate::compute::pool_cpu::ops::filter) fn imaging_core_rank_filter_support()
-> crate::compute::registry::support::ModeSupport {
    IMAGING_RANK_FILTER_SUPPORT
}

fn validate_imaging_rank_selection(
    selection: &crate::compute::registry::support::StrategySelection,
) -> Result<(), crate::error::PilError> {
    let implementation = selection.implementation;
    if implementation.operation == OperationId::ImagingCorePaletteRankFilter
        && implementation.mode == MODE
        && implementation.backend == crate::compute::Backend::Cpu
        && implementation.architecture == crate::compute::Architecture::Default
    {
        Ok(())
    } else {
        Err(crate::error::PilError::InternalError(
            "palette rank filter received a mismatched CPU strategy".into(),
        ))
    }
}

fn execute_imaging_rank_filter(
    image: &DynamicImage,
    size: u32,
    rank: u32,
) -> Result<DynamicImage, crate::error::PilError> {
    let DynamicImage::ImagePalette8(input) = image else {
        return Err(crate::error::PilError::InternalError(
            "palette rank filter received a mismatched raster variant".into(),
        ));
    };
    let output = super::super::helpers::rank::execute_native_bytes(
        input.as_raw(),
        input.width(),
        input.height(),
        1,
        size,
        rank,
    )?
    .unwrap_or_else(|| input.as_raw().to_vec());
    let output = crate::raster::GrayImage::from_raw(input.width(), input.height(), output)
        .ok_or_else(|| {
            crate::error::PilError::InternalError(
                "palette rank output does not match its index raster".into(),
            )
        })?;
    let half = size / 2;
    let width = input.width() - half * 2;
    let height = input.height() - half * 2;
    Ok(DynamicImage::ImagePalette8(output).crop_imm(half, half, width, height))
}

pub(in crate::compute::pool_cpu::ops::filter) fn mode_filter_support()
-> crate::compute::registry::support::ModeSupport {
    super::super::ops::serial_mode_filter(MODE)
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute_mode_filter(
    image: &crate::raster::DynamicImage,
    size: u32,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    super::default::execute_mode_filter(image, size)
}

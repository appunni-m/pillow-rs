//! Prepared Color3DLut execution retaining exact mode and routing selection.

use crate::compute::{ExecutionContext, registry::support::{
    ModeSupport, RequestFacts, StrategyInputs, StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) struct Strategy<'a> {
    image: &'a DynamicImage,
    size: (u32, u32, u32),
    table: &'a [f64],
    channels: u32,
    target_mode: Mode,
    selection: StrategySelection,
}

impl<'a> Strategy<'a> {
    pub(crate) fn prepare(
        image: &'a DynamicImage,
        mode: Mode,
        size: (u32, u32, u32),
        table: &'a [f64],
        channels: u32,
        target_mode: Mode,
        support: ModeSupport,
        ctx: &ExecutionContext,
    ) -> Result<Self, PilError> {
        let facts = StrategyInputs {
            width: image.width(),
            height: image.height(),
            byte_len: Some(image.as_bytes().len()),
            palette: crate::compute::registry::support::PaletteKnowledge::Absent,
            request: RequestFacts::Color3DLut {
                target_mode,
                channels,
                size,
                table_len: table.len(),
            },
        };
        let selection = match support.assess(ctx, facts)? {
            SupportAssessment::Ready(selection) => selection,
            SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
            SupportAssessment::Deferred { .. } => return Err(PilError::InternalError(
                "materialized Color3DLut strategy remained deferred".into(),
            )),
        };
        if image.mode() != mode {
            return Err(PilError::InternalError(
                "Color3DLut strategy received a mismatched source mode".into(),
            ));
        }
        Ok(Self { image, size, table, channels, target_mode, selection })
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn into_parts(
        self,
    ) -> (&'a DynamicImage, (u32, u32, u32), &'a [f64], u32, Mode, StrategySelection) {
        (self.image, self.size, self.table, self.channels, self.target_mode, self.selection)
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let mode = self.image.mode();
        match mode {
            Mode::ImageRgb8 => super::super::rgb8::ops::execute(self),
            Mode::ImageRgba8 => super::super::rgba8::ops::execute(self),
            Mode::ImageCmyk8 => super::super::cmyk8::ops::execute(self),
            Mode::ImageYCbCr8 => super::super::ycbcr8::ops::execute(self),
            Mode::ImageLab8 => super::super::lab8::ops::execute(self),
            Mode::ImageHsv8 => super::super::hsv8::ops::execute(self),
            _ => unreachable!("prepared Color3DLut mode was classified as supported"),
        }
    }
}

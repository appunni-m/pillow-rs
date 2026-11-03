//! Prepared Stat strategy composition with its Histogram child.

use super::stat::{StatOutput, StatRequest};
use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::{
    PreparedHistogram, prepare_histogram,
};
use crate::compute::registry::support::{
    OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::ops::STAT_SUPPORT;

macro_rules! declare_stat_owner {
    ($mode:ident, $image_ty:ty, $implementation:path $(,)?) => {
        pub(crate) fn execute_stat(
            image: &$image_ty,
            selection: crate::compute::registry::support::StrategySelection,
            request: crate::compute::pool_cpu::ops::analysis::helpers::stat::StatRequest<'_>,
        ) -> Result<
            crate::compute::pool_cpu::ops::analysis::helpers::stat::StatOutput,
            crate::error::PilError,
        > {
            let implementation = selection.implementation;
            if implementation.mode != crate::raster::Mode::$mode
                || implementation.operation != crate::compute::registry::support::OperationId::Stat
            {
                return Err(crate::error::PilError::InternalError(
                    "Stat strategy selection did not match its operation and mode".into(),
                ));
            }
            match implementation.architecture {
                crate::compute::Architecture::Default => ($implementation)(image, request),
                architecture => Err(crate::error::PilError::InternalError(format!(
                    "Stat has no {:?} {architecture:?} strategy",
                    crate::raster::Mode::$mode,
                ))),
            }
        }
    };
}

pub(crate) use declare_stat_owner;

/// Holds Stat mode selection and its same-context Histogram child, when needed.
pub(crate) struct PreparedStat<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    selection: StrategySelection,
    histogram: Option<PreparedHistogram<'a>>,
}

impl PreparedStat<'_> {
    /// Returns the selected exact-mode Stat implementation.
    pub(crate) const fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    /// Runs scalar Stat directly or consumes its prepared Histogram child.
    pub(crate) fn execute(self) -> Result<StatOutput, PilError> {
        self.validate_selection()?;
        let histogram = self.histogram.map(|histogram| histogram.execute()).transpose()?;
        let request = match histogram.as_deref() {
            Some(histogram) => StatRequest::Histogram(histogram),
            None => StatRequest::Scalar,
        };
        execute_stat_mode(self.image, self.selection, request)
    }

    /// Executes the mode owner with the Histogram child for formatted Stat.
    pub(crate) fn execute_formatted_histogram(self) -> Result<StatOutput, PilError> {
        self.validate_selection()?;
        let histogram = self
            .histogram
            .map(|histogram| histogram.execute())
            .transpose()?
            .ok_or_else(|| {
                PilError::InternalError("masked Stat did not prepare its Histogram child".into())
            })?;
        execute_stat_mode(
            self.image,
            self.selection,
            StatRequest::FormattedHistogram(&histogram),
        )
    }

    fn validate_selection(&self) -> Result<(), PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "Stat prepared mode did not match its DynamicImage tag".into(),
            ));
        }
        let implementation = self.selection.implementation;
        if implementation.operation != OperationId::Stat || implementation.mode != self.mode {
            return Err(PilError::InternalError(
                "Stat strategy selection did not match its operation and mode".into(),
            ));
        }
        Ok(())
    }
}

fn execute_stat_mode(
    image: &DynamicImage,
    selection: StrategySelection,
    request: StatRequest<'_>,
) -> Result<StatOutput, PilError> {
    match image {
        DynamicImage::One(image) => super::one::ops::execute_stat(image, selection, request),
        DynamicImage::ImageLuma8(image) => super::luma8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageLumaA8(image) => super::luma_a8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageLaPremultiplied8(image) => super::la_premultiplied8::ops::execute_stat(image, selection, request),
        DynamicImage::ImagePalette8(image) => super::palette8::ops::execute_stat(image, selection, request),
        DynamicImage::ImagePaletteAlpha8(image) => super::palette_alpha8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageRgb8(image) => super::rgb8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageRgba8(image) => super::rgba8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageRgbPremultiplied8(image) => super::rgb_premultiplied8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageRgbx8(image) => super::rgbx8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageCmyk8(image) => super::cmyk8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageYCbCr8(image) => super::ycbcr8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageLab8(image) => super::lab8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageHsv8(image) => super::hsv8::ops::execute_stat(image, selection, request),
        DynamicImage::ImageI32(image) => super::i32::ops::execute_stat(image, selection, request),
        DynamicImage::ImageF32(image) => super::f32::ops::execute_stat(image, selection, request),
        DynamicImage::ImageI16(image) => super::i16::ops::execute_stat(image, selection, request),
        DynamicImage::ImageI16L(image) => super::i16l::ops::execute_stat(image, selection, request),
        DynamicImage::ImageI16B(image) => super::i16b::ops::execute_stat(image, selection, request),
        DynamicImage::ImageI16N(image) => super::i16n::ops::execute_stat(image, selection, request),
        DynamicImage::ImageLumaA16(image) => super::luma_a16::ops::execute_stat(image, selection, request),
        DynamicImage::ImageRgb16(image) => super::rgb16::ops::execute_stat(image, selection, request),
        DynamicImage::ImageRgba16(image) => super::rgba16::ops::execute_stat(image, selection, request),
        DynamicImage::ImageRgb32F(image) => super::rgb32f::ops::execute_stat(image, selection, request),
        DynamicImage::ImageRgba32F(image) => super::rgba32f::ops::execute_stat(image, selection, request),
    }
}

/// Selects Stat and its Histogram child from the injected CPU context.
pub(crate) fn prepare_stat<'a>(
    image: &'a DynamicImage,
    mask_raw: Option<&'a [u8]>,
    context: &ExecutionContext,
) -> Result<PreparedStat<'a>, PilError> {
    let mode = image.mode();
    let selection = STAT_SUPPORT.for_mode(mode).select_serial(context)?;
    let histogram = if mask_raw.is_none() && matches!(mode, Mode::ImageI32 | Mode::ImageF32) {
        None
    } else {
        Some(prepare_histogram(image, mask_raw, context)?)
    };
    Ok(PreparedStat { image, mode, selection, histogram })
}

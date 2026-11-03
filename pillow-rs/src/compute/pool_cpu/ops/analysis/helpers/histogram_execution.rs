//! Prepared CPU routing for direct per-band Histogram calls.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationSupport, StrategySelection,
};
use crate::error::PilError;
use crate::ops::analysis::HistogramExtrema;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

use super::super::ops::{ENTROPY_SUPPORT, HISTOGRAM_INPUTS, HISTOGRAM_SUPPORT};

pub(crate) fn selected_histogram_architecture(
    selection: StrategySelection,
    mode: crate::raster::Mode,
) -> Result<crate::compute::Architecture, PilError> {
    let implementation = selection.implementation;
    if implementation.mode != mode || implementation.operation != OperationId::Histogram {
        return Err(PilError::InternalError(
            "Histogram strategy selection did not match its operation and mode".into(),
        ));
    }
    Ok(implementation.architecture)
}

/// Generates the exact-mode architecture owner for one Histogram implementation.
macro_rules! declare_histogram_owner {
    ($mode:ident, $image_ty:ty, $implementation:path $(,)?) => {
        pub(crate) fn execute_histogram(
            image: &$image_ty,
            selection: crate::compute::registry::support::StrategySelection,
            mask_raw: Option<&[u8]>,
        ) -> Result<Vec<u32>, crate::error::PilError> {
            match crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::
                selected_histogram_architecture(selection, crate::raster::Mode::$mode)?
            {
                crate::compute::Architecture::Default => {
                    ($implementation)(image, mask_raw)
                }
                architecture => Err(crate::error::PilError::InternalError(format!(
                    "Histogram has no {:?} {architecture:?} strategy",
                    crate::raster::Mode::$mode,
                ))),
            }
        }
    };
}

pub(crate) use declare_histogram_owner;

/// Owns an optional validated mask for the duration of an analysis scan.
pub(crate) struct PreparedAnalysisMask {
    image: Option<Arc<DynamicImage>>,
}

impl PreparedAnalysisMask {
    /// Borrows the accepted `1` or `L` byte plane for the duration of a scan.
    pub(crate) fn raw(&self) -> Option<&[u8]> {
        match self.image.as_deref() {
            Some(DynamicImage::One(mask) | DynamicImage::ImageLuma8(mask)) => {
                Some(mask.as_raw().as_slice())
            }
            Some(_) => unreachable!("validated analysis mask must use 1 or L storage"),
            None => None,
        }
    }
}

/// Validates a materialized Histogram mask with the established error order.
pub(crate) fn prepare_histogram_mask(
    source: &DynamicImage,
    mask: Option<Arc<DynamicImage>>,
) -> Result<PreparedAnalysisMask, PilError> {
    prepare_materialized_analysis_mask(source, mask, &HISTOGRAM_SUPPORT)
}

/// Validates a materialized Entropy mask with its declared input contract.
pub(crate) fn prepare_entropy_mask(
    source: &DynamicImage,
    mask: Option<Arc<DynamicImage>>,
) -> Result<PreparedAnalysisMask, PilError> {
    prepare_materialized_analysis_mask(source, mask, &ENTROPY_SUPPORT)
}

fn prepare_materialized_analysis_mask(
    source: &DynamicImage,
    image: Option<Arc<DynamicImage>>,
    support: &'static OperationSupport,
) -> Result<PreparedAnalysisMask, PilError> {
    let Some(image) = image else {
        return Ok(PreparedAnalysisMask { image: None });
    };
    validate_mask(
        support,
        source.mode(),
        (source.width(), source.height()),
        image.mode(),
        (image.width(), image.height()),
    )?;
    Ok(PreparedAnalysisMask { image: Some(image) })
}

fn validate_mask(
    support_descriptor: &'static OperationSupport,
    source_mode: Mode,
    source_size: (u32, u32),
    mask_mode: Mode,
    mask_size: (u32, u32),
) -> Result<(), PilError> {
    let support = support_descriptor.for_mode(source_mode);
    let inputs = match support {
        ModeSupport::Supported { inputs, .. } => inputs,
        ModeSupport::Rejected { .. } => support_descriptor.inputs,
    };
    let Some(InputContract::HistogramMask { modes, mode_rejection, size_rejection }) =
        inputs.iter().find(|input| matches!(input, InputContract::HistogramMask { .. }))
    else {
        return Err(PilError::InternalError(
            "Analysis support omitted its mask contract".into(),
        ));
    };
    if source_size != mask_size {
        return Err(PilError::ValueError((*size_rejection).into()));
    }
    if !modes.contains(&mask_mode) {
        return Err(PilError::ValueError((*mode_rejection).into()));
    }
    support.validate_mode()
}

/// Holds exact-mode admission and architecture selection until the Histogram scan.
pub(crate) struct PreparedHistogram<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    mask_raw: Option<&'a [u8]>,
    extrema: Option<HistogramExtrema>,
    selection: StrategySelection,
}

impl PreparedHistogram<'_> {
    /// Returns the concrete Default implementation and any architecture fallback.
    pub(crate) const fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    /// Executes the selected exact-mode Histogram route.
    pub(crate) fn execute(self) -> Result<Vec<u32>, PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "Histogram prepared mode did not match its DynamicImage tag".into(),
            ));
        }
        let implementation = self.selection.implementation;
        if implementation.operation != OperationId::Histogram
            || implementation.mode != self.mode
        {
            return Err(PilError::InternalError(
                "Histogram strategy selection did not match its operation and mode".into(),
            ));
        }
        if implementation.architecture != crate::compute::Architecture::Default {
            return Err(PilError::InternalError(format!(
                "Histogram has no {:?} {:?} strategy",
                self.mode, implementation.architecture,
            )));
        }
        if let Some(extrema) = self.extrema {
            return execute_histogram_with_extrema(
                self.image,
                self.selection,
                self.mask_raw,
                extrema,
            );
        }
        execute_histogram_mode(self.image, self.selection, self.mask_raw)
    }
}

fn execute_histogram_with_extrema(
    image: &DynamicImage,
    selection: StrategySelection,
    mask_raw: Option<&[u8]>,
    extrema: HistogramExtrema,
) -> Result<Vec<u32>, PilError> {
    match (image, extrema) {
        (DynamicImage::ImageI32(image), HistogramExtrema::ImageI32 { minimum, maximum }) => {
            super::super::i32::ops::execute_histogram_with_extrema(
                image,
                selection,
                mask_raw,
                minimum,
                maximum,
            )
        }
        (DynamicImage::ImageF32(image), HistogramExtrema::ImageF32 { minimum, maximum }) => {
            super::super::f32::ops::execute_histogram_with_extrema(
                image,
                selection,
                mask_raw,
                minimum,
                maximum,
            )
        }
        _ => Err(PilError::InternalError(
            "Histogram extrema strategy did not match its DynamicImage mode".into(),
        )),
    }
}

fn execute_histogram_mode(
    image: &DynamicImage,
    selection: StrategySelection,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    match image {
        DynamicImage::ImageLumaA16(_)
        | DynamicImage::ImageRgb16(_)
        | DynamicImage::ImageRgba16(_)
        | DynamicImage::ImageRgb32F(_)
        | DynamicImage::ImageRgba32F(_) => {
            execute_coreonly_histogram(image, selection, mask_raw)
        }
        DynamicImage::One(_)
        | DynamicImage::ImageLuma8(_)
        | DynamicImage::ImageLumaA8(_)
        | DynamicImage::ImageLaPremultiplied8(_)
        | DynamicImage::ImagePalette8(_)
        | DynamicImage::ImagePaletteAlpha8(_)
        | DynamicImage::ImageRgb8(_)
        | DynamicImage::ImageRgba8(_)
        | DynamicImage::ImageRgbPremultiplied8(_)
        | DynamicImage::ImageRgbx8(_)
        | DynamicImage::ImageCmyk8(_)
        | DynamicImage::ImageYCbCr8(_)
        | DynamicImage::ImageLab8(_)
        | DynamicImage::ImageHsv8(_)
        | DynamicImage::ImageI32(_)
        | DynamicImage::ImageF32(_)
        | DynamicImage::ImageI16(_)
        | DynamicImage::ImageI16L(_)
        | DynamicImage::ImageI16B(_)
        | DynamicImage::ImageI16N(_) => execute_pillow_histogram(image, selection, mask_raw),
    }
}

fn execute_pillow_histogram(
    image: &DynamicImage,
    selection: StrategySelection,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    match image {
        DynamicImage::One(image) => super::super::one::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageLuma8(image) => super::super::luma8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageLumaA8(image) => super::super::luma_a8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageLaPremultiplied8(image) => {
            super::super::la_premultiplied8::ops::execute_histogram(image, selection, mask_raw)
        }
        DynamicImage::ImagePalette8(image) => super::super::palette8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImagePaletteAlpha8(image) => {
            super::super::palette_alpha8::ops::execute_histogram(image, selection, mask_raw)
        }
        DynamicImage::ImageRgb8(image) => super::super::rgb8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageRgba8(image) => super::super::rgba8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageRgbPremultiplied8(image) => {
            super::super::rgb_premultiplied8::ops::execute_histogram(image, selection, mask_raw)
        }
        DynamicImage::ImageRgbx8(image) => super::super::rgbx8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageCmyk8(image) => super::super::cmyk8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageYCbCr8(image) => super::super::ycbcr8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageLab8(image) => super::super::lab8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageHsv8(image) => super::super::hsv8::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageI32(image) => super::super::i32::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageF32(image) => super::super::f32::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageI16(image) => super::super::i16::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageI16L(image) => super::super::i16l::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageI16B(image) => super::super::i16b::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageI16N(image) => super::super::i16n::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        _ => Err(PilError::InternalError(
            "Pillow Histogram dispatcher received a CoreOnly mode".into(),
        )),
    }
}

fn execute_coreonly_histogram(
    image: &DynamicImage,
    selection: StrategySelection,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    match image {
        DynamicImage::ImageLumaA16(image) => super::super::luma_a16::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageRgb16(image) => super::super::rgb16::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageRgba16(image) => super::super::rgba16::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageRgb32F(image) => super::super::rgb32f::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        DynamicImage::ImageRgba32F(image) => super::super::rgba32f::ops::execute_histogram(
            image, selection, mask_raw,
        ),
        _ => Err(PilError::InternalError(
            "CoreOnly Histogram dispatcher received a Pillow mode".into(),
        )),
    }
}

/// Selects Histogram from the injected CPU architecture context before scanning.
pub(crate) fn prepare_histogram<'a>(
    image: &'a DynamicImage,
    mask_raw: Option<&'a [u8]>,
    context: &ExecutionContext,
) -> Result<PreparedHistogram<'a>, PilError> {
    prepare_histogram_with_extrema(image, mask_raw, None, context)
}

/// Selects Histogram with its optional native I/F range before scanning.
pub(crate) fn prepare_histogram_with_extrema<'a>(
    image: &'a DynamicImage,
    mask_raw: Option<&'a [u8]>,
    extrema: Option<HistogramExtrema>,
    context: &ExecutionContext,
) -> Result<PreparedHistogram<'a>, PilError> {
    let mode = image.mode();
    let selection = HISTOGRAM_SUPPORT.for_mode(mode).select_serial(context)?;
    Ok(PreparedHistogram {
        image,
        mode,
        mask_raw,
        extrema,
        selection,
    })
}

/// Selects Entropy itself, then prepares its Histogram child under the same context.
pub(crate) fn prepare_entropy_histogram<'a>(
    image: &'a DynamicImage,
    mask_raw: Option<&'a [u8]>,
    context: &ExecutionContext,
) -> Result<(PreparedHistogram<'a>, StrategySelection), PilError> {
    prepare_entropy_histogram_with_extrema(image, mask_raw, None, context)
}

/// Selects Entropy and its Histogram child with an optional native I/F range.
pub(crate) fn prepare_entropy_histogram_with_extrema<'a>(
    image: &'a DynamicImage,
    mask_raw: Option<&'a [u8]>,
    extrema: Option<HistogramExtrema>,
    context: &ExecutionContext,
) -> Result<(PreparedHistogram<'a>, StrategySelection), PilError> {
    let mode = image.mode();
    let selection = ENTROPY_SUPPORT.for_mode(mode).select_serial(context)?;
    let histogram = prepare_histogram_with_extrema(image, mask_raw, extrema, context)?;
    Ok((histogram, selection))
}

//! Prepared dispatch for exact-mode typed GetExtrema strategies.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::image::FormattedExtrema;
use crate::raster::{DynamicImage, Mode};

/// Prepared GetExtrema execution retains the exact source mode and route.

pub(crate) const fn support(
    mode: Mode,
    category: GetExtremaSupportCategory,
) -> crate::compute::registry::support::ModeSupport {
    use crate::compute::registry::support::{
        InputContract, ModeSupport, OperationId, OutputModeRule, RejectionKind,
    };
    const NO_INPUTS: &[InputContract] = &[];
    match category {
        GetExtremaSupportCategory::Pillow => ModeSupport::cpu(
            OperationId::GetExtrema,
            mode,
            OutputModeRule::NoRasterResult,
            NO_INPUTS,
            &[],
        ),
        GetExtremaSupportCategory::CoreOnly => ModeSupport::cpu_core_only(
            OperationId::GetExtrema,
            mode,
            OutputModeRule::NoRasterResult,
            NO_INPUTS,
            &[],
            "native-width multiband extrema extend Pillow's mode contract",
        ),
        GetExtremaSupportCategory::Rejected => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
    }
}

#[derive(Clone, Copy)]
pub(crate) enum GetExtremaSupportCategory {
    Pillow,
    CoreOnly,
    Rejected,
}

macro_rules! declare_getextrema_support {
    ($mode:ident, pillow $(,)?) => {
        pub(crate) const GET_EXTREMA_SUPPORT:
            $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::support(
                $crate::raster::Mode::$mode,
                $crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::GetExtremaSupportCategory::Pillow,
            );
    };
    ($mode:ident, core_only $(,)?) => {
        pub(crate) const GET_EXTREMA_SUPPORT:
            $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::support(
                $crate::raster::Mode::$mode,
                $crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::GetExtremaSupportCategory::CoreOnly,
            );
    };
    ($mode:ident, rejected $(,)?) => {
        pub(crate) const GET_EXTREMA_SUPPORT:
            $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::support(
                $crate::raster::Mode::$mode,
                $crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::GetExtremaSupportCategory::Rejected,
            );
    };
}

pub(crate) use declare_getextrema_support;

pub(crate) struct PreparedGetExtrema<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedGetExtrema<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<FormattedExtrema, PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "GetExtrema prepared mode did not match its DynamicImage tag".into(),
            ));
        }
        if self.image.width() == 0 || self.image.height() == 0 {
            return Ok(if self.mode.band_count() == 1 {
                FormattedExtrema::Empty
            } else {
                FormattedExtrema::EmptyMultiple(self.mode.band_count())
            });
        }
        let selection = self.selection;
        match self.image {
            DynamicImage::One(image) => {
                super::super::one::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageLuma8(image) => {
                super::super::luma8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageLumaA8(image) => {
                super::super::luma_a8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageLaPremultiplied8(image) => {
                super::super::la_premultiplied8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImagePalette8(image) => {
                super::super::palette8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImagePaletteAlpha8(image) => {
                super::super::palette_alpha8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageRgb8(image) => {
                super::super::rgb8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageRgba8(image) => {
                super::super::rgba8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageRgbPremultiplied8(image) => {
                super::super::rgb_premultiplied8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageRgbx8(image) => {
                super::super::rgbx8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageCmyk8(image) => {
                super::super::cmyk8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageYCbCr8(image) => {
                super::super::ycbcr8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageLab8(image) => {
                super::super::lab8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageHsv8(image) => {
                super::super::hsv8::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageI32(image) => {
                super::super::i32::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageF32(image) => {
                super::super::f32::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageI16(image) => {
                super::super::i16::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageI16L(_)
            | DynamicImage::ImageI16B(_)
            | DynamicImage::ImageI16N(_) => {
                Err(PilError::ValueError("image has wrong mode".into()))
            }
            DynamicImage::ImageLumaA16(image) => {
                super::super::luma_a16::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageRgb16(image) => {
                super::super::rgb16::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageRgba16(image) => {
                super::super::rgba16::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageRgb32F(image) => {
                super::super::rgb32f::ops::execute_getextrema(image, selection)
            }
            DynamicImage::ImageRgba32F(image) => {
                super::super::rgba32f::ops::execute_getextrema(image, selection)
            }
        }
    }
}

/// Selects exact-mode GetExtrema from the injected CPU context.
pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    context: &ExecutionContext,
) -> Result<PreparedGetExtrema<'a>, PilError> {
    let mode = image.mode();
    let selection = super::super::ops::GET_EXTREMA_SUPPORT
        .for_mode(mode)
        .select_serial(context)?;
    Ok(PreparedGetExtrema {
        image,
        mode,
        selection,
    })
}

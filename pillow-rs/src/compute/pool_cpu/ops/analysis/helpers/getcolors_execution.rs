//! Prepared exact-mode CPU strategy dispatch for GetColors.

use super::getcolors::GetColorsOutput;
use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::super::ops::GET_COLORS_SUPPORT;

/// Generates the exact-mode Default owner for one GetColors implementation.
macro_rules! declare_getcolors_owner {
    ($mode:ident, $image_ty:ty, $implementation:path $(,)?) => {
        pub(crate) fn execute_getcolors(
            image: &$image_ty,
            selection: crate::compute::registry::support::StrategySelection,
            maxcolors: u32,
        ) -> Result<
            crate::compute::pool_cpu::ops::analysis::helpers::getcolors::GetColorsOutput,
            crate::error::PilError,
        > {
            let implementation = selection.implementation;
            if implementation.mode != crate::raster::Mode::$mode
                || implementation.operation
                    != crate::compute::registry::support::OperationId::GetColors
            {
                return Err(crate::error::PilError::InternalError(
                    "GetColors strategy selection did not match its operation and mode".into(),
                ));
            }
            match implementation.architecture {
                crate::compute::Architecture::Default => ($implementation)(maxcolors, image),
                architecture => Err(crate::error::PilError::InternalError(format!(
                    "GetColors has no {:?} {architecture:?} strategy",
                    crate::raster::Mode::$mode,
                ))),
            }
        }
    };
}

pub(crate) use declare_getcolors_owner;

/// Holds exact-mode admission and architecture selection until color counting.
pub(crate) struct PreparedGetColors<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    maxcolors: u32,
    selection: StrategySelection,
}

impl PreparedGetColors<'_> {
    /// Returns the selected exact-mode Default implementation.
    pub(crate) const fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    /// Executes the exact-mode color-count strategy.
    pub(crate) fn execute(self) -> Result<GetColorsOutput, PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "GetColors prepared mode did not match its DynamicImage tag".into(),
            ));
        }
        let implementation = self.selection.implementation;
        if implementation.operation != OperationId::GetColors || implementation.mode != self.mode {
            return Err(PilError::InternalError(
                "GetColors strategy selection did not match its operation and mode".into(),
            ));
        }
        if implementation.architecture != crate::compute::Architecture::Default {
            return Err(PilError::InternalError(format!(
                "GetColors has no {:?} {:?} strategy",
                self.mode, implementation.architecture,
            )));
        }
        execute_getcolors_mode(self.image, self.selection, self.maxcolors)
    }
}

fn execute_getcolors_mode(
    image: &DynamicImage,
    selection: StrategySelection,
    maxcolors: u32,
) -> Result<GetColorsOutput, PilError> {
    match image {
        DynamicImage::One(image) => super::one::ops::execute_getcolors(image, selection, maxcolors),
        DynamicImage::ImageLuma8(image) => {
            super::luma8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageLumaA8(image) => {
            super::luma_a8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageLaPremultiplied8(image) => {
            super::la_premultiplied8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImagePalette8(image) => {
            super::palette8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImagePaletteAlpha8(image) => {
            super::palette_alpha8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageRgb8(image) => {
            super::rgb8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageRgba8(image) => {
            super::rgba8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageRgbPremultiplied8(image) => {
            super::rgb_premultiplied8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageRgbx8(image) => {
            super::rgbx8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageCmyk8(image) => {
            super::cmyk8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageYCbCr8(image) => {
            super::ycbcr8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageLab8(image) => {
            super::lab8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageHsv8(image) => {
            super::hsv8::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageI32(image) => {
            super::i32::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageF32(image) => {
            super::f32::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageI16(_)
        | DynamicImage::ImageI16L(_)
        | DynamicImage::ImageI16B(_)
        | DynamicImage::ImageI16N(_) => {
            Err(PilError::ValueError("image has wrong mode".into()))
        }
        DynamicImage::ImageLumaA16(image) => {
            super::luma_a16::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageRgb16(image) => {
            super::rgb16::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageRgba16(image) => {
            super::rgba16::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageRgb32F(image) => {
            super::rgb32f::ops::execute_getcolors(image, selection, maxcolors)
        }
        DynamicImage::ImageRgba32F(image) => {
            super::rgba32f::ops::execute_getcolors(image, selection, maxcolors)
        }
    }
}

/// Selects exact-mode GetColors from the injected backend and architecture.
pub(crate) fn prepare_getcolors<'a>(
    image: &'a DynamicImage,
    maxcolors: u32,
    context: &ExecutionContext,
) -> Result<PreparedGetColors<'a>, PilError> {
    let mode = image.mode();
    let selection = GET_COLORS_SUPPORT.for_mode(mode).select_serial(context)?;
    Ok(PreparedGetColors { image, mode, maxcolors, selection })
}

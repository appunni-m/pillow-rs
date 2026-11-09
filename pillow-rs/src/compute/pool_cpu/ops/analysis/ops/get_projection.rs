//! GetProjection support contract and prepared CPU strategy.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationSupport, PaletteKnowledge, RequestFacts, StrategyInputs,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) use super::super::helpers::projection::ProjectionOutput;
use super::super::helpers::projection::{
    native_pixel_bytes, project_lab, project_native_bytes, project_stored_words, storage_error,
};
use super::NO_INPUTS;

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::super::one::ops::get_projection_mode_support(),
        Mode::ImageLuma8 => super::super::luma8::ops::get_projection_mode_support(),
        Mode::ImageLumaA8 => super::super::luma_a8::ops::get_projection_mode_support(),
        Mode::ImageLaPremultiplied8 => {
            super::super::la_premultiplied8::ops::get_projection_mode_support()
        }
        Mode::ImagePalette8 => super::super::palette8::ops::get_projection_mode_support(),
        Mode::ImagePaletteAlpha8 => {
            super::super::palette_alpha8::ops::get_projection_mode_support()
        }
        Mode::ImageRgb8 => super::super::rgb8::ops::SUPPORT,
        Mode::ImageRgba8 => super::super::rgba8::ops::get_projection_mode_support(),
        Mode::ImageRgbPremultiplied8 => {
            super::super::rgb_premultiplied8::ops::get_projection_mode_support()
        }
        Mode::ImageRgbx8 => super::super::rgbx8::ops::get_projection_mode_support(),
        Mode::ImageCmyk8 => super::super::cmyk8::ops::SUPPORT,
        Mode::ImageYCbCr8 => super::super::ycbcr8::ops::get_projection_mode_support(),
        Mode::ImageLab8 => super::super::lab8::ops::get_projection_mode_support(),
        Mode::ImageHsv8 => super::super::hsv8::ops::get_projection_mode_support(),
        Mode::ImageI32 => super::super::i32::ops::get_projection_mode_support(),
        Mode::ImageF32 => super::super::f32::ops::get_projection_mode_support(),
        Mode::ImageI16 => super::super::i16::ops::get_projection_mode_support(),
        Mode::ImageI16L => super::super::i16l::ops::get_projection_mode_support(),
        Mode::ImageI16B => super::super::i16b::ops::get_projection_mode_support(),
        Mode::ImageI16N => super::super::i16n::ops::get_projection_mode_support(),
        Mode::ImageLumaA16 => super::super::luma_a16::ops::get_projection_mode_support(),
        Mode::ImageRgb16 => super::super::rgb16::ops::get_projection_mode_support(),
        Mode::ImageRgba16 => super::super::rgba16::ops::get_projection_mode_support(),
        Mode::ImageRgb32F => super::super::rgb32f::ops::get_projection_mode_support(),
        Mode::ImageRgba32F => super::super::rgba32f::ops::get_projection_mode_support(),
    }
}

/// Exhaustive support declaration for direct materialized GetProjection.
/// TODO(pillow-rs#3ptb): verify native axes and exact architecture receipts.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetProjection, NO_INPUTS, classify_mode);

/// Prepared direct projection, retaining the exact mode and architecture choice.
pub(crate) struct PreparedProjection<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedProjection<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<ProjectionOutput, PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "GetProjection prepared mode did not match its DynamicImage tag".into(),
            ));
        }
        match self.mode {
            Mode::ImageRgb8 => {
                let DynamicImage::ImageRgb8(image) = self.image else {
                    return Err(storage_error(self.mode));
                };
                super::super::rgb8::ops::execute(image, self.selection)
            }
            Mode::ImageCmyk8 => {
                let DynamicImage::ImageCmyk8(image) = self.image else {
                    return Err(storage_error(self.mode));
                };
                super::super::cmyk8::ops::execute(image, self.selection)
            }
            Mode::ImageLab8 => {
                let DynamicImage::ImageLab8(image) = self.image else {
                    return Err(storage_error(self.mode));
                };
                Ok(ProjectionOutput::serial(project_lab(image)))
            }
            Mode::ImageI32 | Mode::ImageF32 => {
                let raw = match self.image {
                    DynamicImage::ImageI32(image) | DynamicImage::ImageF32(image) => image.as_raw(),
                    _ => return Err(storage_error(self.mode)),
                };
                Ok(ProjectionOutput::serial(project_stored_words(
                    raw,
                    self.image.width(),
                    self.image.height(),
                )?))
            }
            mode => {
                let bytes_per_pixel = native_pixel_bytes(mode);
                Ok(ProjectionOutput::serial(project_native_bytes(
                    self.image.as_bytes(),
                    self.image.width(),
                    self.image.height(),
                    bytes_per_pixel,
                )?))
            }
        }
    }
}

/// Resolve the exact mode declaration and architecture before scanning pixels.
pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    context: &ExecutionContext,
) -> Result<PreparedProjection<'a>, PilError> {
    let mode = image.mode();
    let selection = select(
        context,
        mode,
        image.width(),
        image.height(),
        image.as_bytes().len(),
    )?;
    Ok(PreparedProjection {
        image,
        mode,
        selection,
    })
}

/// Execute the prepared native RGB8 strategy for terminal CPU fusion.
pub(crate) fn execute_rgb8_selected(
    image: &crate::raster::RgbImage,
    selection: StrategySelection,
) -> Result<ProjectionOutput, PilError> {
    super::super::rgb8::ops::execute(image, selection)
}

/// Execute the prepared native CMYK8 strategy for terminal CPU fusion.
pub(crate) fn execute_cmyk8_selected(
    image: &crate::raster::RgbaImage,
    selection: StrategySelection,
) -> Result<ProjectionOutput, PilError> {
    super::super::cmyk8::ops::execute(image, selection)
}

fn select(
    context: &ExecutionContext,
    mode: Mode,
    width: u32,
    height: u32,
    byte_len: usize,
) -> Result<StrategySelection, PilError> {
    SUPPORT.for_mode(mode).select(
        context,
        StrategyInputs {
            width,
            height,
            byte_len: Some(byte_len),
            palette: PaletteKnowledge::Unknown,
            request: RequestFacts::Unspecified,
        },
    )
}

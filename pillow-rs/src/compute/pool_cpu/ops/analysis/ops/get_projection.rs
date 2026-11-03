//! GetProjection support contract and prepared CPU strategy.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationSupport, PaletteKnowledge, RequestFacts, StrategyInputs,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::super::helpers::projection::{
    native_pixel_bytes, project_lab, project_native_bytes, project_stored_words, storage_error,
};
use super::super::helpers::projection::ProjectionOutput;
use super::{NO_INPUTS, NO_RASTER_OUTPUT};

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::GetProjection,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
        Mode::ImageRgb8 => super::super::rgb8::ops::SUPPORT,
        Mode::ImageCmyk8 => super::super::cmyk8::ops::SUPPORT,
        // NO_PILLOW_SUPPORT: GetProjection scans internal high-depth rasters
        // directly using their stored native samples.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetProjection,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "native high-depth GetProjection extends the Pillow mode contract",
        ),
    }
}

/// Exhaustive support declaration for direct materialized GetProjection.
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
                    DynamicImage::ImageI32(image) | DynamicImage::ImageF32(image) => {
                        image.as_raw()
                    }
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
    let selection = select(context, mode, image.width(), image.height(), image.as_bytes().len())?;
    Ok(PreparedProjection { image, mode, selection })
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

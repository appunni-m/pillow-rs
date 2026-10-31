//! Pad contract and architecture selection for ImageRgb32F.

use crate::compute::registry::support::{ModeSupport, OperationId, OutputModeRule};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::default::pad_impl;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::PadFrame;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(OperationId::Pad, Mode::ImageRgb32F, OutputModeRule::Preserve, super::super::ops::INPUTS, &[], "native high-depth Pad extends the Pillow operation contract");

pub(crate) fn pad(image: &DynamicImage, frame: PadFrame) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgb32F(image) = image else {
        return Err(PilError::InternalError("Pad mode dispatch received a different raster".into()));
    };
    pad_impl::pad(image, frame).map(DynamicImage::ImageRgb32F)
}

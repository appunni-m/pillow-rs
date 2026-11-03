//! Pad contract and architecture selection for ImageRgba16.

use crate::compute::registry::support::{ModeSupport, OperationId, OutputModeRule};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::default::pad_impl;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::PadFrame;

// NO_PILLOW_SUPPORT: Pad preserves this internal high-depth mode and u16 samples.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(OperationId::Pad, Mode::ImageRgba16, OutputModeRule::Preserve, super::super::ops::NATIVE_U16_FILL_INPUTS, &[], "native high-depth Pad extends the Pillow operation contract");

pub(crate) fn pad(image: &DynamicImage, frame: PadFrame) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba16(image) = image else {
        return Err(PilError::InternalError("Pad mode dispatch received a different raster".into()));
    };
    pad_impl::pad(image, frame).map(DynamicImage::ImageRgba16)
}

//! Pad contract and architecture selection for ImageLaPremultiplied8.

use crate::compute::registry::support::{
    ModeSupport, OperationId, OutputModeRule, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::default::pad_impl;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::PadFrame;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(OperationId::Pad, Mode::ImageLaPremultiplied8, OutputModeRule::Preserve, super::super::ops::MODE_ENCODED_FILL_INPUTS, &[]);

pub(crate) fn pad(
    image: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLaPremultiplied8(image) = image else {
        return Err(PilError::InternalError("Pad mode dispatch received a different raster".into()));
    };
    super::super::helpers::strategies::require_default_selection(
        selection,
        Mode::ImageLaPremultiplied8,
    )?;
    pad_impl::pad(image, frame).map(DynamicImage::ImageLaPremultiplied8)
}

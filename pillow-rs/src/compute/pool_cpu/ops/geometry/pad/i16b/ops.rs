//! Pad contract and architecture selection for ImageI16B.

use super::default::pad_impl;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::PadFrame;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OutputModeRule, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Pad,
    Mode::ImageI16B,
    OutputModeRule::Preserve,
    super::super::ops::NATIVE_U16_FILL_INPUTS,
    &[],
);

pub(crate) fn pad(
    image: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI16B(image) = image else {
        return Err(PilError::InternalError(
            "Pad mode dispatch received a different raster".into(),
        ));
    };
    super::super::helpers::strategies::require_default_selection(selection, Mode::ImageI16B)?;
    pad_impl::pad(image, frame).map(DynamicImage::ImageI16B)
}

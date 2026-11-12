//! Pad contract and architecture selection for ImageI16L.

use super::default::pad_impl;
use crate::compute::pool_cpu::ops::geometry::helpers::pad_dimensions::PadFrame;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OutputModeRule, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Pad,
    Mode::ImageI16L,
    OutputModeRule::Preserve,
    super::super::ops::NATIVE_U16_FILL_INPUTS,
    &[],
);

pub(crate) fn pad(
    image: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI16L(image) = image else {
        return Err(PilError::InternalError(
            "Pad mode dispatch received a different raster".into(),
        ));
    };
    super::super::helpers::pad_strategies::require_default_selection(selection, Mode::ImageI16L)?;
    pad_impl::pad(image, frame).map(DynamicImage::ImageI16L)
}

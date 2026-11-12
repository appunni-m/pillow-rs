//! Pad contract and architecture selection for ImageRgba32F.

use super::default::pad_impl;
use crate::compute::pool_cpu::ops::geometry::helpers::pad_dimensions::PadFrame;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OutputModeRule, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

// NO_PILLOW_SUPPORT: Pad preserves this internal high-depth mode and f32 samples.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Pad,
    Mode::ImageRgba32F,
    OutputModeRule::Preserve,
    super::super::ops::NATIVE_F32_FILL_INPUTS,
    &[],
    "native high-depth Pad extends the Pillow operation contract",
);

pub(crate) fn pad(
    image: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba32F(image) = image else {
        return Err(PilError::InternalError(
            "Pad mode dispatch received a different raster".into(),
        ));
    };
    super::super::helpers::pad_strategies::require_default_selection(
        selection,
        Mode::ImageRgba32F,
    )?;
    pad_impl::pad(image, frame).map(DynamicImage::ImageRgba32F)
}

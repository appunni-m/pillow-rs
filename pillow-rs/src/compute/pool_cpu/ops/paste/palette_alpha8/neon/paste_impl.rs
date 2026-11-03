use std::ops::Deref;

use crate::compute::pool_cpu::ops::paste::helpers::{
    plan::BytePlan,
    strategy::l_mask_neon_selected,
    vector,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

pub(crate) fn execute<I>(strategy: BytePlan<'_, I>) -> Result<DynamicImage, PilError>
where
    I: Deref<Target = DynamicImage>,
{
    let (width, height) = strategy.destination_size;
    let bytes = if l_mask_neon_selected(
        &strategy.selection,
        Mode::ImagePaletteAlpha8,
        2,
        1,
    ) {
        super::pa_row::execute(strategy)?
    } else {
        vector::execute::<2, I>(strategy)?
    };
    let image = GrayAlphaImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::InternalError("Paste vector output dimensions were invalid".into())
    })?;
    Ok(DynamicImage::ImagePaletteAlpha8(image))
}

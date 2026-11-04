//! Native RGBa-to-LA output construction.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{GrayAlphaImage, RgbaImage};

pub(crate) fn execute(source: &RgbaImage) -> Result<(GrayAlphaImage, u64, usize), PilError> {
    let source_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let output_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBa Convert source storage did not match its dimensions".into(),
        ));
    }

    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let vector_blocks = super::super::super::helpers::four_byte_to_luma_alpha::append(
        source.as_raw(),
        &mut output,
        crate::raster::Mode::ImageRgbPremultiplied8,
    );
    if output.len() != output_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBa Convert luma-alpha output shape mismatch".into(),
        ));
    }
    let allocation_bytes = output.capacity();
    let output = GrayAlphaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBa Convert luma-alpha output shape mismatch".into()))?;
    Ok((output, vector_blocks, allocation_bytes))
}

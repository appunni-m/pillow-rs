//! Checked native Luma8 source and byte-output dimensions.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::GrayImage;

pub(super) fn checked_layout(
    source: &GrayImage,
    output_channels: u8,
) -> Result<(usize, usize), PilError> {
    let source_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 1)?;
    let output_dims =
        CheckedDims::new_allow_empty(source.width(), source.height(), output_channels)?;
    if source.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "Luma Convert source storage did not match its dimensions".into(),
        ));
    }
    Ok((source_dims.total_pixels(), output_dims.total_bytes()))
}

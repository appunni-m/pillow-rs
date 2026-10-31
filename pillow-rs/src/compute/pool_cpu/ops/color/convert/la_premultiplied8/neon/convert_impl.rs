//! NEON La-to-LA Convert route.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

pub(crate) fn execute(
    source: &GrayAlphaImage,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    if target != Mode::ImageLumaA8 {
        return Err(PilError::InternalError("invalid La Convert target".into()));
    }
    let (bytes, blocks) = super::super::helpers::unpremultiply::execute(source)?;
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    let output = GrayAlphaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("La Convert output shape mismatch".into()))?;
    Ok(DynamicImage::ImageLumaA8(output))
}

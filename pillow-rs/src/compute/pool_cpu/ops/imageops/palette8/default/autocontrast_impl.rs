//! Default Autocontrast strategy over native palette indices.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

// NO_PILLOW_SUPPORT: this transforms P indices directly and retains P identity.
pub(crate) fn execute(input: &GrayImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    let (width, height) = input.dimensions();
    if lut.len() != 256 {
        return Err(PilError::InternalError(
            "P Autocontrast lookup table must contain 256 entries".into(),
        ));
    }
    let identity_lut =
        crate::compute::pool_cpu::ops::imageops::helpers::histogram::lut_is_identity(lut, 1);
    if width == 0 || height == 0 || identity_lut {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(DynamicImage::ImagePalette8(input.clone()));
    }
    let output = input
        .as_raw()
        .iter()
        .map(|&sample| lut[usize::from(sample)])
        .collect();
    crate::compute::record_pipeline_operation_scalar_tail(input.as_raw().len() as u64);
    crate::compute::record_pipeline_operation_path("scalar-lut");
    let output = GrayImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("P Autocontrast buffer mismatch".into()))?;
    Ok(DynamicImage::ImagePalette8(output))
}

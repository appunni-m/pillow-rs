//! CPU Color3DLUT operation routing with explicit Pillow pixel modes.

use crate::error::PilError;
use crate::pipeline::PixelMode;
use crate::raster::DynamicImage;

mod kernels;

/// Apply a Color3DLUT using the source and target modes carried by the
/// pillow-rs wrapper's pipeline operation.
pub(crate) fn op_color3dlut(
    img: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    source_mode: PixelMode,
    target_mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    let rgba = kernels::execute_default(img, size, table, channels, source_mode)?;
    match target_mode {
        PixelMode::RGB => Ok(DynamicImage::ImageRgb8(kernels::project_rgb(rgba))),
        // CMYK and RGBA share four-byte storage, while the Image wrapper keeps
        // their Pillow modes distinct for subsequent pipeline operations.
        PixelMode::RGBA | PixelMode::CMYK => Ok(DynamicImage::ImageRgba8(rgba)),
        _ => Err(PilError::InternalError(
            "validated color3dlut target mode was not RGB, RGBA, or CMYK".into(),
        )),
    }
}

#[cfg(test)]
#[path = "color3dlut/tests.rs"]
mod tests;

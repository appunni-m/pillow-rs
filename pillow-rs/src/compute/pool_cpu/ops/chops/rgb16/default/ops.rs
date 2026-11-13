//! Default rgb16 ImageChops strategy.

use crate::error::PilError;
use crate::raster::DynamicImage;

use super::super::super::ops::ChopsRequest;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &DynamicImage,
    request: ChopsRequest,
    _other: Option<&DynamicImage>,
) -> Result<DynamicImage, PilError> {
    match request {
        ChopsRequest::Offset { x, y } => execute_offset(image, x, y),
        ChopsRequest::Invert => execute_invert(image),
        _ => super::super::super::ops::execute_shared(image, request),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn execute_offset(image: &DynamicImage, x: i32, y: i32) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgb16(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    // NO_PILLOW_SUPPORT: native typed Offset extends Chops to internal high-depth
    // samples.
    let output = super::super::super::helpers::offset::offset_pixels(buffer, x, y);
    Ok(DynamicImage::ImageRgb16(output))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn execute_invert(image: &DynamicImage) -> Result<DynamicImage, PilError> {
    // NO_PILLOW_SUPPORT: native high-depth inversion extends Pillow ImageChops.
    let DynamicImage::ImageRgb16(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    super::super::super::helpers::unary::invert(buffer).map(DynamicImage::ImageRgb16)
}

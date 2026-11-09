//! Default i32 ImageChops strategy.

use crate::error::PilError;
use crate::raster::DynamicImage;

use super::super::super::ops::ChopsRequest;

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

fn execute_offset(image: &DynamicImage, x: i32, y: i32) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI32(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    let output = super::super::super::helpers::offset::offset_pixels(buffer, x, y);
    Ok(DynamicImage::ImageI32(output))
}

fn execute_invert(image: &DynamicImage) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI32(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    super::super::super::helpers::unary::invert(buffer).map(DynamicImage::ImageI32)
}

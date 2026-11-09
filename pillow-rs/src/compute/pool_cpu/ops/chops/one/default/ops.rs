//! Default one ImageChops strategy.

use crate::error::PilError;
use crate::raster::DynamicImage;

use super::super::super::ops::ChopsRequest;

pub(crate) fn execute(
    image: &DynamicImage,
    request: ChopsRequest,
    other: Option<&DynamicImage>,
) -> Result<DynamicImage, PilError> {
    match request {
        ChopsRequest::Offset { x, y } => execute_offset(image, x, y),
        ChopsRequest::Binary(operation) => execute_binary(image, other, operation),
        ChopsRequest::Invert => execute_invert(image),
        _ => super::super::super::ops::execute_shared(image, request),
    }
}

fn execute_binary(
    image: &DynamicImage,
    other: Option<&DynamicImage>,
    operation: super::super::super::helpers::binary::BinaryOperation,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::One(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    let other = match other {
        Some(
            DynamicImage::One(other)
            | DynamicImage::ImageLuma8(other)
            | DynamicImage::ImagePalette8(other),
        ) => other,
        Some(_) => {
            return Err(PilError::InternalError(
                "Chops binary strategy received a different secondary raster variant".into(),
            ));
        }
        None => {
            return Err(PilError::InternalError(
                "Chops binary plan lost its secondary image".into(),
            ));
        }
    };
    let output = super::super::super::helpers::binary::execute(buffer, other, operation)?;
    Ok(DynamicImage::One(output))
}

fn execute_offset(image: &DynamicImage, x: i32, y: i32) -> Result<DynamicImage, PilError> {
    let DynamicImage::One(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    let output = super::super::super::helpers::offset::offset_pixels(buffer, x, y);
    Ok(DynamicImage::One(output))
}

fn execute_invert(image: &DynamicImage) -> Result<DynamicImage, PilError> {
    let DynamicImage::One(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    super::super::super::helpers::unary::invert(buffer).map(DynamicImage::One)
}

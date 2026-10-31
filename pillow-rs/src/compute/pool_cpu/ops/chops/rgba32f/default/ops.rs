//! Default rgba32f ImageChops strategy.

use crate::error::PilError;
use crate::raster::DynamicImage;

use super::super::super::ops::ChopsRequest;

pub(crate) fn execute(
    image: &DynamicImage,
    request: ChopsRequest,
    other: Option<&DynamicImage>,
) -> Result<DynamicImage, PilError> {
    super::super::super::ops::execute_mode(image, super::super::ops::MODE, request, other)
}

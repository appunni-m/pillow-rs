//! Default luma8 ImageChops strategy.

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

pub(crate) fn execute_fused(
    image: &DynamicImage,
    other: &DynamicImage,
) -> Result<DynamicImage, PilError> {
    super::super::super::helpers::fusion::execute_default(
        image,
        other,
        super::super::ops::MODE,
    )
}

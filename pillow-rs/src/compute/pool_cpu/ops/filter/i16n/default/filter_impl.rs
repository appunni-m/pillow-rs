//! Default implementation for Pillow's native-order I;16 mode.

use crate::compute::pool_cpu::ops::filter::helpers::{
    execute_u16_filter3x3, execute_u16_filter5x5,
};
use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma};

type I16Image = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(super) fn execute(
    image: &DynamicImage,
    request: FilterRequest,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI16N(input) = image else {
        return Err(PilError::InternalError(
            "I;16N strategy received a mismatched raster".into(),
        ));
    };
    let (width, height) = (input.width(), input.height());
    let output = match request {
        FilterRequest::Filter3x3 { kernel, scale, offset } => execute_u16_filter3x3(
            input.as_raw(), width as usize, height as usize, &kernel, scale, offset,
        ),
        FilterRequest::Filter5x5 { kernel, scale, offset } => execute_u16_filter5x5(
            input.as_raw(), width as usize, height as usize, &kernel, scale, offset,
        ),
        _ => {
            return Err(PilError::InternalError(
                "I;16N selected a non-Kernel operation".into(),
            ));
        }
    };
    let output = I16Image::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("I;16N Kernel output shape mismatch".into()))?;
    Ok(DynamicImage::ImageI16N(output))
}

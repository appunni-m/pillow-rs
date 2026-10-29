//! Serial Autocontrast implementation for Pillow's native L mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(input: &GrayImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    let (width, height) = input.dimensions();
    if width == 0 || height == 0 {
        return Ok(DynamicImage::ImageLuma8(input.clone()));
    }
    let source = input.as_raw();
    let mut output = source.to_vec();
    if let Some(tables) = super::super::super::helpers::histogram::native_luma_lut_tables(lut) {
        for (source_row, destination_row) in source
            .chunks_exact(width as usize)
            .zip(output.chunks_exact_mut(width as usize))
        {
            super::super::super::helpers::histogram::apply_native_luma_lut_row(
                source_row,
                destination_row,
                &tables,
            );
        }
    } else {
        for (destination, &sample) in output.iter_mut().zip(source) {
            *destination = lut[usize::from(sample)];
        }
    }
    crate::image_utils::raw_bytes_to_image(width, height, output, 1)
}

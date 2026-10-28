//! Native byte transpose dispatch for CPU images.

use super::{
    CheckedDims, DynamicImage, PilError, TRANSPOSE_TILE_SIZE, TransposeMethod, raw_bytes_to_image,
    should_tile_transpose, transpose_bytes_rows, transpose_bytes_serial, transpose_bytes_tiled,
};
use crate::raster::GenericImageView;

fn native_storage_matches(img: &DynamicImage, channels: usize) -> bool {
    matches!(
        (channels, img),
        (1, DynamicImage::ImageLuma8(_))
            | (2, DynamicImage::ImageLumaA8(_))
            | (3, DynamicImage::ImageRgb8(_))
            | (4, DynamicImage::ImageRgba8(_))
    )
}

pub(super) fn execute_flip(
    img: &DynamicImage,
    method: &TransposeMethod,
    channels: usize,
) -> Result<Option<DynamicImage>, PilError> {
    if !matches!(
        method,
        TransposeMethod::FlipLeftRight
            | TransposeMethod::FlipTopBottom
            | TransposeMethod::Rotate180
    ) || !native_storage_matches(img, channels)
    {
        return Ok(None);
    }
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let dimensions = CheckedDims::new(width, height, channels as u8)?;
    let mut output = dimensions.alloc_buffer();
    let reverse_x = !matches!(method, TransposeMethod::FlipTopBottom);
    let reverse_y = !matches!(method, TransposeMethod::FlipLeftRight);
    macro_rules! copy_rows {
        ($channels:literal) => {
            transpose_bytes_rows::<$channels>(
                img.as_bytes(),
                &mut output,
                &dimensions,
                reverse_x,
                reverse_y,
            )
        };
    }
    match channels {
        1 => copy_rows!(1),
        2 => copy_rows!(2),
        3 => copy_rows!(3),
        4 => copy_rows!(4),
        _ => unreachable!("native byte image has one to four channels"),
    }
    raw_bytes_to_image(width, height, output, channels).map(Some)
}

pub(super) fn execute_axis_swap(
    img: &DynamicImage,
    method: &TransposeMethod,
    channels: usize,
) -> Result<Option<DynamicImage>, PilError> {
    if !matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    ) || !native_storage_matches(img, channels)
    {
        return Ok(None);
    }
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 || (width == 1 && height == 1) {
        return Ok(None);
    }
    let output_dims = CheckedDims::new(height, width, channels as u8)?;
    let mut output = output_dims.alloc_buffer();
    #[cfg(all(target_arch = "x86_64", not(feature = "parallel")))]
    if super::transpose_arch::try_rotate270_rgba_tiled(
        img,
        method,
        &mut output,
        width,
        height,
        channels,
    ) {
        return raw_bytes_to_image(height, width, output, channels).map(Some);
    }

    #[cfg(feature = "parallel")]
    let tile_size = TRANSPOSE_TILE_SIZE;
    #[cfg(not(feature = "parallel"))]
    let tile_size = if channels == 4 {
        super::TRANSPOSE_RGBA_TILE_SIZE
    } else {
        TRANSPOSE_TILE_SIZE
    };
    transpose_channels(img, &mut output, width, height, channels, method, tile_size);
    raw_bytes_to_image(height, width, output, channels).map(Some)
}

#[inline(always)]
fn transpose_channels(
    img: &DynamicImage,
    output: &mut [u8],
    width: u32,
    height: u32,
    channels: usize,
    method: &TransposeMethod,
    tile_size: u32,
) {
    macro_rules! transpose_bytes {
        ($channels:literal) => {
            if should_tile_transpose(width, height) {
                transpose_bytes_tiled::<$channels>(
                    img.as_bytes(),
                    output,
                    width,
                    height,
                    method,
                    tile_size,
                );
            } else {
                transpose_bytes_serial::<$channels>(img.as_bytes(), output, width, height, method);
            }
        };
    }
    match channels {
        1 => transpose_bytes!(1),
        2 => transpose_bytes!(2),
        3 => transpose_bytes!(3),
        4 => transpose_bytes!(4),
        _ => unreachable!("native byte image has one to four channels"),
    }
}

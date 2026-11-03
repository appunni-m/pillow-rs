//! Shared typed Default kernels for exact-mode Offset strategies.

use crate::raster::buffer::ImageBuffer;
use crate::raster::color::Luma;
use crate::raster::traits::Pixel;

/// Wrap typed pixels by Pillow's ordinary pixel-coordinate Offset rule.
pub(crate) fn offset_pixels<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    xoffset: i32,
    yoffset: i32,
) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return ImageBuffer::new(width, height);
    }

    let source_x = (-(i64::from(xoffset))).rem_euclid(i64::from(width)) as u32;
    let source_y = (-(i64::from(yoffset))).rem_euclid(i64::from(height)) as u32;
    let mut output = ImageBuffer::from_fn(width, height, |x, y| {
        *image.get_pixel((x + source_x) % width, (y + source_y) % height)
    });
    let width_usize = width as usize;
    let height_usize = height as usize;
    let xshift = source_x as usize;
    let yshift = source_y as usize;
    output
        .remap_hidden_pixel_lanes_from(image, |index| {
            let output_x = index % width_usize;
            let output_y = index / width_usize;
            let source_x = ((output_x as u64 + xshift as u64) % width_usize as u64) as usize;
            let source_y = ((output_y as u64 + yshift as u64) % height_usize as u64) as usize;
            source_y * width_usize + source_x
        })
        .expect("offset maps valid source pixels");
    output
}

/// Preserve Pillow's byte-oriented I;16 Offset behavior for typed u16 buffers.
/// Each row rotates its first `width` bytes and leaves the second half zero;
/// only `I;16B` asks the mode owner for big-endian byte ordering.
pub(crate) fn offset_i16_samples(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    xoffset: i32,
    yoffset: i32,
    big_endian: bool,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return ImageBuffer::new(width, height);
    }

    let mut source = Vec::with_capacity(image.as_raw().len() * 2);
    for &sample in image.as_raw() {
        let bytes = if big_endian {
            sample.to_be_bytes()
        } else {
            sample.to_ne_bytes()
        };
        source.extend_from_slice(&bytes);
    }

    let row_bytes = width as usize * 2;
    let xshift = (-(i64::from(xoffset))).rem_euclid(i64::from(width)) as usize;
    let yshift = (-(i64::from(yoffset))).rem_euclid(i64::from(height)) as usize;
    let mut destination = vec![0_u8; source.len()];
    for y in 0..height as usize {
        let source_y = (y + yshift) % height as usize;
        let source_row = source_y * row_bytes;
        let destination_row = y * row_bytes;
        for x in 0..width as usize {
            let source_x = (x + xshift) % width as usize;
            destination[destination_row + x] = source[source_row + source_x];
        }
    }

    let pixels = destination
        .chunks_exact(2)
        .map(|bytes| {
            let bytes = [bytes[0], bytes[1]];
            if big_endian {
                u16::from_be_bytes(bytes)
            } else {
                u16::from_ne_bytes(bytes)
            }
        })
        .collect();
    ImageBuffer::from_vec(width, height, pixels).expect("offset preserves the source dimensions")
}

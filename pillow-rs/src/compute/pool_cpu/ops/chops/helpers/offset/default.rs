//! Shared typed Default kernels for exact-mode Offset strategies.

use crate::raster::buffer::ImageBuffer;
use crate::raster::traits::Pixel;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

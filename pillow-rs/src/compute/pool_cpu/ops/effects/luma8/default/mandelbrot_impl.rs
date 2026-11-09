//! Serial Default Mandelbrot implementation for exact L output.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<GeneratedImage, PilError> {
    let (width, height) = size;
    let (x0, y0, x1, y1) = extent;
    let dx = x1 - x0;
    let dy = y1 - y0;
    // Pillow divides by width - 1 and height - 1 without a degenerate-axis
    // guard. A one-pixel axis produces NaN coordinates and an all-zero axis.
    let dr = dx / (width - 1) as f64;
    let di = dy / (height - 1) as f64;
    let margin = super::super::helpers::mandelbrot::membership_margin(quality);
    let mut pixels = CheckedDims::new(width, height, 1)?.alloc_buffer();

    for y in 0..height {
        let row_start = (y * width) as usize;
        let ci = y as f64 * di + y0;
        for x in 0..width {
            let cr = x as f64 * dr + x0;
            pixels[row_start + x as usize] =
                if super::super::helpers::mandelbrot::known_interior(cr, ci, margin) {
                    0
                } else {
                    super::super::helpers::mandelbrot::pixel_value(cr, ci, quality)
                };
        }
    }

    let image = GrayImage::from_raw(width, height, pixels).ok_or_else(|| {
        PilError::InternalError("EffectMandelbrot L buffer shape mismatch".into())
    })?;
    Ok(GeneratedImage {
        image: DynamicImage::ImageLuma8(image),
        vector_blocks: 0,
        scalar_tail: 0,
    })
}

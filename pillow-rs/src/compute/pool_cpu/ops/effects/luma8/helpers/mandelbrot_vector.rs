//! Existing eight-lane Mandelbrot loop over exact native L samples.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};
use wide::f64x8;

pub(crate) fn execute(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<GeneratedImage, PilError> {
    const LANES: usize = 8;
    let (width, height) = size;
    let (x0, y0, x1, y1) = extent;
    let dx = x1 - x0;
    let dy = y1 - y0;
    // Pillow divides by width - 1 and height - 1 without a degenerate-axis
    // guard. A one-pixel axis produces NaN coordinates and an all-zero axis.
    let dr = dx / (width - 1) as f64;
    let di = dy / (height - 1) as f64;
    let margin = super::mandelbrot::membership_margin(quality);
    let mut pixels = CheckedDims::new(width, height, 1)?.alloc_buffer();
    let vector_width = width as usize / LANES * LANES;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for y in 0..height as usize {
        let row_start = y * width as usize;
        let ci = y as f64 * di + y0;
        let ci_vector = f64x8::splat(ci);
        for x_start in (0..vector_width).step_by(LANES) {
            let mut real_lanes = [0.0f64; LANES];
            let mut active = [false; LANES];
            let mut active_count = 0usize;
            for lane in 0..LANES {
                let x = x_start + lane;
                let cr = x as f64 * dr + x0;
                real_lanes[lane] = cr;
                if super::mandelbrot::known_interior(cr, ci, margin) {
                    pixels[row_start + x] = 0;
                } else {
                    active[lane] = true;
                    active_count += 1;
                }
            }
            if active_count == 0 {
                continue;
            }

            let cr_vector = f64x8::new(real_lanes);
            let mut zx = f64x8::splat(0.0);
            let mut zy = f64x8::splat(0.0);
            let mut zx2 = f64x8::splat(0.0);
            let mut zy2 = f64x8::splat(0.0);
            let mut iteration = 1i32;
            loop {
                // Keep separate wide operations in Pillow's scalar order; do
                // not fuse the recurrence into multiply-add instructions.
                zy = (f64x8::splat(2.0) * zx) * zy + ci_vector;
                zx = zx2 - zy2 + cr_vector;
                zx2 = zx * zx;
                zy2 = zy * zy;
                let magnitudes = (zx2 + zy2).to_array();
                for lane in 0..LANES {
                    if active[lane] && magnitudes[lane] > 100.0 {
                        pixels[row_start + x_start + lane] = (iteration * 255 / quality) as u8;
                        active[lane] = false;
                        active_count -= 1;
                    }
                }
                vector_blocks = vector_blocks.saturating_add(1);
                if active_count == 0 {
                    break;
                }
                if iteration > quality {
                    for (lane, lane_active) in active.iter().enumerate() {
                        if *lane_active {
                            pixels[row_start + x_start + lane] = 0;
                        }
                    }
                    break;
                }
                iteration += 1;
            }
        }

        for x in vector_width..width as usize {
            let cr = x as f64 * dr + x0;
            pixels[row_start + x] = if super::mandelbrot::known_interior(cr, ci, margin) {
                0
            } else {
                super::mandelbrot::pixel_value(cr, ci, quality)
            };
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }

    let image = GrayImage::from_raw(width, height, pixels)
        .ok_or_else(|| PilError::InternalError("EffectMandelbrot vector buffer shape mismatch".into()))?;
    Ok(GeneratedImage {
        image: DynamicImage::ImageLuma8(image),
        vector_blocks,
        scalar_tail,
    })
}

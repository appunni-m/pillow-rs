//! SIMD adapter for the Mandelbrot image generator.

use super::{CheckedDims, simd_unsupported};
use crate::error::PilError;
use crate::raster::DynamicImage;
use wide::f64x8;

/// Generate Pillow's native-L Mandelbrot raster with eight double-precision
/// SIMD lanes. Consecutive x coordinates share an iteration counter while
/// escaped lanes are masked in scalar control; lane arithmetic remains vector
/// operations and preserves Pillow's exact recurrence order.
pub(crate) fn simd_mandelbrot_generate(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<DynamicImage, PilError> {
    const LANES: usize = 8;
    let (width, height) = size;
    if width < LANES as u32 || height == 0 || quality < 2 {
        return Err(simd_unsupported("Mandelbrot"));
    }

    let (x0, y0, x1, y1) = extent;
    let pixel_width = x1 - x0;
    let pixel_height = y1 - y0;
    if pixel_width < 0.0 || pixel_height < 0.0 {
        return Err(simd_unsupported("Mandelbrot"));
    }
    let dr = pixel_width / (width - 1) as f64;
    let di = pixel_height / (height - 1) as f64;
    let margin = crate::ops::module_fns::mandelbrot_membership_margin(quality);
    let mut output = CheckedDims::new(width, height, 1)?.alloc_buffer();
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
                if crate::ops::module_fns::mandelbrot_known_interior(cr, ci, margin) {
                    output[row_start + x] = 0;
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
                // These separate wide operations intentionally match Pillow's
                // scalar sequence; do not fuse them into multiply-adds.
                zy = (f64x8::splat(2.0) * zx) * zy + ci_vector;
                zx = zx2 - zy2 + cr_vector;
                zx2 = zx * zx;
                zy2 = zy * zy;
                let magnitudes = (zx2 + zy2).to_array();
                for lane in 0..LANES {
                    if active[lane] && magnitudes[lane] > 100.0 {
                        output[row_start + x_start + lane] = (iteration * 255 / quality) as u8;
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
                            output[row_start + x_start + lane] = 0;
                        }
                    }
                    break;
                }
                iteration += 1;
            }
        }

        for x in vector_width..width as usize {
            let cr = x as f64 * dr + x0;
            output[row_start + x] =
                if crate::ops::module_fns::mandelbrot_known_interior(cr, ci, margin) {
                    0
                } else {
                    crate::ops::module_fns::mandelbrot_pixel_value(cr, ci, quality)
                };
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, 1)
}

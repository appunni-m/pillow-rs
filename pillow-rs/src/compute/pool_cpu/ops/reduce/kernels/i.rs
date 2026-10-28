//! Pillow I-mode reduction kernel over the wrapper's four-byte scalar carrier.

use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;
use crate::ops::pil_resize::{i32_samples_from_le_bytes, round_up};
use crate::raster::DynamicImage;

pub(in crate::compute::pool_cpu::ops) fn reduce_i_thumbnail(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    factor_x: u32,
    factor_y: u32,
) -> Result<DynamicImage, PilError> {
    // I's four-byte image is a scalar carrier here; borrow it to avoid a copy.
    let (src_w, src_h, source_image) = match img {
        DynamicImage::ImageRgba8(image) => (image.width(), image.height(), image),
        _ => {
            return Err(PilError::ValueError(
                "I-mode thumbnail reduction requires four-byte scalar storage".into(),
            ));
        }
    };
    let source_bytes = source_image.as_raw();
    let source_samples = i32_samples_from_le_bytes(
        source_bytes,
        source_bytes.len() / std::mem::size_of::<i32>(),
    );
    let source_stride = src_w as usize;
    let sample_i32 = |x: u32, y: u32| source_samples[y as usize * source_stride + x as usize];
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);

    // Pillow's common native-I thumbnail reducing-gap uses complete 2×2
    // quartets. Traverse each pair of source rows contiguously and preserve
    // Reduce.c's wrapping INT32 additions before the f64 average. Partial
    // blocks and other factors stay on the generic path below.
    if factor_x == 2 && factor_y == 2 && src_w > 0 && src_h > 0 && src_w % 2 == 0 && src_h % 2 == 0
    {
        let row_pair_len = source_stride
            .checked_mul(2)
            .ok_or_else(|| PilError::InternalError("I thumbnail row size overflow".into()))?;
        for rows in source_samples.chunks_exact(row_pair_len) {
            let (top, bottom) = rows.split_at(source_stride);
            for (top_pair, bottom_pair) in top.chunks_exact(2).zip(bottom.chunks_exact(2)) {
                let quartet = top_pair[0]
                    .wrapping_add(top_pair[1])
                    .wrapping_add(bottom_pair[0])
                    .wrapping_add(bottom_pair[1]);
                let value = round_up(f64::from(quartet) / 4.0) as i32;
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        return raw_bytes_to_image(dst_w, dst_h, out, 4);
    }

    let main_width = src_w / factor_x;
    let main_height = src_h / factor_y;
    for y in 0..dst_h {
        let source_y = y * factor_y;
        let block_h = factor_y.min(src_h - source_y);
        for x in 0..dst_w {
            let source_x = x * factor_x;
            let block_w = factor_x.min(src_w - source_x);
            // ImagingReduceNxN_32bpc adds pairs/quartets while the samples
            // are still INT32, so each intermediate addition wraps at 32
            // bits before the result is promoted to the double accumulator.
            // Its corner helper instead adds each partial-edge sample
            // directly to the double accumulator. Preserve those two paths
            // separately; summing every sample as i64 changes overflow cases.
            let mut sum = 0.0f64;
            if x < main_width && y < main_height {
                let value = |offset_x: u32, offset_y: u32| {
                    sample_i32(source_x + offset_x, source_y + offset_y)
                };
                let mut dy = 0;
                while dy + 1 < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let quartet = value(dx, dy)
                            .wrapping_add(value(dx + 1, dy))
                            .wrapping_add(value(dx, dy + 1))
                            .wrapping_add(value(dx + 1, dy + 1));
                        sum += f64::from(quartet);
                        dx += 2;
                    }
                    if dx < block_w {
                        let pair = value(dx, dy).wrapping_add(value(dx, dy + 1));
                        sum += f64::from(pair);
                    }
                    dy += 2;
                }
                if dy < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let pair = value(dx, dy).wrapping_add(value(dx + 1, dy));
                        sum += f64::from(pair);
                        dx += 2;
                    }
                    if dx < block_w {
                        sum += f64::from(value(dx, dy));
                    }
                }
            } else {
                for dy in 0..block_h {
                    for dx in 0..block_w {
                        sum += f64::from(sample_i32(source_x + dx, source_y + dy));
                    }
                }
            }
            let value = round_up(sum / f64::from(block_w * block_h)) as i32;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, out, 4)
}

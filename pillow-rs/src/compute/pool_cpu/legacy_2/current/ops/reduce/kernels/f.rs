//! Pillow F-mode reduction kernel over the wrapper's four-byte scalar carrier.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::DynamicImage;

pub(super) fn reduce_f_thumbnail_2x2_samples(
    dst_w: u32,
    dst_h: u32,
    source_width: u32,
    output_bytes: usize,
    mut sample_f32: impl FnMut(usize) -> f32,
) -> Result<DynamicImage, PilError> {
    let source_width = source_width as usize;
    let output_row_bytes = (dst_w as usize)
        .checked_mul(4)
        .ok_or_else(|| PilError::InternalError("F thumbnail row size overflow".into()))?;
    let mut output = vec![0; output_bytes];
    for y in 0..dst_h as usize {
        let top_row = y * 2 * source_width;
        let bottom_row = top_row + source_width;
        let output_row_start = y * output_row_bytes;
        let output_row_end = output_row_start + output_row_bytes;
        let output_row = &mut output[output_row_start..output_row_end];
        for (x, output_sample) in output_row.chunks_exact_mut(4).enumerate() {
            let source_x = x * 2;
            let top_left = sample_f32(top_row + source_x);
            let top_right = sample_f32(top_row + source_x + 1);
            let bottom_left = sample_f32(bottom_row + source_x);
            let bottom_right = sample_f32(bottom_row + source_x + 1);
            // Pillow adds this quartet in f32 order before promoting it to f64.
            let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
            let mut sum = 0.0f64;
            sum += f64::from(quartet);
            let value = (sum * 0.25) as f32;
            output_sample.copy_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, output, 4)
}

pub(in crate::compute::pool_cpu::ops) fn reduce_f_thumbnail(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    factor_x: u32,
    factor_y: u32,
) -> Result<DynamicImage, PilError> {
    // F's four-byte image is a scalar carrier here; borrow it to avoid a copy.
    let (src_w, src_h, source_image) = match img {
        DynamicImage::ImageRgba8(image) => (image.width(), image.height(), image),
        _ => {
            return Err(PilError::ValueError(
                "F-mode thumbnail reduction requires four-byte scalar storage".into(),
            ));
        }
    };
    // `thumbnail` commonly applies a 2×2 reducing-gap pass before its F32
    // resize. Keep that frequent interior case branch-free: read the four
    // packed scalar samples by row offset, preserve Pillow's f32 quartet
    // order, then promote only the completed quartet to the f64 accumulator.
    if factor_x == 2
        && factor_y == 2
        && src_w % 2 == 0
        && src_h % 2 == 0
        && dst_w == src_w / 2
        && dst_h == src_h / 2
    {
        let source_dims = CheckedDims::new_allow_empty(src_w, src_h, 4)?;
        let source_bytes = source_image.as_raw();
        if source_bytes.len() != source_dims.total_bytes() {
            return Err(PilError::InternalError(
                "F-mode thumbnail source buffer shape mismatch".into(),
            ));
        }
        let output_dims = CheckedDims::new_allow_empty(dst_w, dst_h, 4)?;
        let output_bytes = output_dims.total_bytes();
        if let Some(samples) = crate::ops::pil_resize::f32_samples_borrowed_from_le_bytes(
            source_bytes,
            source_dims.total_bytes() / 4,
        ) {
            return reduce_f_thumbnail_2x2_samples(dst_w, dst_h, src_w, output_bytes, |index| {
                samples[index]
            });
        }
        return reduce_f_thumbnail_2x2_samples(dst_w, dst_h, src_w, output_bytes, |index| {
            let byte_index = index * 4;
            f32::from_le_bytes([
                source_bytes[byte_index],
                source_bytes[byte_index + 1],
                source_bytes[byte_index + 2],
                source_bytes[byte_index + 3],
            ])
        });
    }

    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);
    for y in 0..dst_h {
        reduce_f_thumbnail_general_row(source_image, dst_w, factor_x, factor_y, y, &mut out);
    }
    raw_bytes_to_image(dst_w, dst_h, out, 4)
}

fn reduce_f_thumbnail_general_row(
    source_image: &crate::raster::RgbaImage,
    dst_w: u32,
    factor_x: u32,
    factor_y: u32,
    y: u32,
    out: &mut Vec<u8>,
) {
    let src_w = source_image.width();
    let src_h = source_image.height();
    let sample_f32 = |x: u32, y: u32| {
        let sample = source_image.get_pixel(x, y);
        f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]])
    };
    let main_width = src_w / factor_x;
    let main_height = src_h / factor_y;
    let source_y = y * factor_y;
    let block_h = factor_y.min(src_h - source_y);
    for x in 0..dst_w {
        let source_x = x * factor_x;
        let block_w = factor_x.min(src_w - source_x);
        // `ImagingReduceNxN_32bpc` uses a double accumulator, but its
        // interior 2x2 groups are formed by float additions before being
        // promoted to double. The corner helper handles partial right,
        // bottom, and bottom-right blocks as scalar float values. Keep
        // those two paths distinct: a flat f32 sum drifts on constants,
        // while a flat f64 sum differs on heterogeneous blocks.
        let mut sum = 0.0f64;
        if x < main_width && y < main_height {
            let mut dy = 0;
            while dy + 1 < block_h {
                let mut dx = 0;
                while dx + 1 < block_w {
                    let value = |offset_x: u32, offset_y: u32| {
                        sample_f32(source_x + offset_x, source_y + offset_y)
                    };
                    let top_left = value(dx, dy);
                    let top_right = value(dx + 1, dy);
                    let bottom_left = value(dx, dy + 1);
                    let bottom_right = value(dx + 1, dy + 1);
                    let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
                    sum += f64::from(quartet);
                    dx += 2;
                }
                if dx < block_w {
                    let value = |offset_y: u32| sample_f32(source_x + dx, source_y + offset_y);
                    sum += f64::from(value(dy) + value(dy + 1));
                }
                dy += 2;
            }
            if dy < block_h {
                let mut dx = 0;
                while dx + 1 < block_w {
                    let value = |offset_x: u32| sample_f32(source_x + offset_x, source_y + dy);
                    sum += f64::from(value(dx) + value(dx + 1));
                    dx += 2;
                }
                if dx < block_w {
                    sum += f64::from(sample_f32(source_x + dx, source_y + dy));
                }
            }
        } else {
            for dy in 0..block_h {
                for dx in 0..block_w {
                    sum += f64::from(sample_f32(source_x + dx, source_y + dy));
                }
            }
        }
        let value = (sum * (1.0 / f64::from(block_w * block_h))) as f32;
        out.extend_from_slice(&value.to_le_bytes());
    }
}

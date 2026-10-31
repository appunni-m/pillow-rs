use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::filter::helpers::SMALL_RANK_AREA;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};
use super::super::helpers::native_uniform_f_mode_image;

pub(super) fn rank_filter_f_requires_legacy_sort(raw: &[u8]) -> bool {
    let mut saw_negative_zero = false;
    let mut saw_positive_zero = false;
    for bytes in raw.chunks_exact(4) {
        let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if value.is_nan() {
            // The existing partial_cmp fallback treats every NaN as equal to
            // every value. Keep its exact sort behavior for these inputs.
            return true;
        }
        if value == 0.0 {
            saw_negative_zero |= value.is_sign_negative();
            saw_positive_zero |= !value.is_sign_negative();
            if saw_negative_zero && saw_positive_zero {
                // partial_cmp considers the two zero signs equal, though
                // their output bytes differ.
                return true;
            }
        }
    }
    false
}

#[inline]
pub(super) fn select_rank_filter_f_value(
    values: &mut [f32],
    rank: usize,
    legacy_sort: bool,
) -> f32 {
    let compare = |a: &f32, b: &f32| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal);
    if legacy_sort {
        values.sort_unstable_by(compare);
        values[rank]
    } else {
        *values.select_nth_unstable_by(rank, compare).1
    }
}


fn rank_filter_f_large_serial(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    half: i32,
    area: usize,
    rank: usize,
    legacy_sort: bool,
) {
    let mut values = Vec::with_capacity(area);
    for y in 0..h {
        for x in 0..w {
            values.clear();
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w - 1);
                    let sy = (y + dy).clamp(0, h - 1);
                    let base = (sy * w + sx) as usize * 4;
                    values.push(f32::from_le_bytes([
                        raw[base],
                        raw[base + 1],
                        raw[base + 2],
                        raw[base + 3],
                    ]));
                }
            }
            let selected = select_rank_filter_f_value(&mut values, rank, legacy_sort);
            let out_base = (y * w + x) as usize * 4;
            out[out_base..out_base + 4].copy_from_slice(&selected.to_le_bytes());
        }
    }
}



/// Precomputed dimensions and neighborhood bounds for one rank-filter operation.
#[derive(Clone, Copy)]
pub(super) struct RankFilterWindow {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) width_i32: i32,
    pub(super) height_i32: i32,
    pub(super) half: i32,
    pub(super) area: usize,
    pub(super) rank: usize,
}

/// Select an F-mode rank from a small, fixed-size neighborhood.
fn rank_filter_f_small(
    raw: &[u8],
    output: &mut [u8],
    width: i32,
    height: i32,
    half: i32,
    area: usize,
    rank: usize,
) {
    for y in 0..height {
        for x in 0..width {
            let mut values = [0f32; SMALL_RANK_AREA];
            let mut value_index = 0usize;
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, width - 1);
                    let sy = (y + dy).clamp(0, height - 1);
                    let base = (sy * width + sx) as usize * 4;
                    values[value_index] = f32::from_le_bytes([
                        raw[base],
                        raw[base + 1],
                        raw[base + 2],
                        raw[base + 3],
                    ]);
                    value_index += 1;
                }
            }
            values[..area]
                .sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let out_base = (y * width + x) as usize * 4;
            output[out_base..out_base + 4].copy_from_slice(&values[rank].to_le_bytes());
        }
    }
}

/// Apply an F-mode rank filter to little-endian f32 samples in four-byte storage.
fn rank_filter_f_impl(
    img: &DynamicImage,
    window: RankFilterWindow,
) -> Result<DynamicImage, PilError> {
    let RankFilterWindow {
        width: w_u32,
        height: h_u32,
        width_i32: w,
        height_i32: h,
        half,
        area,
        rank,
    } = window;

    // F-mode stores each scalar as one little-endian f32 in the four-byte
    // carrier. Borrow that storage directly and retain the accessor only for
    // mismatched internal variants.
    let DynamicImage::ImageF32(image) = img else {
        return Err(PilError::InternalError(
            "RankFilter F strategy received a non-F raster".into(),
        ));
    };
    let raw = image.as_raw();
    if area > 1 && native_uniform_f_mode_image(raw, w_u32 as usize, h_u32 as usize) {
        return Ok(img.clone());
    }
    let mut out = CheckedDims::new(w as u32, h as u32, 4)?.alloc_buffer();
    if area <= SMALL_RANK_AREA {
        rank_filter_f_small(raw, &mut out, w, h, half, area, rank);
    } else {
        let legacy_sort = rank_filter_f_requires_legacy_sort(raw);
        
        
        rank_filter_f_large_serial(raw, &mut out, w, h, half, area, rank, legacy_sort);
    }
    let result = DynamicImage::ImageF32(
        RgbaImage::from_raw(w_u32, h_u32, out)
            .ok_or_else(|| PilError::ValueError("rank_filter_impl(F): buffer error".into()))?,
    );
    Ok(result)
}

pub(super) fn execute(
    image: &DynamicImage,
    size: u32,
    rank: u32,
) -> Result<DynamicImage, PilError> {
    let width = image.width();
    let height = image.height();
    let area = (size * size) as usize;
    rank_filter_f_impl(
        image,
        RankFilterWindow {
            width,
            height,
            width_i32: width as i32,
            height_i32: height as i32,
            half: (size / 2) as i32,
            area,
            rank: rank as usize,
        },
    )
}

#[cfg(test)]
#[path = "rank_tests.rs"]
mod tests;

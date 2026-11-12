use super::super::helpers::native_uniform_f_mode_image;
use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::filter::helpers::SMALL_RANK_AREA;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

// VERIFY: source=reviewed@2026-11-13; parity=pass@migration-parity-26066f371e8c4e0a80be534279849518; benchmark=pending
/// Selects the same F32 rank sample as Pillow's in-place partition algorithm.
#[inline]
fn select_rank_filter_f_pillow(values: &mut [f32], rank: usize) -> f32 {
    let target = rank as isize;
    let mut left = 0_isize;
    let mut right = values.len() as isize - 1;
    while left < right {
        let pivot = values[target as usize];
        let mut i = left;
        let mut j = right;
        loop {
            while i <= right && values[i as usize] < pivot {
                i += 1;
            }
            while j >= left && pivot < values[j as usize] {
                j -= 1;
            }
            if i <= j {
                values.swap(i as usize, j as usize);
                i += 1;
                j -= 1;
            }
            if i > j {
                break;
            }
        }
        if j < target {
            left = i;
        }
        if target < i {
            right = j;
        }
    }
    values[target as usize]
}

// VERIFY: source=reviewed@2026-11-13; parity=pass@migration-parity-26066f371e8c4e0a80be534279849518; benchmark=pending
/// Reuses one neighborhood buffer for every pixel in a large F-mode window.
fn rank_filter_f_large_serial(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    half: i32,
    area: usize,
    rank: usize,
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
            let selected = select_rank_filter_f_pillow(&mut values, rank);
            let out_base = (y * w + x) as usize * 4;
            out[out_base..out_base + 4].copy_from_slice(&selected.to_le_bytes());
        }
    }
}

/// Precomputed dimensions and neighborhood bounds for one rank-filter operation.
/// The sampling helpers clamp edges to fuse Pillow's expansion and output crop.
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

// VERIFY: source=reviewed@2026-11-13; parity=pass@migration-parity-26066f371e8c4e0a80be534279849518; benchmark=pending
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
            let selected = select_rank_filter_f_pillow(&mut values[..area], rank);
            let out_base = (y * width + x) as usize * 4;
            output[out_base..out_base + 4].copy_from_slice(&selected.to_le_bytes());
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pass@migration-parity-26066f371e8c4e0a80be534279849518; benchmark=pending
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
        rank_filter_f_large_serial(raw, &mut out, w, h, half, area, rank);
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

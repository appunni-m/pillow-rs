//! Scalar fallback for scalar orientation, padding, and crop operations.

use super::resize::{resize, round_positive_ties_even};
use crate::pipeline::TransposeMethod;

/// Flip: vertical coordinate remap. output[y][x] = input[H-1-y][x]
#[inline]
pub fn flip(pixels: &mut [u32], w: u32, h: u32, mode: u32) {
    let has_a = super::mode_has_alpha(mode);
    let row_size = w as usize;
    // Process top half, swap with bottom half
    for y in 0..(h as usize / 2) {
        let top = y * row_size;
        let bottom = (h as usize - 1 - y) * row_size;
        for x in 0..row_size {
            let t = pixels[top + x];
            let b = pixels[bottom + x];
            // Clamp alpha for non-alpha modes during swap
            let mask = if has_a { 0xFFFF_FFFF } else { 0x00FF_FFFF };
            pixels[top + x] = b & mask;
            pixels[bottom + x] = t & mask;
        }
    }
    // Middle row (odd height): clamp alpha
    if h % 2 == 1 && !has_a {
        let mid = (h as usize / 2) * row_size;
        for x in 0..row_size {
            pixels[mid + x] &= 0x00FF_FFFF;
        }
    }
}

/// Duplicate: identity copy with mode-aware alpha clamping.
/// Only preserves alpha byte for modes with an alpha channel.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn duplicate(pixels: &mut [u32], mode: u32) {
    let has_a = super::mode_has_alpha(mode);
    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;
        let out_a = if has_a { a } else { 0xFF00_0000 };
        *p = r | (g << 8) | (b << 16) | out_a;
    }
}

// ── Spatial operations (mirror, transpose, box_blur) ───────────────────────

/// Mirror: horizontal coordinate remap. output[y][x] = input[y][W-1-x]
/// Mode-aware: processes each row by swapping pairs from ends toward center.
/// For L/LA modes: G and B are set to R since only R carries luma.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn mirror(pixels: &mut [u32], w: u32, h: u32, mode: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let row_size = w as usize;
    let half_w = w as usize / 2;
    for y in 0..(h as usize) {
        let row = y * row_size;
        for x in 0..half_w {
            let left = row + x;
            let right = row + (row_size - 1 - x);
            let l = pixels[left];
            let r = pixels[right];

            if has_gb {
                // RGB/RGBA: swap raw u32 with alpha clamping
                let mask = if has_a { 0xFFFF_FFFF } else { 0x00FF_FFFF };
                pixels[left] = r & mask;
                pixels[right] = l & mask;
            } else {
                // L/LA: extract R (luma), set G=B=R, preserve alpha
                let l_r = l & 0xFF;
                let l_a = l & 0xFF00_0000;
                let r_r = r & 0xFF;
                let r_a = r & 0xFF00_0000;
                let l_out = l_r | (l_r << 8) | (l_r << 16) | if has_a { l_a } else { 0xFF00_0000 };
                let r_out = r_r | (r_r << 8) | (r_r << 16) | if has_a { r_a } else { 0xFF00_0000 };
                pixels[left] = r_out;
                pixels[right] = l_out;
            }
        }
        // Middle pixel in odd-width row
        if w % 2 == 1 {
            let mid = row + (w as usize / 2);
            if !has_gb {
                let p = pixels[mid];
                let r = p & 0xFF;
                let a = p & 0xFF00_0000;
                pixels[mid] = r | (r << 8) | (r << 16) | if has_a { a } else { 0xFF00_0000 };
            } else if !has_a {
                pixels[mid] &= 0x00FF_FFFF;
            }
        }
    }
}

/// Transpose: transpose/rotate with coordinate remapping.
///
/// Operations 0,1,3 are in-place (same dimensions). Operations 2,4,5,6
/// change dimensions (output is H×W). Returns (pixels, new_w, new_h).
///
/// The packed fallback is called only for RGB/RGBA typed images. Ordinary
/// L/LA inputs use native byte layouts, and 16-bit luma inputs use the CPU
/// geometry path before reaching this function.
/// mode: 2=RGB, 3=RGBA
#[inline]
pub fn transpose(
    pixels: &mut [u32],
    w: u32,
    h: u32,
    mode: u32,
    method: &TransposeMethod,
) -> (Vec<u32>, u32, u32) {
    let has_a = super::mode_is_rgba(mode);
    let w_us = w as usize;
    let h_us = h as usize;

    match method {
        // ── FlipLeftRight: horizontal mirror ──
        TransposeMethod::FlipLeftRight => {
            mirror(pixels, w, h, mode);
            (Vec::new(), w, h)
        }

        // ── FlipTopBottom: vertical flip ──
        TransposeMethod::FlipTopBottom => {
            flip(pixels, w, h, mode);
            (Vec::new(), w, h)
        }

        // ── Rotate90 (CCW): output[H×W] row_out=col, col_out=W-1-row ──
        TransposeMethod::Rotate90 => {
            let mut out = vec![0u32; w_us * h_us];
            for row_out in 0..w_us {
                for col_out in 0..h_us {
                    // output[row_out][col_out] = source[col_out][W-1-row_out]
                    let src_y = col_out;
                    let src_x = w_us - 1 - row_out;
                    let src_idx = src_y * w_us + src_x;
                    let sp = pixels[src_idx];

                    let dst_idx = row_out * h_us + col_out;
                    out[dst_idx] = sp;
                }
            }
            (out, h, w)
        }

        // ── Rotate180: reverse everything ──
        TransposeMethod::Rotate180 => {
            // Reverse entire buffer: swap first with last, etc.
            let total = w_us * h_us;
            for i in 0..total / 2 {
                let j = total - 1 - i;
                let li = pixels[i];
                let rj = pixels[j];

                let mask = if has_a { 0xFFFF_FFFF } else { 0x00FF_FFFF };
                pixels[i] = rj & mask;
                pixels[j] = li & mask;
            }
            // Middle pixel if odd total
            if total % 2 == 1 && !has_a {
                pixels[total / 2] &= 0x00FF_FFFF;
            }
            (Vec::new(), w, h)
        }

        // ── Rotate270 (CW): output[H×W] row_out=col, col_out=row ──
        TransposeMethod::Rotate270 => {
            let mut out = vec![0u32; w_us * h_us];
            for row_out in 0..w_us {
                for col_out in 0..h_us {
                    // output[row_out][col_out] = source[H-1-col_out][row_out]
                    let src_y = h_us - 1 - col_out;
                    let src_x = row_out;
                    let src_idx = src_y * w_us + src_x;
                    let sp = pixels[src_idx];

                    let dst_idx = row_out * h_us + col_out;
                    out[dst_idx] = sp;
                }
            }
            (out, h, w)
        }

        // ── Transpose: output[H×W] row_out=col, col_out=row ──
        TransposeMethod::Transpose => {
            let mut out = vec![0u32; w_us * h_us];
            for row_out in 0..w_us {
                for col_out in 0..h_us {
                    // output[row_out][col_out] = source[col_out][row_out]
                    let src_idx = col_out * w_us + row_out;
                    let sp = pixels[src_idx];

                    let dst_idx = row_out * h_us + col_out;
                    out[dst_idx] = sp;
                }
            }
            (out, h, w)
        }

        // ── Transverse: output[H×W] row_out=W-1-col, col_out=H-1-row ──
        TransposeMethod::Transverse => {
            let mut out = vec![0u32; w_us * h_us];
            for row_out in 0..w_us {
                for col_out in 0..h_us {
                    // output[row_out][col_out] = source[W-1-row_out][H-1-col_out]
                    let src_y = h_us - 1 - col_out;
                    let src_x = w_us - 1 - row_out;
                    let src_idx = src_y * w_us + src_x;
                    let sp = pixels[src_idx];

                    let dst_idx = row_out * h_us + col_out;
                    out[dst_idx] = sp;
                }
            }
            (out, h, w)
        }
    }
}

#[inline]
pub fn pad(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    filter: u32,
    centering_x: f64,
    centering_y: f64,
    fill_rgba: u32,
) -> (Vec<u32>, u32, u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    // ImageOps.pad first contains the source in the destination, using the
    // requested resampling filter, then pastes that resized image according to
    // the centering pair. The previous SIMD adapter copied/cropped the source
    // directly, which skipped both stages and diverged for non-square targets.
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (
            vec![0; (dst_w as usize).saturating_mul(dst_h as usize)],
            dst_w,
            dst_h,
        );
    }
    let source_ratio = w as f64 / h as f64;
    let destination_ratio = dst_w as f64 / dst_h as f64;
    let (new_w, new_h) = if (source_ratio - destination_ratio).abs() < 1e-10 {
        (dst_w, dst_h)
    } else if source_ratio > destination_ratio {
        (
            dst_w,
            round_positive_ties_even(h as f64 / w as f64 * dst_w as f64).max(1),
        )
    } else {
        (
            round_positive_ties_even(w as f64 / h as f64 * dst_h as f64).max(1),
            dst_h,
        )
    };
    let (resized, _, _) = resize(pixels, w, h, new_w, new_h, mode, filter);

    // Pre-compute mode-aware fill pixel (packed u32)
    let fill_r = fill_rgba & 0xFF;
    let fill = if has_gb {
        let fill_g = (fill_rgba >> 8) & 0xFF;
        let fill_b = (fill_rgba >> 16) & 0xFF;
        let fill_a = if has_a {
            fill_rgba & 0xFF00_0000
        } else {
            0xFF00_0000
        };
        fill_r | (fill_g << 8) | (fill_b << 16) | fill_a
    } else {
        let fill_a = if has_a {
            fill_rgba & 0xFF00_0000
        } else {
            0xFF00_0000
        };
        fill_r | (fill_r << 8) | (fill_r << 16) | fill_a
    };

    let dst_size = (dst_w as usize) * (dst_h as usize);
    let mut out = vec![fill; dst_size];

    let cx = centering_x.clamp(0.0, 1.0);
    let cy = centering_y.clamp(0.0, 1.0);
    let offset_x = round_positive_ties_even((dst_w - new_w) as f64 * cx);
    let offset_y = round_positive_ties_even((dst_h - new_h) as f64 * cy);
    for sy in 0..new_h {
        let dy = offset_y + sy;
        if dy >= dst_h {
            continue;
        }
        for sx in 0..new_w {
            let dx = offset_x + sx;
            if dx >= dst_w {
                continue;
            }
            let sp = resized[(sy * new_w + sx) as usize];
            let out_p = if has_gb {
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                (sp & 0x00FF_FFFF) | a
            } else {
                let r = sp & 0xFF;
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                r | (r << 8) | (r << 16) | a
            };
            out[(dy * dst_w + dx) as usize] = out_p;
        }
    }

    (out, dst_w, dst_h)
}

/// Expand: add `border` pixels on all 4 sides, filling with fill_rgba.
///
/// New size = (w + 2*border) x (h + 2*border).
/// Source image is placed at offset (border, border) in the new canvas.
/// Returns (pixels, new_w, new_h).
///
/// Mode-aware: alpha byte clamped for non-alpha modes (L, RGB).
/// For L/LA modes: G = B = R (luma carried in R channel).
/// For RGB/RGBA modes: G and B are independent.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn expand(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    border: u32,
    fill_rgba: u32,
) -> (Vec<u32>, u32, u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let b = border as i32;
    let new_w = w + 2 * border;
    let new_h = h + 2 * border;
    let new_w_i = new_w as i32;
    let w_i = w as i32;
    let h_i = h as i32;

    // Pre-compute mode-aware fill pixel
    let fill_r = fill_rgba & 0xFF;
    let fill = if has_gb {
        let fill_g = (fill_rgba >> 8) & 0xFF;
        let fill_b = (fill_rgba >> 16) & 0xFF;
        let fill_a = if has_a {
            fill_rgba & 0xFF00_0000
        } else {
            0xFF00_0000
        };
        fill_r | (fill_g << 8) | (fill_b << 16) | fill_a
    } else {
        let fill_a = if has_a {
            fill_rgba & 0xFF00_0000
        } else {
            0xFF00_0000
        };
        fill_r | (fill_r << 8) | (fill_r << 16) | fill_a
    };

    let dst_size = (new_w as usize) * (new_h as usize);
    let mut out = vec![fill; dst_size];

    // Copy source pixels into the center of the expanded canvas
    for sy in 0..h_i {
        let dst_y = sy + b;
        let src_row = (sy * w_i) as usize;
        let dst_row_start = (dst_y * new_w_i + b) as usize;

        for dx in 0..w_i {
            let sp = pixels[src_row + dx as usize];
            let out_p = if has_gb {
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                (sp & 0x00FF_FFFF) | a
            } else {
                let r = sp & 0xFF;
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                r | (r << 8) | (r << 16) | a
            };
            out[dst_row_start + dx as usize] = out_p;
        }
    }

    (out, new_w, new_h)
}

/// Crop border: remove `border` pixels from all 4 sides.
///
/// New size = (w - 2*border) x (h - 2*border).
/// Copies the inner sub-region of the source image.
/// Returns (pixels, new_w, new_h).
///
/// Mode-aware: alpha byte clamped for non-alpha modes (L, RGB).
/// For L/LA modes: G = B = R (luma carried in R channel).
/// For RGB/RGBA modes: G and B are independent.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn crop_border(pixels: &[u32], w: u32, h: u32, mode: u32, border: u32) -> (Vec<u32>, u32, u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let new_w = w.saturating_sub(2 * border);
    let new_h = h.saturating_sub(2 * border);
    let w_us = w as usize;
    let new_w_us = new_w as usize;

    let dst_size = new_w_us * (new_h as usize);
    let mut out = vec![0u32; dst_size];

    for dy in 0..(new_h as usize) {
        let src_row = (dy + border as usize) * w_us + border as usize;
        let dst_row = dy * new_w_us;
        for dx in 0..new_w_us {
            let sp = pixels[src_row + dx];
            let out_p = if has_gb {
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                (sp & 0x00FF_FFFF) | a
            } else {
                let r = sp & 0xFF;
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                r | (r << 8) | (r << 16) | a
            };
            out[dst_row + dx] = out_p;
        }
    }

    (out, new_w, new_h)
}

// ── Spatial operations (crop, thumbnail, reduce, convert) ──────────────────

/// Crop: extract sub-region from the image.
///
/// Copies rows between `top..bottom` and columns between `left..right`.
/// Returns `(Vec<u32>, new_w, new_h)` where `new_w = right-left, new_h = bottom-top`.
/// Mode-aware alpha clamp: alpha forced to 0xFF for L and RGB modes.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn crop(
    pixels: &[u32],
    w: u32,
    _h: u32,
    mode: u32,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
) -> (Vec<u32>, u32, u32) {
    let has_a = super::mode_has_alpha(mode);
    let new_w = right.saturating_sub(left);
    let new_h = bottom.saturating_sub(top);
    let mut out = vec![0u32; (new_w * new_h) as usize];

    for y in 0..new_h {
        for x in 0..new_w {
            let src_y = top + y;
            let src_x = left + x;
            let src_idx = (src_y * w + src_x) as usize;
            let p = pixels[src_idx];

            let r = p & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = (p >> 16) & 0xFF;
            let a = p & 0xFF00_0000;
            let out_a = if has_a { a } else { 0xFF00_0000 };

            let dst_idx = (y * new_w + x) as usize;
            out[dst_idx] = r | (g << 8) | (b << 16) | out_a;
        }
    }

    (out, new_w, new_h)
}

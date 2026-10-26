use super::super::{mode_has_alpha, mode_has_green_blue};

/// Box blur: average all pixels in (2*radius+1)^2 window, per channel.
///
/// Uses separable horizontal then vertical passes with PIL-style fixed-point
/// arithmetic (24-bit precision). Source coordinates are clamped to image
/// bounds (replicate-edge behavior, matching PIL).
///
/// For L/LA modes: G = B = R after averaging (luma carried in R channel).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn box_blur(pixels: &mut [u32], w: u32, h: u32, mode: u32, radius: u32) {
    if radius == 0 {
        return;
    }
    let has_gb = mode_has_green_blue(mode);
    let has_a = mode_has_alpha(mode);
    let r = radius as i32;
    let w_i = w as i32;
    let h_i = h as i32;
    let window_pixels = (2 * r + 1) as u32;
    let ww: u32 = ((1u64 << 24) / window_pixels as u64) as u32;
    let bias: u32 = 1u32 << 23;

    // ── Horizontal pass ──
    let mut hpass = vec![0u32; (w * h) as usize];
    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;

            if has_gb {
                // RGB/RGBA: separate R, G, B, A accumulation
                let mut acc_r = 0u64;
                let mut acc_g = 0u64;
                let mut acc_b = 0u64;
                let mut acc_a = 0u64;
                for dx in -r..=r {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sp2 = pixels[(y * w_i + sx) as usize];
                    acc_r += (sp2 & 0xFF) as u64;
                    acc_g += ((sp2 >> 8) & 0xFF) as u64;
                    acc_b += ((sp2 >> 16) & 0xFF) as u64;
                    if has_a {
                        acc_a += ((sp2 >> 24) & 0xFF) as u64;
                    }
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let out_g = ((acc_g * ww as u64 + bias as u64) >> 24) as u32;
                let out_b = ((acc_b * ww as u64 + bias as u64) >> 24) as u32;
                let out_a = if has_a {
                    ((acc_a * ww as u64 + bias as u64) >> 24) as u32
                } else {
                    0xFF
                };
                hpass[idx] = out_r | (out_g << 8) | (out_b << 16) | (out_a << 24);
            } else {
                // L/LA: R carries luma, with alpha accumulated separately.
                let mut acc_r = 0u64;
                let mut acc_a = 0u64;
                for dx in -r..=r {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sp2 = pixels[(y * w_i + sx) as usize];
                    acc_r += (sp2 & 0xFF) as u64;
                    if has_a {
                        acc_a += ((sp2 >> 24) & 0xFF) as u64;
                    }
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let out_a = if has_a {
                    ((acc_a * ww as u64 + bias as u64) >> 24) as u32
                } else {
                    0xFF
                };
                hpass[idx] = out_r | (out_r << 8) | (out_r << 16) | (out_a << 24);
            }
        }
    }

    box_blur_vertical_pass(
        pixels,
        &hpass,
        BoxBlurPlan {
            width: w_i,
            height: h_i,
            radius: r,
            has_gb,
            has_alpha: has_a,
            weight: ww,
            bias,
        },
    );
}

struct BoxBlurPlan {
    width: i32,
    height: i32,
    radius: i32,
    has_gb: bool,
    has_alpha: bool,
    weight: u32,
    bias: u32,
}

fn box_blur_vertical_pass(pixels: &mut [u32], hpass: &[u32], plan: BoxBlurPlan) {
    let BoxBlurPlan {
        width: w_i,
        height: h_i,
        radius: r,
        has_gb,
        has_alpha: has_a,
        weight: ww,
        bias,
    } = plan;
    // ── Vertical pass ──
    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;

            if has_gb {
                let mut acc_r = 0u64;
                let mut acc_g = 0u64;
                let mut acc_b = 0u64;
                let mut acc_a = 0u64;
                for dy in -r..=r {
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let sp2 = hpass[(sy * w_i + x) as usize];
                    acc_r += (sp2 & 0xFF) as u64;
                    acc_g += ((sp2 >> 8) & 0xFF) as u64;
                    acc_b += ((sp2 >> 16) & 0xFF) as u64;
                    if has_a {
                        acc_a += ((sp2 >> 24) & 0xFF) as u64;
                    }
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let out_g = ((acc_g * ww as u64 + bias as u64) >> 24) as u32;
                let out_b = ((acc_b * ww as u64 + bias as u64) >> 24) as u32;
                let out_a = if has_a {
                    ((acc_a * ww as u64 + bias as u64) >> 24) as u32
                } else {
                    0xFF
                };
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | (out_a << 24);
            } else {
                let mut acc_r = 0u64;
                let mut acc_a = 0u64;
                for dy in -r..=r {
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let sp2 = hpass[(sy * w_i + x) as usize];
                    acc_r += (sp2 & 0xFF) as u64;
                    if has_a {
                        acc_a += ((sp2 >> 24) & 0xFF) as u64;
                    }
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let out_a = if has_a {
                    ((acc_a * ww as u64 + bias as u64) >> 24) as u32
                } else {
                    0xFF
                };
                pixels[idx] = out_r | (out_r << 8) | (out_r << 16) | (out_a << 24);
            }
        }
    }
}

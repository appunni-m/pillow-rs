//! Scalar fallback for scalar neighborhood and rank filters.

// ── Rank-based window filter operations ──

/// Evaluate one Pillow convolution row in the same contraction order as the
/// CPU filter implementation.
///
/// Pillow's arm64 build starts with the centre product and then emits fused
/// multiply-adds for the left and right products. Keeping that order here is
/// required for exact byte parity when a result is just below an integer.
#[inline]
fn pillow_kernel_row_3(pixels: [f32; 3], kernel: &[f32]) -> f32 {
    let sum = pixels[1] * kernel[1];
    let sum = pixels[0].mul_add(kernel[0], sum);
    pixels[2].mul_add(kernel[2], sum)
}

/// Five-tap counterpart of [`pillow_kernel_row_3`].
#[inline]
fn pillow_kernel_row_5(pixels: [f32; 5], kernel: &[f32]) -> f32 {
    let sum = pixels[1] * kernel[1];
    let sum = pixels[0].mul_add(kernel[0], sum);
    let sum = pixels[2].mul_add(kernel[2], sum);
    let sum = pixels[3].mul_add(kernel[3], sum);
    pixels[4].mul_add(kernel[4], sum)
}

/// Pillow's UINT8 filter clamp: truncate values in the open interval and
/// clamp the endpoints before storing a byte.
#[inline]
fn clip8_filter(value: f32) -> u32 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u32
    }
}

/// Median filter: for each pixel, output median of size×size neighborhood.
///
/// For each pixel, collects all R (and G/B for RGB/RGBA) channel values within
/// the size×size window (with clamped border handling), sorts them, and outputs
/// the value at index `size*size/2`.
///
/// For L/LA modes (mode < 2): only R channel is processed. G and B mirror R.
/// For RGB/RGBA modes (mode >= 2): R, G, B processed independently.
/// Alpha is filtered independently in LA/RGBA, and forced to 0xFF in L/RGB.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn median_filter(pixels: &mut [u32], w: u32, h: u32, mode: u32, size: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let half = (size / 2) as i32;
    let w_i = w as i32;
    let h_i = h as i32;
    let area = (size * size) as usize;
    let mid = area / 2;

    // Copy source since we read from neighbors while writing
    let src = pixels.to_vec();
    let mut r_vals = Vec::with_capacity(area);
    let mut a_vals = if has_a {
        Vec::with_capacity(area)
    } else {
        Vec::new()
    };
    let mut g_vals = Vec::with_capacity(area);
    let mut b_vals = Vec::with_capacity(area);

    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            // Collect R channel values from window
            r_vals.clear();
            a_vals.clear();
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let sp = src[(sy * w_i + sx) as usize];
                    r_vals.push((sp & 0xFF) as u8);
                    if has_a {
                        a_vals.push((sp >> 24) as u8);
                    }
                }
            }
            r_vals.sort_unstable();
            let out_r = r_vals[mid] as u32;
            let out_a = if has_a {
                a_vals.sort_unstable();
                (a_vals[mid] as u32) << 24
            } else {
                0xFF00_0000
            };

            if has_gb {
                g_vals.clear();
                b_vals.clear();
                for dy in -half..=half {
                    for dx in -half..=half {
                        let sx = (x + dx).clamp(0, w_i - 1);
                        let sy = (y + dy).clamp(0, h_i - 1);
                        let sp = src[(sy * w_i + sx) as usize];
                        g_vals.push(((sp >> 8) & 0xFF) as u8);
                        b_vals.push(((sp >> 16) & 0xFF) as u8);
                    }
                }
                g_vals.sort_unstable();
                b_vals.sort_unstable();
                let out_g = g_vals[mid] as u32;
                let out_b = b_vals[mid] as u32;
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            } else {
                pixels[idx] = out_r | (out_r << 8) | (out_r << 16) | out_a;
            }
        }
    }
}

/// Max filter: for each pixel, output maximum of size×size neighborhood.
///
/// Tracks running max per channel while iterating the window (no sorting needed).
/// For L/LA modes (mode < 2): only R channel is tracked. G and B mirror R.
/// For RGB/RGBA modes (mode >= 2): R, G, B tracked independently.
/// Alpha is filtered independently in LA/RGBA, and forced to 0xFF in L/RGB.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn max_filter(pixels: &mut [u32], w: u32, h: u32, mode: u32, size: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let half = (size / 2) as i32;
    let w_i = w as i32;
    let h_i = h as i32;

    // Copy source since we read from neighbors while writing
    let src = pixels.to_vec();

    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            // Track running max for R channel
            let mut max_r = 0u8;
            let mut max_a = 0u8;
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let sp = src[(sy * w_i + sx) as usize];
                    max_r = max_r.max((sp & 0xFF) as u8);
                    if has_a {
                        max_a = max_a.max((sp >> 24) as u8);
                    }
                }
            }
            let out_r = max_r as u32;
            let out_a = if has_a {
                (max_a as u32) << 24
            } else {
                0xFF00_0000
            };

            if has_gb {
                let mut max_g = 0u8;
                let mut max_b = 0u8;
                for dy in -half..=half {
                    for dx in -half..=half {
                        let sx = (x + dx).clamp(0, w_i - 1);
                        let sy = (y + dy).clamp(0, h_i - 1);
                        let sp = src[(sy * w_i + sx) as usize];
                        max_g = max_g.max(((sp >> 8) & 0xFF) as u8);
                        max_b = max_b.max(((sp >> 16) & 0xFF) as u8);
                    }
                }
                let out_g = max_g as u32;
                let out_b = max_b as u32;
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            } else {
                pixels[idx] = out_r | (out_r << 8) | (out_r << 16) | out_a;
            }
        }
    }
}

/// Min filter: for each pixel, output minimum of size×size neighborhood.
///
/// Tracks running min per channel while iterating the window (no sorting needed).
/// For L/LA modes (mode < 2): only R channel is tracked. G and B mirror R.
/// For RGB/RGBA modes (mode >= 2): R, G, B tracked independently.
/// Alpha is filtered independently in LA/RGBA, and forced to 0xFF in L/RGB.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn min_filter(pixels: &mut [u32], w: u32, h: u32, mode: u32, size: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let half = (size / 2) as i32;
    let w_i = w as i32;
    let h_i = h as i32;

    // Copy source since we read from neighbors while writing
    let src = pixels.to_vec();

    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            // Track running min for R channel
            let mut min_r = 255u8;
            let mut min_a = 255u8;
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let sp = src[(sy * w_i + sx) as usize];
                    min_r = min_r.min((sp & 0xFF) as u8);
                    if has_a {
                        min_a = min_a.min((sp >> 24) as u8);
                    }
                }
            }
            let out_r = min_r as u32;
            let out_a = if has_a {
                (min_a as u32) << 24
            } else {
                0xFF00_0000
            };

            if has_gb {
                let mut min_g = 255u8;
                let mut min_b = 255u8;
                for dy in -half..=half {
                    for dx in -half..=half {
                        let sx = (x + dx).clamp(0, w_i - 1);
                        let sy = (y + dy).clamp(0, h_i - 1);
                        let sp = src[(sy * w_i + sx) as usize];
                        min_g = min_g.min(((sp >> 8) & 0xFF) as u8);
                        min_b = min_b.min(((sp >> 16) & 0xFF) as u8);
                    }
                }
                let out_g = min_g as u32;
                let out_b = min_b as u32;
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            } else {
                pixels[idx] = out_r | (out_r << 8) | (out_r << 16) | out_a;
            }
        }
    }
}

/// Rank filter: for each pixel, output value at position `rank` from sorted
/// size×size neighborhood.
///
/// rank=0 gives min, rank=size*size/2 gives median, rank=size*size-1 gives max.
/// Rank is clamped to [0, size*size-1].
///
/// For L/LA modes (mode < 2): only R channel is sorted. G and B mirror R.
/// For RGB/RGBA modes (mode >= 2): R, G, B sorted independently.
/// Alpha is filtered independently in LA/RGBA, and forced to 0xFF in L/RGB.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
fn select_rank_histogram(histogram: &[usize; 256], rank: usize) -> u32 {
    let mut seen = 0usize;
    for (value, count) in histogram.iter().enumerate() {
        seen += *count;
        if seen > rank {
            return value as u32;
        }
    }
    255
}

#[inline]
pub fn rank_filter(pixels: &mut [u32], w: u32, h: u32, mode: u32, size: u32, rank: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let half = (size / 2) as i32;
    let w_i = w as i32;
    let h_i = h as i32;
    let area = (size * size) as usize;
    let rank = (rank.min((area - 1) as u32)) as usize;

    // Copy source since we read from neighbors while writing
    let src = pixels.to_vec();

    for y in 0..h_i {
        let mut histogram = [[0usize; 256]; 4];
        for dy in -half..=half {
            let sy = (y + dy).clamp(0, h_i - 1);
            for dx in -half..=half {
                let sx = dx.clamp(0, w_i - 1);
                let pixel = src[(sy * w_i + sx) as usize];
                histogram[0][(pixel & 0xFF) as usize] += 1;
                if has_gb {
                    histogram[1][((pixel >> 8) & 0xFF) as usize] += 1;
                    histogram[2][((pixel >> 16) & 0xFF) as usize] += 1;
                }
                if has_a {
                    histogram[3][(pixel >> 24) as usize] += 1;
                }
            }
        }

        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            let out_r = select_rank_histogram(&histogram[0], rank);
            let out_a = if has_a {
                select_rank_histogram(&histogram[3], rank) << 24
            } else {
                0xFF00_0000
            };
            pixels[idx] = if has_gb {
                let out_g = select_rank_histogram(&histogram[1], rank);
                let out_b = select_rank_histogram(&histogram[2], rank);
                out_r | (out_g << 8) | (out_b << 16) | out_a
            } else {
                out_r | (out_r << 8) | (out_r << 16) | out_a
            };

            if x + 1 < w_i {
                let remove_x = (x - half).clamp(0, w_i - 1);
                let add_x = (x + half + 1).clamp(0, w_i - 1);
                for dy in -half..=half {
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let remove_pixel = src[(sy * w_i + remove_x) as usize];
                    let add_pixel = src[(sy * w_i + add_x) as usize];
                    let remove_r = (remove_pixel & 0xFF) as usize;
                    let add_r = (add_pixel & 0xFF) as usize;
                    histogram[0][remove_r] -= 1;
                    histogram[0][add_r] += 1;
                    if has_gb {
                        histogram[1][((remove_pixel >> 8) & 0xFF) as usize] -= 1;
                        histogram[1][((add_pixel >> 8) & 0xFF) as usize] += 1;
                        histogram[2][((remove_pixel >> 16) & 0xFF) as usize] -= 1;
                        histogram[2][((add_pixel >> 16) & 0xFF) as usize] += 1;
                    }
                    if has_a {
                        histogram[3][(remove_pixel >> 24) as usize] -= 1;
                        histogram[3][(add_pixel >> 24) as usize] += 1;
                    }
                }
            }
        }
    }
}

// ── Convolution filter operations ─────────────────────────────────────────

/// 3x3 convolution filter on packed u32 RGBA pixels.
///
/// For each pixel, convolves a 3x3 neighborhood with the given kernel,
/// applies scale/offset, and clamps to [0,255]. Border pixels are handled
/// by clamping coordinates (replicate-edge behavior, matching PIL).
///
/// Mode-aware:
/// - L/LA (mode 0/1): convolves R channel only; G = B = out_r.
/// - RGB/RGBA (mode 2/3): convolves all three channels independently.
/// - Alpha preserved in LA/RGBA; forced to 0xFF in L/RGB.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn filter_3x3(
    pixels: &mut [u32],
    w: u32,
    h: u32,
    mode: u32,
    kernel: &[f32],
    scale: f32,
    offset: i32,
) {
    if w < 3 || h < 3 {
        return;
    }
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let src = pixels.to_vec();
    let w_i = w as i32;
    let h_i = h as i32;
    let s = scale;
    let normalized: [f32; 9] = std::array::from_fn(|index| kernel[index] / s);

    // Pillow leaves the one-pixel border unchanged for convolution filters;
    // only the fully covered interior is evaluated.
    for y in 1..h_i - 1 {
        for x in 1..w_i - 1 {
            let base = |dx: i32, dy: i32| -> usize { ((y + dy) * w_i + (x + dx)) as usize };
            let channel = |shift: u32, dx: i32, dy: i32| -> f32 {
                ((src[base(dx, dy)] >> shift) & 0xFF) as f32
            };
            let row = |shift: u32, dy: i32, coefficients: &[f32]| -> f32 {
                pillow_kernel_row_3(
                    [
                        channel(shift, -1, dy),
                        channel(shift, 0, dy),
                        channel(shift, 1, dy),
                    ],
                    coefficients,
                )
            };
            let mut sum_r = offset as f32 + 0.5;
            sum_r += row(0, 1, &normalized[0..3]);
            sum_r += row(0, 0, &normalized[3..6]);
            sum_r += row(0, -1, &normalized[6..9]);
            let mut sum_g = offset as f32 + 0.5;
            sum_g += row(8, 1, &normalized[0..3]);
            sum_g += row(8, 0, &normalized[3..6]);
            sum_g += row(8, -1, &normalized[6..9]);
            let mut sum_b = offset as f32 + 0.5;
            sum_b += row(16, 1, &normalized[0..3]);
            sum_b += row(16, 0, &normalized[3..6]);
            sum_b += row(16, -1, &normalized[6..9]);
            let mut sum_a = offset as f32 + 0.5;
            sum_a += row(24, 1, &normalized[0..3]);
            sum_a += row(24, 0, &normalized[3..6]);
            sum_a += row(24, -1, &normalized[6..9]);

            let out_r = clip8_filter(sum_r);
            let out_g_raw = clip8_filter(sum_g);
            let out_b_raw = clip8_filter(sum_b);

            // L/LA: G = B = out_r; RGB/RGBA: independent G and B
            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };

            let out_a = if has_a {
                clip8_filter(sum_a) << 24
            } else {
                0xFF00_0000
            };

            let idx = base(0, 0);
            pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
    }
}

/// 5x5 convolution filter on packed u32 RGBA pixels.
///
/// Same semantics as `filter_3x3` but with a 5x5 kernel (kernel must have 25 entries).
/// Border clamping, mode-awareness, and alpha handling match `filter_3x3`.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn filter_5x5(
    pixels: &mut [u32],
    w: u32,
    h: u32,
    mode: u32,
    kernel: &[f32],
    scale: f32,
    offset: i32,
) {
    if w < 5 || h < 5 {
        return;
    }
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let src = pixels.to_vec();
    let w_i = w as i32;
    let h_i = h as i32;
    let s = scale;
    let normalized: [f32; 25] = std::array::from_fn(|index| kernel[index] / s);

    // As in Pillow's ImagingFilter, the two-pixel border is copied through.
    for y in 2..h_i - 2 {
        for x in 2..w_i - 2 {
            let base = |dx: i32, dy: i32| -> usize { ((y + dy) * w_i + (x + dx)) as usize };
            let channel = |shift: u32, dx: i32, dy: i32| -> f32 {
                ((src[base(dx, dy)] >> shift) & 0xFF) as f32
            };
            let row = |shift: u32, dy: i32, coefficients: &[f32]| -> f32 {
                pillow_kernel_row_5(
                    [
                        channel(shift, -2, dy),
                        channel(shift, -1, dy),
                        channel(shift, 0, dy),
                        channel(shift, 1, dy),
                        channel(shift, 2, dy),
                    ],
                    coefficients,
                )
            };
            let mut sum_r = offset as f32 + 0.5;
            sum_r += row(0, 2, &normalized[0..5]);
            sum_r += row(0, 1, &normalized[5..10]);
            sum_r += row(0, 0, &normalized[10..15]);
            sum_r += row(0, -1, &normalized[15..20]);
            sum_r += row(0, -2, &normalized[20..25]);
            let mut sum_g = offset as f32 + 0.5;
            sum_g += row(8, 2, &normalized[0..5]);
            sum_g += row(8, 1, &normalized[5..10]);
            sum_g += row(8, 0, &normalized[10..15]);
            sum_g += row(8, -1, &normalized[15..20]);
            sum_g += row(8, -2, &normalized[20..25]);
            let mut sum_b = offset as f32 + 0.5;
            sum_b += row(16, 2, &normalized[0..5]);
            sum_b += row(16, 1, &normalized[5..10]);
            sum_b += row(16, 0, &normalized[10..15]);
            sum_b += row(16, -1, &normalized[15..20]);
            sum_b += row(16, -2, &normalized[20..25]);
            let mut sum_a = offset as f32 + 0.5;
            sum_a += row(24, 2, &normalized[0..5]);
            sum_a += row(24, 1, &normalized[5..10]);
            sum_a += row(24, 0, &normalized[10..15]);
            sum_a += row(24, -1, &normalized[15..20]);
            sum_a += row(24, -2, &normalized[20..25]);

            let out_r = clip8_filter(sum_r);
            let out_g_raw = clip8_filter(sum_g);
            let out_b_raw = clip8_filter(sum_b);

            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };

            let out_a = if has_a {
                clip8_filter(sum_a) << 24
            } else {
                0xFF00_0000
            };

            let idx = base(0, 0);
            pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
    }
}

/// PIL ImageEnhance.Sharpness on packed u32 RGBA pixels.
///
/// PIL algorithm:
/// 1. Apply SMOOTH kernel [1,1,1, 1,5,1, 1,1,1] / 13 (NOT a sharpen kernel!)
/// 2. Blend: out = blurred * (1.0 - factor) + original * factor
///
/// `factor_fp` is a fixed-point factor (factor * 1000) where:
/// - factor_fp = 1000 (1.0) -> output = original (identity)
/// - factor_fp < 1000 (<1.0) -> output is MORE blurred (anti-sharpen)
/// - factor_fp > 1000 (>1.0) -> output = original + (original - blurred) * (factor-1) (unsharp mask)
///
/// CPU reference: pool_cpu/ops/enhance.rs op_enhance_sharpness
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA, 4=CMYK
#[inline]
pub fn sharpness(pixels: &mut [u32], w: u32, h: u32, mode: u32, factor_fp: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let is_cmyk = mode == 4;
    let src = pixels.to_vec();
    let w_i = w as i32;
    let h_i = h as i32;
    // SMOOTH kernel pre-divided: [1,1,1, 1,5,1, 1,1,1] / 13
    let inv_scale = 1.0f32 / 13.0f32;
    let k_edges = inv_scale; // edges = 1/13
    let k_center = 5.0f32 * inv_scale; // center = 5/13
    let rounding_bias = 0.5f32; // offset=0 => 0.0 + 0.5
    // Blend weight: factor_fp / 1000. Pillow blends the integer-filtered
    // samples in double precision and truncates to u8; keeping this separate
    // from the f32 convolution is required for exact parity at half factors.
    let t = factor_fp as f64 / 1000.0;
    let one_minus_t = 1.0 - t;

    // Pillow's ImageFilter.filter leaves the one-pixel border unchanged. A
    // clamped convolution over the complete frame is a tempting SIMD
    // shortcut, but it changes every edge pixel for non-identity factors.
    let mut blurred = src.clone();
    if w_i >= 3 && h_i >= 3 {
        for y in 1..(h_i - 1) {
            for x in 1..(w_i - 1) {
                let idx = (y * w_i + x) as usize;

                // Convolve with SMOOTH kernel
                let mut sum_r: f32 = 0.0;
                let mut sum_g: f32 = 0.0;
                let mut sum_b: f32 = 0.0;
                let mut sum_a: f32 = 0.0;

                for ky in -1..=1 {
                    for kx in -1..=1 {
                        let sx = x + kx;
                        let sy = y + ky;
                        let sp = src[(sy * w_i + sx) as usize];
                        // Kernel position: center (1,1) gets 5/13, edges get 1/13
                        let k = if ky == 0 && kx == 0 {
                            k_center
                        } else {
                            k_edges
                        };
                        sum_r += (sp & 0xFF) as f32 * k;
                        sum_g += ((sp >> 8) & 0xFF) as f32 * k;
                        sum_b += ((sp >> 16) & 0xFF) as f32 * k;
                        if is_cmyk {
                            sum_a += ((sp >> 24) & 0xFF) as f32 * k;
                        }
                    }
                }

                // Apply rounding bias and clamp to [0, 255]
                let blur_r = ((sum_r + rounding_bias) as i32).clamp(0, 255) as u32;
                let blur_g = ((sum_g + rounding_bias) as i32).clamp(0, 255) as u32;
                let blur_b = ((sum_b + rounding_bias) as i32).clamp(0, 255) as u32;
                let blur_a = ((sum_a + rounding_bias) as i32).clamp(0, 255) as u32;

                blurred[idx] = blur_r
                    | ((if has_gb { blur_g } else { blur_r }) << 8)
                    | ((if has_gb { blur_b } else { blur_r }) << 16)
                    | if is_cmyk {
                        blur_a << 24
                    } else {
                        src[idx] & 0xFF00_0000
                    };
            }
        }
    }

    for (idx, pixel) in pixels.iter_mut().enumerate() {
        let orig = src[idx];
        let blur = blurred[idx];
        let blend = |filtered: u32, original: u32| {
            (filtered as f64 * one_minus_t + original as f64 * t).clamp(0.0, 255.0) as u32
        };
        let out_r = blend(blur & 0xFF, orig & 0xFF);
        let out_g = if has_gb {
            blend((blur >> 8) & 0xFF, (orig >> 8) & 0xFF)
        } else {
            out_r
        };
        let out_b = if has_gb {
            blend((blur >> 16) & 0xFF, (orig >> 16) & 0xFF)
        } else {
            out_r
        };
        let out_a = if is_cmyk {
            blend((blur >> 24) & 0xFF, (orig >> 24) & 0xFF) << 24
        } else if has_a {
            orig & 0xFF00_0000
        } else {
            0xFF00_0000
        };
        *pixel = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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

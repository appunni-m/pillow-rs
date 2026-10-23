//! Scalar fallback for scalar histogram and equalization operations.

/// Histogram equalization helper: build a LUT from a 256-bin histogram using PIL's
/// step formula. Returns identity LUT if step <= 0 or only one non-zero bin.
#[inline(always)]
fn build_equalize_lut(hist: &[u32; 256]) -> [u8; 256] {
    // Collect non-zero bins
    let nonzero: Vec<u32> = hist.iter().filter(|&&c| c > 0).copied().collect();
    if nonzero.len() <= 1 {
        // Identity LUT — only one distinct value or all zero
        let mut lut = [0u8; 256];
        for i in 0..256 {
            lut[i] = i as u8;
        }
        return lut;
    }
    let total: u32 = nonzero.iter().sum();
    // PIL step formula: (sum(non_zero_bins) - last_bin_count) / 255
    let step = (total - nonzero[nonzero.len() - 1]) / 255;
    if step == 0 {
        let mut lut = [0u8; 256];
        for i in 0..256 {
            lut[i] = i as u8;
        }
        return lut;
    }
    // PIL equalize: start with step/2 accumulator
    let mut n = step / 2;
    let mut lut = [0u8; 256];
    for i in 0..256 {
        lut[i] = (n / step).min(255) as u8;
        n += hist[i];
    }
    lut
}

/// Equalize: histogram equalization matching PIL's ImageOps.equalize algorithm.
///
/// Builds a histogram, computes CDF, and builds a LUT using PIL's step formula
/// (`step = (sum(non_zero_bins) - last_bin_count) / 255`). Then remaps all pixels.
///
/// For L/LA modes (mode < 2): only the R channel is equalized (luma); G and B
/// mirror the equalized R value.
/// For RGB/RGBA modes (mode >= 2): all three channels are equalized independently.
/// Alpha is preserved in LA/RGBA, forced to 0xFF in L/RGB.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn equalize(pixels: &mut [u32], w: u32, h: u32, mode: u32) {
    if w == 0 || h == 0 || pixels.is_empty() {
        return;
    }

    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    if has_gb {
        // RGB/RGBA: equalize R, G, B independently
        let mut hist_r = [0u32; 256];
        let mut hist_g = [0u32; 256];
        let mut hist_b = [0u32; 256];

        for &p in pixels.iter() {
            hist_r[(p & 0xFF) as usize] += 1;
            hist_g[((p >> 8) & 0xFF) as usize] += 1;
            hist_b[((p >> 16) & 0xFF) as usize] += 1;
        }

        let lut_r = build_equalize_lut(&hist_r);
        let lut_g = build_equalize_lut(&hist_g);
        let lut_b = build_equalize_lut(&hist_b);

        for p in pixels.iter_mut() {
            let r = *p & 0xFF;
            let g = (*p >> 8) & 0xFF;
            let b = (*p >> 16) & 0xFF;
            let a = *p & 0xFF00_0000;

            let out_r = lut_r[r as usize] as u32;
            let out_g = lut_g[g as usize] as u32;
            let out_b = lut_b[b as usize] as u32;
            let out_a = if has_a { a } else { 0xFF00_0000 };

            *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
    } else {
        // L/LA: equalize R channel only, G = B = R
        let mut hist = [0u32; 256];
        for &p in pixels.iter() {
            hist[(p & 0xFF) as usize] += 1;
        }

        let lut = build_equalize_lut(&hist);

        for p in pixels.iter_mut() {
            let r = *p & 0xFF;
            let a = *p & 0xFF00_0000;
            let out_r = lut[r as usize] as u32;
            let out_a = if has_a { a } else { 0xFF00_0000 };
            *p = out_r | (out_r << 8) | (out_r << 16) | out_a;
        }
    }
}

/// Autocontrast: stretch image contrast based on histogram cutoff.
///
/// Builds per-channel 256-bin histograms, finds lo/hi at cutoff percentiles,
/// then linearly maps [lo, hi] to [0, 255].
///
/// For L/LA (mode < 2): only R channel processed. G = B = R after stretch.
/// For RGB/RGBA (mode >= 2): each channel independently with per-channel lo/hi.
/// Alpha preserved for LA/RGBA, forced to 0xFF for L/RGB.
///
/// Uses histogram-based O(n + 256) algorithm instead of sorting (O(n log n)),
/// matching PIL's histogram approach.
///
/// cutoff: integer percentage (0-49). Higher values clip more pixels from
/// each histogram tail for more aggressive contrast.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn autocontrast(pixels: &mut [u32], w: u32, h: u32, mode: u32, cutoff: u32) {
    let has_a = super::mode_has_alpha(mode);
    let total = (w * h) as usize;
    if total == 0 {
        return;
    }

    // Use u64 for the multiplication to avoid overflow on large images (32-bit WASM).
    let low_idx = (total as u64 * cutoff as u64 / 100) as usize;
    let high_idx =
        ((total as u64 * (100u64 - cutoff as u64) / 100) as usize).min(total.saturating_sub(1));

    if low_idx >= high_idx {
        return; // No valid range (cutoff too high or single-value image)
    }

    // Find the pixel value at the given percentile position from a 256-bin histogram.
    #[inline(always)]
    fn hist_val_at(hist: &[u32; 256], idx: usize) -> u32 {
        let mut acc = 0u32;
        for (val, &count) in hist.iter().enumerate() {
            acc += count;
            if acc > idx as u32 {
                return val as u32;
            }
        }
        255
    }

    if mode < 2 {
        // L/LA: single-channel (R). G = B = R after stretch.
        let mut hist = [0u32; 256];
        for &p in pixels.iter() {
            hist[(p & 0xFF) as usize] += 1;
        }

        let lo = hist_val_at(&hist, low_idx);
        let hi = hist_val_at(&hist, high_idx);

        if hi <= lo {
            return;
        }

        let range = hi - lo;
        for p in pixels.iter_mut() {
            let r = *p & 0xFF;
            let a = *p & 0xFF00_0000;
            // Pillow clamps values below the low percentile to zero. Using
            // saturating subtraction preserves that behavior instead of
            // panicking on a valid histogram with a nonzero cutoff.
            let out_r = (r.saturating_sub(lo) * 255 / range).min(255);
            let out_a = if has_a { a } else { 0xFF00_0000 };
            *p = out_r | (out_r << 8) | (out_r << 16) | out_a;
        }
    } else {
        // RGB/RGBA: per-channel processing with independent lo/hi.
        let mut hist_r = [0u32; 256];
        let mut hist_g = [0u32; 256];
        let mut hist_b = [0u32; 256];

        for &p in pixels.iter() {
            hist_r[(p & 0xFF) as usize] += 1;
            hist_g[((p >> 8) & 0xFF) as usize] += 1;
            hist_b[((p >> 16) & 0xFF) as usize] += 1;
        }

        let r_lo = hist_val_at(&hist_r, low_idx);
        let r_hi = hist_val_at(&hist_r, high_idx);
        let g_lo = hist_val_at(&hist_g, low_idx);
        let g_hi = hist_val_at(&hist_g, high_idx);
        let b_lo = hist_val_at(&hist_b, low_idx);
        let b_hi = hist_val_at(&hist_b, high_idx);

        let r_range = r_hi.saturating_sub(r_lo);
        let g_range = g_hi.saturating_sub(g_lo);
        let b_range = b_hi.saturating_sub(b_lo);

        let do_r = r_range > 0;
        let do_g = g_range > 0;
        let do_b = b_range > 0;

        if !do_r && !do_g && !do_b {
            return;
        }

        for p in pixels.iter_mut() {
            let r = *p & 0xFF;
            let g = (*p >> 8) & 0xFF;
            let b = (*p >> 16) & 0xFF;
            let a = *p & 0xFF00_0000;

            let out_r = if do_r {
                (r.saturating_sub(r_lo) * 255 / r_range).min(255)
            } else {
                r
            };
            let out_g = if do_g {
                (g.saturating_sub(g_lo) * 255 / g_range).min(255)
            } else {
                g
            };
            let out_b = if do_b {
                (b.saturating_sub(b_lo) * 255 / b_range).min(255)
            } else {
                b
            };
            let out_a = if has_a { a } else { 0xFF00_0000 };

            *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
    }
}

//! Scalar fallback for unary point transforms and constant fills.

/// Invert: 255 - value for each active channel.
/// Mode-aware: only touches channels present in the image mode.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn invert(pixels: &mut [u32], mode: u32) {
    let mask_r: u32 = 0x0000_00FF;
    let mask_g: u32 = 0x0000_FF00;
    let mask_b: u32 = 0x00FF_0000;
    let mask_a: u32 = 0xFF00_0000;

    let has_gb = super::mode_has_green_blue(mode); // RGB or RGBA
    let has_a = super::mode_has_alpha(mode); // LA or RGBA

    for p in pixels.iter_mut() {
        let r = *p & mask_r;
        let g = *p & mask_g;
        let b = *p & mask_b;
        let a = *p & mask_a;

        let out_r = 0xFF - r; // R always inverted (carries luma in L/LA)
        let out_g = if has_gb { 0x0000_FF00 - g } else { g };
        let out_b = if has_gb { 0x00FF_0000 - b } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | out_g | out_b | out_a;
    }
}

/// Grayscale: BT.601 luma, writes to R always, to G/B only if mode >= 2.
#[inline]
pub fn grayscale(pixels: &mut [u32], mode: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    for p in pixels.iter_mut() {
        let r = *p & 0x0000_00FF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        // BT.601: (299*R + 587*G + 114*B + 500) / 1000
        let luma = ((299 * r + 587 * g + 114 * b + 500) / 1000).min(255);

        let out_r = luma;
        let out_g = if has_gb { luma << 8 } else { g << 8 };
        let out_b = if has_gb { luma << 16 } else { b << 16 };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | out_g | out_b | out_a;
    }
}

/// Solarize: if channel >= threshold, invert it.
#[inline]
pub fn solarize(pixels: &mut [u32], mode: u32, threshold: u8) {
    let t = threshold as u32;
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let out_r = if r >= t { 255 - r } else { r };
        let out_g = if has_gb && g >= t { 255 - g } else { g };
        let out_b = if has_gb && b >= t { 255 - b } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Brightness: multiply active channels by factor (fixed-point: factor * 1000).
/// Mode 4 is CMYK, stored as C/M/Y/K in the packed RGBA lanes.
#[inline]
pub fn brightness(pixels: &mut [u32], mode: u32, factor_fp: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let has_cmyk = mode == 4;

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = (*p >> 24) & 0xFF;

        let out_r = (r * factor_fp / 1000).min(255);
        let out_g_raw = (g * factor_fp / 1000).min(255);
        let out_b_raw = (b * factor_fp / 1000).min(255);

        let out_g = if has_gb { out_g_raw } else { g };
        let out_b = if has_gb { out_b_raw } else { b };
        let out_a = if has_a {
            a << 24
        } else if has_cmyk {
            (a * factor_fp / 1000).min(255) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Contrast with the rounded Pillow image mean as its midpoint.
#[inline]
pub fn contrast(pixels: &mut [u32], mode: u32, factor: f64, mean: f64) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let adjust = |c: u32| -> u32 {
            (mean * (1.0 - factor) + c as f64 * factor).clamp(0.0, 255.0) as u32
        };

        let out_r = adjust(r);
        let out_g = if has_gb { adjust(g) } else { g };
        let out_b = if has_gb { adjust(b) } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// ColorSaturation: adjust saturation using BT.601 luma.
/// factor_fp = factor * 1000. For L/LA modes (no color info), no-op.
/// Mode-aware: only touches G/B for mode >= 2 (RGB, RGBA).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn color_saturation(pixels: &mut [u32], mode: u32, factor_fp: u32) {
    // L/LA modes have no color information — no-op
    if mode < 2 {
        return;
    }
    let has_a = super::mode_is_rgba(mode);

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        // Keep the same rounded BT.601 conversion as `pil_grayscale`.
        let luma = ((19595 * r + 38470 * g + 7471 * b + 32768) >> 16) as f64;
        let factor = factor_fp as f64 / 1000.0;

        // Pillow evaluates this blend in floating point before truncating to u8.
        // Integer division toward zero is one byte too high for negative blends.
        let adjust =
            |channel: u32| (luma + factor * (channel as f64 - luma)).clamp(0.0, 255.0) as u32;
        let out_r = adjust(r);
        let out_g = adjust(g);
        let out_b = adjust(b);
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Colorize: map luma to two-color gradient (black -> white).
/// black_rgb / white_rgb packed as 0x00_BB_GGRR (no alpha).
/// For L/LA: luma = R. For RGB/RGBA: luma = BT.601.
/// Apply the Pillow ``ImageOps.colorize`` LUT per pixel.
///
/// Pillow always promotes `ImageOps.colorize` to RGB.  The packed buffer can
/// still carry an alpha lane while the adapter is running, but the result
/// must use every LUT channel rather than preserving the source G/B lanes.
#[inline]
pub fn colorize(pixels: &mut [u32], mode: u32, lut: &[[u8; 256]; 3]) {
    let has_gb = super::mode_has_green_blue(mode);

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;

        let luma = if has_gb {
            ((299 * r + 587 * g + 114 * b + 500) / 1000).min(255) as i32
        } else {
            r as i32
        };

        let index = luma.clamp(0, 255) as usize;
        let out_r = lut[0][index] as u32;
        let out_g = lut[1][index] as u32;
        let out_b = lut[2][index] as u32;

        *p = out_r | (out_g << 8) | (out_b << 16) | 0xFF00_0000;
    }
}

/// Constant: replace all pixels with a constant packed u32 value.
/// Mode-aware: R byte always from value; G/B only for mode >= 2 (RGB/RGBA);
/// G/B preserved from original for L/LA. Alpha byte from value for alpha
/// modes (LA/RGBA), forced to 0xFF for non-alpha modes.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn constant(pixels: &mut [u32], mode: u32, value: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    let val_r = value & 0xFF;
    let val_g = (value >> 8) & 0xFF;
    let val_b = (value >> 16) & 0xFF;
    let val_a = value & 0xFF00_0000;

    for p in pixels.iter_mut() {
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;

        let out_r = val_r;
        let out_g = if has_gb { val_g } else { g };
        let out_b = if has_gb { val_b } else { b };
        let out_a = if has_a { val_a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

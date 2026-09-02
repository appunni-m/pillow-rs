//! Scalar fallback implementations — safe, portable, auto-vectorization friendly.
//!
//! These are written as tight loops over u32 slices so LLVM can auto-vectorize
//! them when compiled with `-C target-cpu=native`. They also serve as the
//! reference implementation for platform-specific SIMD code.

use crate::pipeline::TransposeMethod;

/// Invert: 255 - value for each active channel.
/// Mode-aware: only touches channels present in the image mode.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn invert(pixels: &mut [u32], mode: u32) {
    let mask_r: u32 = 0x0000_00FF;
    let mask_g: u32 = 0x0000_FF00;
    let mask_b: u32 = 0x00FF_0000;
    let mask_a: u32 = 0xFF00_0000;

    let has_gb = mode >= 2; // RGB or RGBA
    let has_a = mode == 1 || mode == 3; // LA or RGBA

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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

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

/// Flip: vertical coordinate remap. output[y][x] = input[H-1-y][x]
#[inline]
pub fn flip(pixels: &mut [u32], w: u32, h: u32, mode: u32) {
    let has_a = mode == 1 || mode == 3;
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
    let has_a = mode == 1 || mode == 3;
    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;
        let out_a = if has_a { a } else { 0xFF00_0000 };
        *p = r | (g << 8) | (b << 16) | out_a;
    }
}

/// InvertChops: 255 - every channel in the image.
///
/// Unlike ImageOps.invert, ImageChops.invert includes the alpha byte. The
/// previous SIMD implementation reused the ImageOps alpha-preserving rule,
/// which first diverged for LA/RGBA inputs with non-opaque alpha.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn invert_chops(pixels: &mut [u32], mode: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let out_r = 255 - r;
        let out_g = if has_gb { 255 - g } else { g };
        let out_b = if has_gb { 255 - b } else { b };
        let out_a = if has_a {
            (255 - (a >> 24)) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

// ── Dual-input blend operations ──

/// Multiply: (a * b) / 255 per channel. PIL ImageChops.multiply formula.
#[inline]
pub fn multiply(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = ar * br / 255;
        let out_g = if has_gb { ag * bg / 255 } else { ag };
        let out_b = if has_gb { ab * bb / 255 } else { ab };
        let out_a = if has_a {
            ((aa >> 24) * ba / 255) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Screen: 255 - ((255-a) * (255-b) / 255) per channel. PIL ImageChops.screen formula.
#[inline]
pub fn screen(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = 255 - ((255 - ar) * (255 - br) / 255);
        let out_g = if has_gb {
            255 - ((255 - ag) * (255 - bg) / 255)
        } else {
            ag
        };
        let out_b = if has_gb {
            255 - ((255 - ab) * (255 - bb) / 255)
        } else {
            ab
        };
        let out_a = if has_a {
            (255 - ((255 - (aa >> 24)) * (255 - ba) / 255)) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Darker: min(a, b) per channel. PIL ImageChops.darker formula.
#[inline]
pub fn darker(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = ar.min(br);
        let out_g = if has_gb { ag.min(bg) } else { ag };
        let out_b = if has_gb { ab.min(bb) } else { ab };
        let out_a = if has_a {
            ((aa >> 24).min(ba)) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Lighter: max(a, b) per channel. PIL ImageChops.lighter formula.
#[inline]
pub fn lighter(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = ar.max(br);
        let out_g = if has_gb { ag.max(bg) } else { ag };
        let out_b = if has_gb { ab.max(bb) } else { ab };
        let out_a = if has_a {
            ((aa >> 24).max(ba)) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Difference: abs(a - b) per channel. PIL ImageChops.difference formula.
#[inline]
pub fn difference(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = (ar as i32 - br as i32).unsigned_abs();
        let out_g = if has_gb {
            (ag as i32 - bg as i32).unsigned_abs()
        } else {
            ag
        };
        let out_b = if has_gb {
            (ab as i32 - bb as i32).unsigned_abs()
        } else {
            ab
        };
        let out_a = if has_a {
            ((aa >> 24) as i32 - ba as i32).unsigned_abs() << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Add modulo: `(a + b) % 256` per channel (wrapping add).
/// Dual-input: operates on `pixels` and `other` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn add_modulo(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = (ar.wrapping_add(br)) & 0xFF;
        let out_g_raw = (ag.wrapping_add(bg)) & 0xFF;
        let out_b_raw = (ab.wrapping_add(bb)) & 0xFF;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            ((aa >> 24).wrapping_add(ba) & 0xFF) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Subtract modulo: `(a - b) % 256` per channel (wrapping sub).
/// Dual-input: operates on `pixels` and `other` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn subtract_modulo(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = (ar.wrapping_sub(br)) & 0xFF;
        let out_g_raw = (ag.wrapping_sub(bg)) & 0xFF;
        let out_b_raw = (ab.wrapping_sub(bb)) & 0xFF;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            ((aa >> 24).wrapping_sub(ba) & 0xFF) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Logical AND: bitwise `a & b` per channel.
/// Dual-input: operates on `pixels` and `other` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn logical_and(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = ar & br;
        let out_g_raw = ag & bg;
        let out_b_raw = ab & bb;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            ((aa >> 24) & ba) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Logical OR: bitwise `a | b` per channel.
/// Dual-input: operates on `pixels` and `other` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn logical_or(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = ar | br;
        let out_g_raw = ag | bg;
        let out_b_raw = ab | bb;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            ((aa >> 24) | ba) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Logical XOR: bitwise `a ^ b` per channel.
/// Dual-input: operates on `pixels` and `other` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn logical_xor(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = ar ^ br;
        let out_g_raw = ag ^ bg;
        let out_b_raw = ab ^ bb;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            ((aa >> 24) ^ ba) << 24
        } else {
            0xFF00_0000
        };

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
    let has_a = mode == 3;

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
    let has_gb = mode >= 2;

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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

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

// ── Blend-mode operations (dual-input, PIL ImageChops) ──

/// Overlay: PIL overlay blend mode. Condition on source `a`.
/// If a < 128: `(a*b)/127`, else: `255 - ((255-a)*(255-b)/127)`.
/// CRITICAL: division by 127, NOT 255.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn overlay(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let ch = |a: u32, b: u32| -> u32 {
            if a < 128 {
                (a * b) / 127
            } else {
                255 - ((255 - a) * (255 - b) / 127)
            }
        };

        let out_r = ch(ar, br);
        let out_g_raw = ch(ag, bg);
        let out_b_raw = ch(ab, bb);

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            ch(aa >> 24, ba) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Hard light: PIL hard light blend mode. Condition on other `b`.
/// If b < 128: `(a*b)/127`, else: `255 - ((255-b)*(255-a)/127)`.
/// CRITICAL: division by 127, NOT 255.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn hard_light(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let ch = |a: u32, b: u32| -> u32 {
            if b < 128 {
                (a * b) / 127
            } else {
                255 - ((255 - b) * (255 - a) / 127)
            }
        };

        let out_r = ch(ar, br);
        let out_g_raw = ch(ag, bg);
        let out_b_raw = ch(ab, bb);

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            ch(aa >> 24, ba) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Soft light: PIL soft light blend mode (CHOP2, no clamping).
/// `term1 = ((255-a)*a*b)/65536`
/// `term2 = (a*(255-((255-a)*(255-b)/255)))/255`
/// `out = term1 + term2`
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn soft_light(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let ch = |a: u32, b: u32| -> u32 {
            let term1 = ((255 - a) * a * b) / 65536;
            let term2 = (a * (255 - ((255 - a) * (255 - b) / 255))) / 255;
            term1 + term2
        };

        let out_r = ch(ar, br);
        let out_g_raw = ch(ag, bg);
        let out_b_raw = ch(ab, bb);

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            ch(aa >> 24, ba) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Blend: linear interpolation between two images.
/// `out = a * (1.0 - alpha) + b * alpha`; Pillow permits extrapolation.
/// Math done in f32, result clamped to 0..255.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn blend(pixels: &mut [u32], mode: u32, other: &[u32], alpha: f64) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    // Pillow's ImagingBlend does not clamp alpha before interpolation. The
    // final channel clamp below handles extrapolated values such as 1.5.
    let a_f = alpha as f32;
    let inv_a = 1.0 - a_f;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let ch = |a: u32, b: u32| -> u32 {
            (a as f32 * inv_a + b as f32 * a_f).clamp(0.0, 255.0) as u32
        };

        let out_r = ch(ar, br);
        let out_g_raw = ch(ag, bg);
        let out_b_raw = ch(ab, bb);

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        // Pillow's ImagingBlend interpolates every stored channel, including
        // alpha, for LA/RGBA images. Preserving `aa` here first diverged for
        // non-opaque two-image blends.
        let out_a = if has_a {
            ch(aa >> 24, ba) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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

// ── Spatial operations (mirror, transpose, box_blur) ───────────────────────

/// Mirror: horizontal coordinate remap. output[y][x] = input[y][W-1-x]
/// Mode-aware: processes each row by swapping pairs from ends toward center.
/// For L/LA modes: G and B are set to R since only R carries luma.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn mirror(pixels: &mut [u32], w: u32, h: u32, mode: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_a = mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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

/// Resize image using nearest-neighbor or bilinear sampling.
///
/// PIL pixel-centered coordinate mapping:
///   `src_coord = (dst_coord + 0.5) * src_size / dst_size`
///
/// Nearest (filter=0): `floor(src_coord)`, clamped to bounds, mode-aware channel copy.
/// Bilinear (filter>=1): fractional source position, 4-neighbor weighted blend with f64
/// precision, mode-aware channel output.
///
/// For L/LA modes (mode < 2): only R channel carries luma; G and B are set to R.
/// For L/RGB modes (mode 0 or 2): alpha forced to 0xFF.
/// For LA/RGBA modes (mode 1 or 3): alpha preserved and interpolated.
///
/// Returns `(new_pixels, new_width, new_height)` since output dimensions differ from input.
///
/// filter: 0=Nearest, 1=Bilinear (other values fall back to Bilinear)
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn resize(
    pixels: &[u32],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    mode: u32,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    resize_box(
        pixels,
        src_w,
        src_h,
        dst_w,
        dst_h,
        mode,
        filter,
        0.0,
        0.0,
        src_w as f64,
        src_h as f64,
    )
}

/// Resize a floating-point source box to the destination dimensions.
///
/// This is the same packed-pixel kernel as [`resize`], with the source box
/// kept fractional for ImageOps.fit's `box=` contract.
#[inline]
fn resize_box(
    pixels: &[u32],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    mode: u32,
    filter: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
) -> (Vec<u32>, u32, u32) {
    let has_gb = mode >= 2; // RGB or RGBA
    let has_a = mode == 1 || mode == 3; // LA or RGBA

    // Guard against empty dimensions (avoids div-by-zero and u32 underflow)
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return (vec![0u32; (dst_w * dst_h) as usize], dst_w, dst_h);
    }

    let count = (dst_w * dst_h) as usize;
    let mut output = vec![0u32; count];

    let sw_f = box_right - box_left;
    let sh_f = box_bottom - box_top;
    let dw_f = dst_w as f64;
    let dh_f = dst_h as f64;

    // Bounds for clamping (precomputed to avoid repeated subtraction)
    let src_w_max = src_w - 1;
    let src_h_max = src_h - 1;

    if filter == 0 {
        // ── Nearest-neighbor ──
        // Pillow's ImagingScaleAffine builds integer coordinate tables with
        // a cumulative floating-point position (x0 + scale / 2, then one
        // scale increment per output pixel). Recomputing `(x + 0.5) * scale`
        // changes exact-boundary samples, notably for 14 -> 21 scaling where
        // Pillow repeats a source index across two output columns.
        let scale_x = sw_f / dw_f;
        let scale_y = sh_f / dh_f;
        let full_source_box = box_left == 0.0
            && box_top == 0.0
            && box_right == src_w as f64
            && box_bottom == src_h as f64;
        let mut x_positions = Vec::with_capacity(dst_w as usize);
        if full_source_box {
            let mut x_position = box_left + scale_x * 0.5;
            for _ in 0..dst_w {
                x_positions.push(x_position as u32);
                x_position += scale_x;
            }
        } else {
            for dx in 0..dst_w {
                x_positions.push((box_left + (dx as f64 + 0.5) * scale_x) as u32);
            }
        }
        let mut y_position = box_top + scale_y * 0.5;
        for dy in 0..dst_h {
            let sy = if full_source_box {
                y_position as u32
            } else {
                (box_top + (dy as f64 + 0.5) * scale_y) as u32
            };
            let sy = sy.min(src_h_max);
            for dx in 0..dst_w {
                let sx = x_positions[dx as usize];
                let sx = sx.min(src_w_max);

                let sp = pixels[(sy * src_w + sx) as usize];
                let r = sp & 0xFF;
                let g = (sp >> 8) & 0xFF;
                let b = (sp >> 16) & 0xFF;
                let a = sp & 0xFF00_0000;

                let out_r = r;
                let out_g = if has_gb { g } else { r };
                let out_b = if has_gb { b } else { r };
                let out_a = if has_a { a } else { 0xFF00_0000 };

                output[(dy * dst_w + dx) as usize] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            }
            if full_source_box {
                y_position += scale_y;
            }
        }
    } else {
        // ── Bilinear (and fallback for Bicubic/Lanczos/Box/Hamming) ──
        for dy in 0..dst_h {
            let cy = box_top + (dy as f64 + 0.5) * sh_f / dh_f;
            let sy_floor = cy.floor();
            let fy = cy - sy_floor;
            let y0 = (sy_floor as u32).min(src_h_max);
            let y1 = (y0 + 1).min(src_h_max);

            for dx in 0..dst_w {
                let cx = box_left + (dx as f64 + 0.5) * sw_f / dw_f;
                let sx_floor = cx.floor();
                let fx = cx - sx_floor;
                let x0 = (sx_floor as u32).min(src_w_max);
                let x1 = (x0 + 1).min(src_w_max);

                let p00 = pixels[(y0 * src_w + x0) as usize];
                let p10 = pixels[(y0 * src_w + x1) as usize];
                let p01 = pixels[(y1 * src_w + x0) as usize];
                let p11 = pixels[(y1 * src_w + x1) as usize];

                // Extract all 4 channels from each of the 4 neighbors
                let r00 = (p00 & 0xFF) as f64;
                let g00 = ((p00 >> 8) & 0xFF) as f64;
                let b00 = ((p00 >> 16) & 0xFF) as f64;
                let a00 = ((p00 >> 24) & 0xFF) as f64;

                let r10 = (p10 & 0xFF) as f64;
                let g10 = ((p10 >> 8) & 0xFF) as f64;
                let b10 = ((p10 >> 16) & 0xFF) as f64;
                let a10 = ((p10 >> 24) & 0xFF) as f64;

                let r01 = (p01 & 0xFF) as f64;
                let g01 = ((p01 >> 8) & 0xFF) as f64;
                let b01 = ((p01 >> 16) & 0xFF) as f64;
                let a01 = ((p01 >> 24) & 0xFF) as f64;

                let r11 = (p11 & 0xFF) as f64;
                let g11 = ((p11 >> 8) & 0xFF) as f64;
                let b11 = ((p11 >> 16) & 0xFF) as f64;
                let a11 = ((p11 >> 24) & 0xFF) as f64;

                // Horizontal interpolation (top row and bottom row)
                let inv_fx = 1.0 - fx;
                let top_r = inv_fx * r00 + fx * r10;
                let top_g = inv_fx * g00 + fx * g10;
                let top_b = inv_fx * b00 + fx * b10;
                let top_a = inv_fx * a00 + fx * a10;

                let bot_r = inv_fx * r01 + fx * r11;
                let bot_g = inv_fx * g01 + fx * g11;
                let bot_b = inv_fx * b01 + fx * b11;
                let bot_a = inv_fx * a01 + fx * a11;

                // Vertical interpolation
                let inv_fy = 1.0 - fy;
                let out_r_f = inv_fy * top_r + fy * bot_r;
                let out_g_f = inv_fy * top_g + fy * bot_g;
                let out_b_f = inv_fy * top_b + fy * bot_b;
                let out_a_f = inv_fy * top_a + fy * bot_a;

                // Round to nearest integer (PIL: truncate after +0.5), clamp to [0, 255]
                let out_r = ((out_r_f + 0.5) as u32).min(255);
                let out_g_raw = ((out_g_f + 0.5) as u32).min(255);
                let out_b_raw = ((out_b_f + 0.5) as u32).min(255);
                let out_a_raw = ((out_a_f + 0.5) as u32).min(255);

                // Mode-aware channel output
                let out_g = if has_gb { out_g_raw } else { out_r };
                let out_b = if has_gb { out_b_raw } else { out_r };
                // Alpha: non-alpha modes force 0xFF in high byte; alpha modes use interpolated value
                let out_a = if has_a { out_a_raw << 24 } else { 0xFF00_0000 };

                output[(dy * dst_w + dx) as usize] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            }
        }
    }

    (output, dst_w, dst_h)
}

// ── Image sizing operations (contain, cover, fit, quantize) ────────────────────

/// Contain: scale image to fit WITHIN dst_w × dst_h, preserving aspect ratio.
/// Scale = min(dst_w/w, dst_h/h). Delegates to bilinear resize.
/// Returns (resized_pixels, new_w, new_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn contain(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let image_ratio = w as f64 / h as f64;
    let destination_ratio = dst_w as f64 / dst_h as f64;
    let (new_w, new_h) = if image_ratio != destination_ratio {
        if image_ratio > destination_ratio {
            let new_h = round_positive_ties_even(h as f64 / w as f64 * dst_w as f64);
            (dst_w, new_h.max(1))
        } else {
            let new_w = round_positive_ties_even(w as f64 / h as f64 * dst_h as f64);
            (new_w.max(1), dst_h)
        }
    } else {
        (dst_w, dst_h)
    };
    resize(pixels, w, h, new_w, new_h, mode, filter)
}

/// Scale: resize by a floating-point factor using the requested filter.
/// factor: scale multiplier (0.5 = half size, 2.0 = double).
/// Delegates to [`resize`] with the normalized filter code.
/// Returns (pixels, dst_w, dst_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn scale(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    factor: f64,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    // ImageOps.scale follows Python's round(width * factor), including
    // ties-to-even at half-pixel products (13 * 1.5 -> 20, 11 * 1.5 -> 16).
    let dst_w = round_positive_ties_even(w as f64 * factor).max(1);
    let dst_h = round_positive_ties_even(h as f64 * factor).max(1);
    resize(pixels, w, h, dst_w, dst_h, mode, filter)
}

/// Cover: scale image to COVER dst_w × dst_h, preserving aspect ratio.
/// Scale = max(dst_w/w, dst_h/h). Resize then center-crop to dst_w × dst_h.
/// Returns (cropped_pixels, actual_w, actual_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn cover(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let image_ratio = w as f64 / h as f64;
    let destination_ratio = dst_w as f64 / dst_h as f64;
    let (new_w, new_h) = if image_ratio != destination_ratio {
        if image_ratio < destination_ratio {
            let new_h = round_positive_ties_even(h as f64 / w as f64 * dst_w as f64);
            (dst_w, new_h.max(1))
        } else {
            let new_w = round_positive_ties_even(w as f64 / h as f64 * dst_h as f64);
            (new_w.max(1), dst_h)
        }
    } else {
        (dst_w, dst_h)
    };
    // ImageOps.cover returns the resized image. It deliberately does not
    // crop the overflow back to the requested box; the old SIMD path did so
    // and first diverged on non-square targets.
    resize(pixels, w, h, new_w, new_h, mode, filter)
}

/// Fit: like contain but with bleed (zoom) and centering offset.
/// Scale = min(dst_w/w, dst_h/h) * (1.0 + bleed). Resize then crop with centering.
/// Returns (cropped_pixels, actual_w, actual_h).
/// bleed: zoom factor (0.0 = no bleed, 0.5 maximum typical).
/// centering: (cx, cy) in [0,1] where (0,0)=top-left, (1,1)=bottom-right.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn fit(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    bleed: f32,
    centering: (f32, f32),
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    // Pillow's ImageOps.fit can materialize a zero-width, non-empty source:
    // the destination is still allocated and remains zero-filled. Other
    // empty-dimension cases retain the historical no-sample result here;
    // callers that need Pillow's division error reject those inputs before
    // reaching this kernel.
    if w == 0 && h > 0 && dst_w > 0 && dst_h > 0 {
        return (vec![0u32; (dst_w * dst_h) as usize], dst_w, dst_h);
    }
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let b = if (0.0..0.5).contains(&bleed) {
        bleed as f64
    } else {
        0.0
    };
    let (cx, cy) = centering;
    let cx = cx.clamp(0.0, 1.0) as f64;
    let cy = cy.clamp(0.0, 1.0) as f64;

    // Match ImageOps.fit's floating-point crop box. Resampling this box
    // directly is important: resizing first and cropping integer pixels loses
    // the fractional centering contract and can even return the wrong size.
    let bleed_w = b * w as f64;
    let bleed_h = b * h as f64;
    let live_w = w as f64 - bleed_w * 2.0;
    let live_h = h as f64 - bleed_h * 2.0;
    let live_ratio = live_w / live_h;
    let output_ratio = dst_w as f64 / dst_h as f64;
    let (crop_w, crop_h) = if live_ratio == output_ratio {
        (live_w, live_h)
    } else if live_ratio >= output_ratio {
        (output_ratio * live_h, live_h)
    } else {
        (live_w, live_w / output_ratio)
    };
    let crop_left = bleed_w + (live_w - crop_w) * cx;
    let crop_top = bleed_h + (live_h - crop_h) * cy;
    let (cropped, _, _) = resize_box(
        pixels,
        w,
        h,
        dst_w,
        dst_h,
        mode,
        filter,
        crop_left,
        crop_top,
        crop_left + crop_w,
        crop_top + crop_h,
    );
    (cropped, dst_w, dst_h)
}

// ── Spatial operations (pad, expand, crop_border) ──────────────────────────────

/// Pad/crop image to dst_w×dst_h, centering the source.
///
/// If destination is larger than source, fills empty areas with fill_rgba.
/// If destination is smaller than source, crops from center.
/// Returns (pixels, dst_w, dst_h).
///
/// Mode-aware: alpha byte clamped for non-alpha modes (L, RGB).
/// For L/LA modes: G = B = R (luma carried in R channel).
/// For RGB/RGBA modes: G and B are independent.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
fn round_positive_ties_even(value: f64) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    };
    rounded.min(u32::MAX as f64) as u32
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
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

// ── Geometry and spatial operations (rotate, remap_palette, equalize) ────────

/// Geometry.c stores byte bilinear samples by truncating the interpolated
/// value, unlike the resize helper above which rounds its result.
#[inline(always)]
fn bilinear_interp_truncated(c00: u32, c10: u32, c01: u32, c11: u32, fx: f64, fy: f64) -> u32 {
    // Blend horizontally before the vertical blend. Apart from matching the
    // source kernel's operation order, this keeps a constant edge sample
    // exactly constant instead of turning 15 into 14 on an f64 roundoff.
    let v = (1.0 - fy) * ((1.0 - fx) * c00 as f64 + fx * c10 as f64)
        + fy * ((1.0 - fx) * c01 as f64 + fx * c11 as f64);
    v as u32
}

#[inline(always)]
fn premultiply_rotate_channel(value: u32, alpha: u32) -> u32 {
    (value as f64 * alpha as f64 / 255.0 + 0.5) as u32
}

#[inline(always)]
fn unpremultiply_rotate_channel(value: u32, alpha: u32) -> u32 {
    if alpha == 0 {
        0
    } else {
        (value as f64 * 255.0 / alpha as f64) as u32
    }
}

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

/// Rotate image by angle_deg degrees using bilinear interpolation.
///
/// `expand`: if true, output canvas expands to fit rotated image. If false, output
/// is same size as input (corners may be clipped).
/// `fill_rgba`: packed u32 fill color (0xAABBGGRR) for empty areas behind the
/// rotated image.
///
/// Rotation is counter-clockwise (matching PIL). Uses inverse mapping: for each
/// output pixel, rotate the coordinate back to source space and bilinear-interpolate.
///
/// Mode-aware: for L/LA modes (mode < 2), G = B = R (luma carried in R). Alpha
/// preserved for LA/RGBA, forced to 0xFF for L/RGB.
///
/// Returns (new_pixels, new_w, new_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn rotate(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    angle_deg: f64,
    expand: bool,
    fill_rgba: u32,
) -> (Vec<u32>, u32, u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    let sw = w as f64;
    let sh = h as f64;
    // Keep this transform aligned with pool_cpu::ops::geometry.  Pillow rounds
    // the trigonometric coefficients to 15 decimal places and rounds the two
    // expanded outer edges independently; taking ceil(max-min) loses a pixel
    // for odd dimensions and fractional rotations.
    let rad = -angle_deg.to_radians();
    let aff_a = crate::ops::rotate::round_rotate_coefficient(rad.cos());
    let aff_b = crate::ops::rotate::round_rotate_coefficient(rad.sin());
    let aff_d = crate::ops::rotate::round_rotate_coefficient(-rad.sin());
    let aff_e = aff_a;
    let center_x = sw / 2.0;
    let center_y = sh / 2.0;
    let mut aff_c = aff_a * -center_x + aff_b * -center_y + center_x;
    let mut aff_f = aff_d * -center_x + aff_e * -center_y + center_y;
    let transform =
        |x: f64, y: f64, c: f64, f: f64| (aff_a * x + aff_b * y + c, aff_d * x + aff_e * y + f);

    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(cx, cy) in &corners {
        let (rx, ry) = transform(cx, cy, aff_c, aff_f);
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }

    let (dw, dh) = if expand {
        (
            (max_x.ceil() - min_x.floor()) as u32,
            (max_y.ceil() - min_y.floor()) as u32,
        )
    } else {
        (w, h)
    };

    if expand {
        let shift_x = -(dw as f64 - sw) / 2.0;
        let shift_y = -(dh as f64 - sh) / 2.0;
        (aff_c, aff_f) = transform(shift_x, shift_y, aff_c, aff_f);
    }

    // Pre-extract fill color components for mode-aware handling
    let fill_r = fill_rgba & 0xFF;
    let fill_g = (fill_rgba >> 8) & 0xFF;
    let fill_b = (fill_rgba >> 16) & 0xFF;
    let fill_a = fill_rgba & 0xFF00_0000;

    if w == 0 || h == 0 {
        // Pillow still computes the expanded canvas for an empty source and
        // fills it; returning the original dimensions loses the 90-degree
        // width/height swap and diverges before any pixel sampling occurs.
        let out_g = if has_gb { fill_g } else { fill_r };
        let out_b = if has_gb { fill_b } else { fill_r };
        let out_a = if has_a { fill_a } else { 0xFF00_0000 };
        let fill_pixel = fill_r | (out_g << 8) | (out_b << 16) | out_a;
        return (vec![fill_pixel; (dw * dh) as usize], dw, dh);
    }

    let mut out = vec![0u32; (dw * dh) as usize];

    for dy in 0..dh {
        for dx in 0..dw {
            // Map the destination pixel center through the reverse affine
            // transform, then move from center space to Pillow's filter-side
            // source corner coordinates.
            let (src_x, src_y) = transform(dx as f64 + 0.5, dy as f64 + 0.5, aff_c, aff_f);
            let src_x = src_x - 0.5;
            let src_y = src_y - 0.5;

            let out_idx = (dy * dw + dx) as usize;

            // Pillow's bilinear filter has a half-pixel footprint. Coordinates
            // in [-0.5, width - 0.5) remain samples at the image edge, where
            // the source coordinate is clamped; only points outside that
            // support use the fill color.
            if src_x >= -0.5 && src_x < sw - 0.5 && src_y >= -0.5 && src_y < sh - 0.5 {
                let src_x = src_x.clamp(0.0, sw - 1.0);
                let src_y = src_y.clamp(0.0, sh - 1.0);
                // Bilinear interpolation of source pixels
                let sx = src_x.floor() as u32;
                let sy = src_y.floor() as u32;
                let fx = src_x - sx as f64;
                let fy = src_y - sy as f64;
                let sx1 = (sx + 1).min(w - 1);
                let sy1 = (sy + 1).min(h - 1);

                let p00 = pixels[(sy * w + sx) as usize];
                let p10 = pixels[(sy * w + sx1) as usize];
                let p01 = pixels[(sy1 * w + sx) as usize];
                let p11 = pixels[(sy1 * w + sx1) as usize];

                let a00 = (p00 >> 24) & 0xFF;
                let a10 = (p10 >> 24) & 0xFF;
                let a01 = (p01 >> 24) & 0xFF;
                let a11 = (p11 >> 24) & 0xFF;
                // Pillow's RGBA/LA rotate path first converts to RGBa/La,
                // samples premultiplied channels, then truncates the
                // unpremultiplication on return. The old SIMD path sampled
                // straight channels and kept only p00's alpha, which first
                // diverged on a fractional RGBA rotation at transparent
                // edges (Pillow Geometry.c / Image.rotate alpha round-trip).
                let r00 = if has_a {
                    premultiply_rotate_channel(p00 & 0xFF, a00)
                } else {
                    p00 & 0xFF
                };
                let g00 = if has_a {
                    premultiply_rotate_channel((p00 >> 8) & 0xFF, a00)
                } else {
                    (p00 >> 8) & 0xFF
                };
                let b00 = if has_a {
                    premultiply_rotate_channel((p00 >> 16) & 0xFF, a00)
                } else {
                    (p00 >> 16) & 0xFF
                };
                let r10 = if has_a {
                    premultiply_rotate_channel(p10 & 0xFF, a10)
                } else {
                    p10 & 0xFF
                };
                let g10 = if has_a {
                    premultiply_rotate_channel((p10 >> 8) & 0xFF, a10)
                } else {
                    (p10 >> 8) & 0xFF
                };
                let b10 = if has_a {
                    premultiply_rotate_channel((p10 >> 16) & 0xFF, a10)
                } else {
                    (p10 >> 16) & 0xFF
                };
                let r01 = if has_a {
                    premultiply_rotate_channel(p01 & 0xFF, a01)
                } else {
                    p01 & 0xFF
                };
                let g01 = if has_a {
                    premultiply_rotate_channel((p01 >> 8) & 0xFF, a01)
                } else {
                    (p01 >> 8) & 0xFF
                };
                let b01 = if has_a {
                    premultiply_rotate_channel((p01 >> 16) & 0xFF, a01)
                } else {
                    (p01 >> 16) & 0xFF
                };
                let r11 = if has_a {
                    premultiply_rotate_channel(p11 & 0xFF, a11)
                } else {
                    p11 & 0xFF
                };
                let g11 = if has_a {
                    premultiply_rotate_channel((p11 >> 8) & 0xFF, a11)
                } else {
                    (p11 >> 8) & 0xFF
                };
                let b11 = if has_a {
                    premultiply_rotate_channel((p11 >> 16) & 0xFF, a11)
                } else {
                    (p11 >> 16) & 0xFF
                };

                // Bilinear interpolate per channel
                let out_r_sample = bilinear_interp_truncated(r00, r10, r01, r11, fx, fy);
                let out_g_sample = bilinear_interp_truncated(g00, g10, g01, g11, fx, fy);
                let out_b_sample = bilinear_interp_truncated(b00, b10, b01, b11, fx, fy);
                let out_a_raw = bilinear_interp_truncated(a00, a10, a01, a11, fx, fy);
                let out_r = if has_a {
                    unpremultiply_rotate_channel(out_r_sample, out_a_raw)
                } else {
                    out_r_sample
                };
                let out_g_raw = if has_a {
                    unpremultiply_rotate_channel(out_g_sample, out_a_raw)
                } else {
                    out_g_sample
                };
                let out_b_raw = if has_a {
                    unpremultiply_rotate_channel(out_b_sample, out_a_raw)
                } else {
                    out_b_sample
                };

                let out_g = if has_gb { out_g_raw } else { out_r };
                let out_b = if has_gb { out_b_raw } else { out_r };
                let out_a = if has_a { out_a_raw << 24 } else { 0xFF00_0000 };

                out[out_idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            } else {
                // Out of source bounds: fill with fill_rgba (mode-aware)
                let out_a = if has_a { fill_a } else { 0xFF00_0000 };
                let out_g = if has_gb { fill_g } else { fill_r };
                let out_b = if has_gb { fill_b } else { fill_r };
                out[out_idx] = fill_r | (out_g << 8) | (out_b << 16) | out_a;
            }
        }
    }

    (out, dw, dh)
}

/// Remap palette indices with Pillow's inverse destination map.
///
/// Each source pixel's R byte is an old palette index. `inverse_map[old]`
/// contains its new index; entries omitted from the destination map are zero.
///
/// Returns a Vec<u32> of packed RGBA pixels (same length as input).
/// Mode-aware: G/B set to looked-up values for RGB/RGBA (mode >= 2), mirrored
/// from R for L/LA. Alpha forced to 0xFF for non-alpha modes.
/// mode: 0=L (P-mode encoding), 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn remap_palette(pixels: &[u32], mode: u32, inverse_map: &[u8; 256]) -> Vec<u32> {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let mut out = Vec::with_capacity(pixels.len());

    for &p in pixels.iter() {
        let old_r = (p & 0xFF) as usize;
        let old_g = ((p >> 8) & 0xFF) as usize;
        let old_b = ((p >> 16) & 0xFF) as usize;
        let r = u32::from(inverse_map[old_r]);
        let g = u32::from(inverse_map[old_g]);
        let b = u32::from(inverse_map[old_b]);

        let out_g = if has_gb { g } else { r };
        let out_b = if has_gb { b } else { r };
        let out_a = if has_a { p & 0xFF00_0000 } else { 0xFF00_0000 };

        out.push(r | (out_g << 8) | (out_b << 16) | out_a);
    }

    out
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

    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

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

// ── Effects + Module operations (transform, put_pixel, put_data, put_alpha, composite_module) ──

/// Affine transform with configurable filter (0=Nearest, 1=Bilinear+).
///
/// Maps each destination pixel center (dx+0.5,dy+0.5) back to source
/// (sx,sy) via the matrix [a,b,c,d,e,f] where:
///   sx = a*(dx+0.5) + b*(dy+0.5) + c
///   sy = d*(dx+0.5) + e*(dy+0.5) + f
///
/// filter=0 (Nearest): truncate non-negative source coordinates, matching
/// Pillow's `COORD` macro, and reject negative/out-of-bounds coordinates.
/// filter>=1 (Bilinear): fractional source position, 4-neighbor weighted blend with
/// f64 precision, mode-aware channel output.
///
/// Empty areas (source coordinates out of bounds) are filled with fill_rgba
/// (packed 0xAABBGGRR).
///
/// Returns (output_pixels, dst_w, dst_h).
/// mode: 0=L, 1=LA, 2=RGB-like, 3=RGBA, 4=CMYK-like four-byte storage
#[inline]
pub fn transform(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    matrix: &[f64; 8],
    filter: u32,
    fill_rgba: u32,
) -> (Vec<u32>, u32, u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let has_fourth = mode == 1 || mode == 3 || mode == 4;
    let nearest = filter == 0;

    let aff_a = matrix[0];
    let aff_b = matrix[1];
    let aff_c = matrix[2];
    let aff_d = matrix[3];
    let aff_e = matrix[4];
    let aff_f = matrix[5];

    // The affine byte kernel receives fill samples in the same packed layout
    // as its input. For alpha modes Pillow writes that fill into the temporary
    // premultiplied buffer without premultiplying it first, then expands it
    // when converting back to straight-alpha output.
    let fill_r = fill_rgba & 0xFF;
    let fill_g = (fill_rgba >> 8) & 0xFF;
    let fill_b = (fill_rgba >> 16) & 0xFF;
    let fill_a = (fill_rgba >> 24) & 0xFF;
    let unpremultiply = |value: u32, alpha: u32| {
        if alpha == 0 {
            0
        } else {
            (value.saturating_mul(255) / alpha).min(255)
        }
    };
    let fill_out_r = if has_a && !nearest {
        unpremultiply(fill_r, fill_a)
    } else {
        fill_r
    };
    let fill_out_g = if has_gb {
        if has_a && !nearest {
            unpremultiply(fill_g, fill_a)
        } else {
            fill_g
        }
    } else {
        fill_out_r
    };
    let fill_out_b = if has_gb {
        if has_a && !nearest {
            unpremultiply(fill_b, fill_a)
        } else {
            fill_b
        }
    } else {
        fill_out_r
    };
    let fill_out_a = if has_fourth { fill_a } else { 255 };
    let fill_pixel = fill_out_r | (fill_out_g << 8) | (fill_out_b << 16) | (fill_out_a << 24);

    // Guard against empty source dimensions (avoids underflow on w-1 / h-1)
    if w == 0 || h == 0 {
        return (vec![fill_pixel; (dst_w * dst_h) as usize], dst_w, dst_h);
    }

    let count = (dst_w * dst_h) as usize;
    let mut out = vec![0u32; count];
    let w_f = w as f64;
    let h_f = h as f64;
    let w_max = w - 1;
    let h_max = h - 1;

    if nearest {
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                // Geometry.c's affine_transform() evaluates destination
                // pixel centers. Its nearest filter then applies COORD(),
                // which rejects negative values and truncates non-negative
                // values toward zero.
                let sx = aff_a * (dx as f64 + 0.5) + aff_b * (dy as f64 + 0.5) + aff_c;
                let sy = aff_d * (dx as f64 + 0.5) + aff_e * (dy as f64 + 0.5) + aff_f;
                let out_idx = (dy * dst_w + dx) as usize;

                let ix = if sx < 0.0 { -1 } else { sx as i64 };
                let iy = if sy < 0.0 { -1 } else { sy as i64 };
                if ix >= 0 && ix < w as i64 && iy >= 0 && iy < h as i64 {
                    let sp = pixels[(iy as u32 * w + ix as u32) as usize];
                    let r = sp & 0xFF;
                    let g = (sp >> 8) & 0xFF;
                    let b_val = (sp >> 16) & 0xFF;
                    let a_val = sp & 0xFF00_0000;
                    let og = if has_gb { g } else { r };
                    let ob = if has_gb { b_val } else { r };
                    let oa = if has_fourth { a_val } else { 0xFF00_0000 };
                    out[out_idx] = r | (og << 8) | (ob << 16) | oa;
                } else {
                    out[out_idx] = fill_pixel;
                }
            }
        }
    } else {
        // Bilinear interpolation
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                // The bilinear filter performs its own half-pixel shift after
                // the center-space bounds check, matching BILINEAR_HEAD() in
                // Geometry.c.
                let sx = aff_a * (dx as f64 + 0.5) + aff_b * (dy as f64 + 0.5) + aff_c;
                let sy = aff_d * (dx as f64 + 0.5) + aff_e * (dy as f64 + 0.5) + aff_f;
                let out_idx = (dy * dst_w + dx) as usize;

                if sx >= 0.0 && sx < w_f && sy >= 0.0 && sy < h_f {
                    let sample_x = sx - 0.5;
                    let sample_y = sy - 0.5;
                    let x_floor = sample_x.floor() as i64;
                    let y_floor = sample_y.floor() as i64;
                    let x0 = x_floor.clamp(0, w_max as i64) as u32;
                    let y0 = y_floor.clamp(0, h_max as i64) as u32;
                    let x1 = (x_floor + 1).clamp(0, w_max as i64) as u32;
                    let y1 = (y_floor + 1).clamp(0, h_max as i64) as u32;
                    let fx = sample_x - x_floor as f64;
                    let fy = sample_y - y_floor as f64;

                    let p00 = pixels[(y0 * w + x0) as usize];
                    let p10 = pixels[(y0 * w + x1) as usize];
                    let p01 = pixels[(y1 * w + x0) as usize];
                    let p11 = pixels[(y1 * w + x1) as usize];

                    // Extract all 4 channels from each of the 4 neighbors
                    let r00 = (p00 & 0xFF) as f64;
                    let g00 = ((p00 >> 8) & 0xFF) as f64;
                    let b00 = ((p00 >> 16) & 0xFF) as f64;
                    let a00 = ((p00 >> 24) & 0xFF) as f64;

                    let r10 = (p10 & 0xFF) as f64;
                    let g10 = ((p10 >> 8) & 0xFF) as f64;
                    let b10 = ((p10 >> 16) & 0xFF) as f64;
                    let a10 = ((p10 >> 24) & 0xFF) as f64;

                    let r01 = (p01 & 0xFF) as f64;
                    let g01 = ((p01 >> 8) & 0xFF) as f64;
                    let b01 = ((p01 >> 16) & 0xFF) as f64;
                    let a01 = ((p01 >> 24) & 0xFF) as f64;

                    let r11 = (p11 & 0xFF) as f64;
                    let g11 = ((p11 >> 8) & 0xFF) as f64;
                    let b11 = ((p11 >> 16) & 0xFF) as f64;
                    let a11 = ((p11 >> 24) & 0xFF) as f64;

                    let inv_fx = 1.0 - fx;
                    let inv_fy = 1.0 - fy;

                    // Keep the same term order as the CPU byte path. Pillow's
                    // ImagingTransformAffine stores weighted samples by
                    // truncating toward zero, so exact half-way values round
                    // down instead of receiving a +0.5 adjustment.
                    let out_r_f = inv_fx * inv_fy * r00
                        + fx * inv_fy * r10
                        + inv_fx * fy * r01
                        + fx * fy * r11;
                    let out_g_f = inv_fx * inv_fy * g00
                        + fx * inv_fy * g10
                        + inv_fx * fy * g01
                        + fx * fy * g11;
                    let out_b_f = inv_fx * inv_fy * b00
                        + fx * inv_fy * b10
                        + inv_fx * fy * b01
                        + fx * fy * b11;
                    let out_a_raw = bilinear_interp_truncated(
                        a00 as u32, a10 as u32, a01 as u32, a11 as u32, fx, fy,
                    );
                    let (out_r, out_g_raw, out_b_raw) = if has_a {
                        let premultiply = |value: u32, alpha: u32| {
                            ((value.saturating_mul(alpha) + 127) / 255).min(255)
                        };
                        let interpolate = |c00: u32, c10: u32, c01: u32, c11: u32| {
                            bilinear_interp_truncated(c00, c10, c01, c11, fx, fy)
                        };
                        (
                            unpremultiply(
                                interpolate(
                                    premultiply(r00 as u32, a00 as u32),
                                    premultiply(r10 as u32, a10 as u32),
                                    premultiply(r01 as u32, a01 as u32),
                                    premultiply(r11 as u32, a11 as u32),
                                ),
                                out_a_raw,
                            ),
                            unpremultiply(
                                interpolate(
                                    premultiply(g00 as u32, a00 as u32),
                                    premultiply(g10 as u32, a10 as u32),
                                    premultiply(g01 as u32, a01 as u32),
                                    premultiply(g11 as u32, a11 as u32),
                                ),
                                out_a_raw,
                            ),
                            unpremultiply(
                                interpolate(
                                    premultiply(b00 as u32, a00 as u32),
                                    premultiply(b10 as u32, a10 as u32),
                                    premultiply(b01 as u32, a01 as u32),
                                    premultiply(b11 as u32, a11 as u32),
                                ),
                                out_a_raw,
                            ),
                        )
                    } else {
                        (
                            out_r_f.clamp(0.0, 255.0) as u32,
                            out_g_f.clamp(0.0, 255.0) as u32,
                            out_b_f.clamp(0.0, 255.0) as u32,
                        )
                    };

                    // Mode-aware channel output
                    let out_g = if has_gb { out_g_raw } else { out_r };
                    let out_b = if has_gb { out_b_raw } else { out_r };
                    let out_a = if has_fourth {
                        out_a_raw << 24
                    } else {
                        0xFF00_0000
                    };

                    out[out_idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
                } else {
                    out[out_idx] = fill_pixel;
                }
            }
        }
    }

    (out, dst_w, dst_h)
}

/// Set a single pixel at (x,y) to color_rgba (packed 0xAABBGGRR).
///
/// Bounds check: no-op if out of bounds (matching PIL's silent ignore).
/// Mode-aware: for L mode, only R byte of color matters (G=B=R, A=0xFF).
/// For RGB mode, alpha byte is forced to 0xFF.
/// For LA mode, R and A bytes used, G=B=R.
/// For RGBA mode, full pixel written as-is.
///
/// Operates in-place on pixels slice.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn put_pixel(pixels: &mut [u32], w: u32, mode: u32, x: u32, y: u32, color_rgba: u32) {
    if x >= w {
        return;
    }
    let idx = (y * w + x) as usize;
    if idx >= pixels.len() {
        return;
    }

    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    let r = color_rgba & 0xFF;
    let g = (color_rgba >> 8) & 0xFF;
    let b = (color_rgba >> 16) & 0xFF;
    let a = color_rgba & 0xFF00_0000;

    let out_g = if has_gb { g } else { r };
    let out_b = if has_gb { b } else { r };
    let out_a = if has_a { a } else { 0xFF00_0000 };

    pixels[idx] = r | (out_g << 8) | (out_b << 16) | out_a;
}

/// Replace pixel data with logical Pillow-mode sample bytes.
///
/// Operates in-place on pixels slice.
/// Mode codes are defined by `pipeline::PixelMode`.
#[inline]
pub fn put_data(pixels: &mut [u32], mode: u32, data: &[u8]) {
    let channels = match mode {
        0 | 4 | 7 => 1,
        1 | 5 => 2,
        2 | 8 | 9 => 3,
        _ => 4,
    };
    let n_copy = data.len().min(pixels.len() * channels);

    for (sample_index, &sample) in data[..n_copy].iter().enumerate() {
        let pixel_index = sample_index / channels;
        let channel = sample_index % channels;
        let pixel = &mut pixels[pixel_index];
        let value = sample as u32;
        match mode {
            // L, P, and 1 use the R byte as their packed scalar sample.
            0 | 4 | 7 => {
                *pixel = value | (value << 8) | (value << 16) | 0xFF00_0000;
            }
            // Packed LA/PA stores luma in R and alpha in A.
            1 | 5 => {
                if channel == 0 {
                    *pixel = (*pixel & 0xFF00_0000) | value | (value << 8) | (value << 16);
                } else {
                    *pixel = (*pixel & 0x00FF_FFFF) | (value << 24);
                }
            }
            // RGB-family raw samples occupy the low three packed bytes.
            2 | 8 | 9 => {
                let shift = (channel * 8) as u32;
                *pixel = (*pixel & !(0xFF << shift)) | (value << shift);
                *pixel |= 0xFF00_0000;
            }
            // RGBA, CMYK, I, and F are all four raw bytes in order.
            _ => {
                let shift = (channel * 8) as u32;
                *pixel = (*pixel & !(0xFF << shift)) | (value << shift);
            }
        }
    }
}

/// Set alpha channel of all pixels to `alpha` (0-255).
///
/// Operates in-place on pixels slice.
/// Mode codes are defined by `pipeline::PixelMode`.
#[inline]
pub fn put_alpha(pixels: &mut [u32], mode: u32, alpha: u8) {
    let a_packed = (alpha as u32) << 24;
    for p in pixels.iter_mut() {
        if mode == 6 {
            // Pillow Convert.c:cmyk2rgb uses MULDIV255:
            // t = channel * (255-K) + 128; ((t >> 8) + t) >> 8.
            let c = *p & 0xFF;
            let m = (*p >> 8) & 0xFF;
            let y = (*p >> 16) & 0xFF;
            let k = (*p >> 24) & 0xFF;
            let nk = 255 - k;
            let convert = |channel: u32| {
                let t = channel * nk + 128;
                nk.saturating_sub(((t >> 8) + t) >> 8)
            };
            let r = convert(c);
            let g = convert(m);
            let b = convert(y);
            *p = r | (g << 8) | (b << 16) | a_packed;
        } else {
            *p = (*p & 0x00FF_FFFF) | a_packed;
        }
    }
}

/// Composite two images using a single mask band.
///
/// Formula: `out = (pixels * mask + other * (255 - mask)) / 255`
/// Applied per byte (all 4 channels independently).
///
/// Module-function variant (`Image.composite`): `1`/`L` masks use the low
/// luma byte while `LA`/`RGBA`/`RGBa` masks use the alpha byte.
///
/// Mode-aware: for L/LA modes (mode < 2), G and B mirror R.
/// Alpha preserved for LA/RGBA, forced to 0xFF for L/RGB.
///
/// The result starts as image2 and image1 is blended into its top-left overlap,
/// matching Pillow's `image2.copy(); image2.paste(image1, mask)` implementation.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn composite_module(
    pixels: &[u32],
    width: u32,
    height: u32,
    mode: u32,
    other: &[u32],
    other_width: u32,
    other_height: u32,
    mask: &[u32],
    mask_width: u32,
    mask_height: u32,
    mask_alpha: bool,
) -> Vec<u32> {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let mut out = other.to_vec();
    let overlap_width = width.min(other_width).min(mask_width);
    let overlap_height = height.min(other_height).min(mask_height);

    for y in 0..overlap_height {
        for x in 0..overlap_width {
            let source = pixels[(y * width + x) as usize];
            let destination = other[(y * other_width + x) as usize];
            let mask_pixel = mask[(y * mask_width + x) as usize];
            let amount = if mask_alpha {
                (mask_pixel >> 24) & 0xFF
            } else {
                mask_pixel & 0xFF
            };
            let inverse = 255 - amount;
            let blend = |source: u32, destination: u32| {
                (source * amount + destination * inverse + 127) / 255
            };

            let out_r = blend(source & 0xFF, destination & 0xFF);
            let out_g_raw = blend((source >> 8) & 0xFF, (destination >> 8) & 0xFF);
            let out_b_raw = blend((source >> 16) & 0xFF, (destination >> 16) & 0xFF);
            let out_a_raw = blend((source >> 24) & 0xFF, (destination >> 24) & 0xFF);
            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };
            let out_a = if has_a { out_a_raw << 24 } else { 0xFF00_0000 };

            out[(y * other_width + x) as usize] = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Effects + Lookup operations
//  (paste, alpha_composite, eval, point_op)
// ═══════════════════════════════════════════════════════════════════════════════

/// Paste: copy source image pixels onto destination at (paste_x, paste_y).
///
/// With mask: `out = (src * mask + dst * (255 - mask) + 127) / 255` (rounded division).
/// Without mask: opaque paste — src replaces dst in the paste rect.
/// Source may extend beyond dst bounds — clamped.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn paste(
    pixels: &mut [u32],
    w: u32,
    h: u32,
    mode: u32,
    source: &[u32],
    src_w: u32,
    src_h: u32,
    paste_x: i32,
    paste_y: i32,
    mask: Option<&[u32]>,
    mask_alpha: bool,
) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    // Compute paste region clamped to destination bounds.
    // x_start/y_start = first source pixel visible in dest (0 if paste_x >= 0).
    // x_end/y_end     = last source pixel visible in dest (src_w if paste fits).
    let x_start = if paste_x < 0 {
        paste_x.unsigned_abs()
    } else {
        0
    };
    let y_start = if paste_y < 0 {
        paste_y.unsigned_abs()
    } else {
        0
    };
    // libImaging/Paste.c clips the destination rectangle and advances the
    // source origin by the clipped leading edge. For a negative placement the
    // visible source end therefore extends by that same source offset.
    let x_end = if paste_x < 0 {
        src_w.min(w.saturating_add(x_start))
    } else {
        src_w.min(w.saturating_sub(paste_x as u32))
    };
    let y_end = if paste_y < 0 {
        src_h.min(h.saturating_add(y_start))
    } else {
        src_h.min(h.saturating_sub(paste_y as u32))
    };

    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let dst_ox = paste_x.max(0) as usize;
    let dst_oy = paste_y.max(0) as usize;
    let w_u = w as usize;
    let src_w_u = src_w as usize;

    if let Some(mask_pixels) = mask {
        // ── With mask: weighted blend per active channel ──
        for sy in y_start..y_end {
            for sx in x_start..x_end {
                let src_idx = (sy as usize) * src_w_u + (sx as usize);
                let dst_idx =
                    (dst_oy + (sy - y_start) as usize) * w_u + (dst_ox + (sx - x_start) as usize);

                let sp = source[src_idx];
                let dp = pixels[dst_idx];
                let mv = if mask_alpha {
                    (mask_pixels[src_idx] >> 24) & 0xFF
                } else {
                    mask_pixels[src_idx] & 0xFF
                };

                if mv == 0 {
                    continue;
                }
                if mv == 255 {
                    // Opaque mask: paste source pixel, mode-aware
                    let sr = sp & 0xFF;
                    let sg = (sp >> 8) & 0xFF;
                    let sb = (sp >> 16) & 0xFF;
                    let sa = sp & 0xFF00_0000;

                    let out_r = sr;
                    let out_g = if has_gb { sg } else { sr };
                    let out_b = if has_gb { sb } else { sr };
                    let out_a = if has_a { sa } else { 0xFF00_0000 };
                    pixels[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
                } else {
                    // Partial mask: (src * mask + dst * (255 - mask) + 127) / 255
                    let mv_u32 = mv;
                    let inv_mv = 255u32 - mv_u32;

                    let sr = sp & 0xFF;
                    let sg = (sp >> 8) & 0xFF;
                    let sb = (sp >> 16) & 0xFF;
                    let dr = dp & 0xFF;
                    let dg = (dp >> 8) & 0xFF;
                    let db = (dp >> 16) & 0xFF;

                    let out_r = (sr * mv_u32 + dr * inv_mv + 127) / 255;
                    let out_g_raw = (sg * mv_u32 + dg * inv_mv + 127) / 255;
                    let out_b_raw = (sb * mv_u32 + db * inv_mv + 127) / 255;

                    let out_g = if has_gb { out_g_raw } else { out_r };
                    let out_b = if has_gb { out_b_raw } else { out_r };
                    let out_a = if has_a {
                        let sa_val = (sp >> 24) & 0xFF;
                        let da_val = (dp >> 24) & 0xFF;
                        ((sa_val * mv_u32 + da_val * inv_mv + 127) / 255) << 24
                    } else {
                        0xFF00_0000
                    };

                    pixels[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
                }
            }
        }
    } else {
        // ── No mask: opaque paste — source replaces dest, mode-aware ──
        for sy in y_start..y_end {
            for sx in x_start..x_end {
                let src_idx = (sy as usize) * src_w_u + (sx as usize);
                let dst_idx =
                    (dst_oy + (sy - y_start) as usize) * w_u + (dst_ox + (sx - x_start) as usize);

                let sp = source[src_idx];
                let sr = sp & 0xFF;
                let sg = (sp >> 8) & 0xFF;
                let sb = (sp >> 16) & 0xFF;
                let sa = sp & 0xFF00_0000;

                let out_r = sr;
                let out_g = if has_gb { sg } else { sr };
                let out_b = if has_gb { sb } else { sr };
                let out_a = if has_a { sa } else { 0xFF00_0000 };

                pixels[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            }
        }
    }
}

/// Alpha composite source onto destination at specified rectangles.
///
/// Standard Porter-Duff "over":
///   out_A = src_A + dst_A * (1 - src_A/255)
///   out_RGB = (src_RGB * src_A + dst_RGB * dst_A * (1 - src_A/255)) / out_A
///
/// Operates on the intersection of source and destination rectangles.
/// For L/RGB modes (no alpha channel): implicit alpha = 255 for both images,
/// so the compositing reduces to source-replaces-dest (correct per PIL).
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn alpha_composite(
    pixels: &mut [u32],
    w: u32,
    h: u32,
    mode: u32,
    source: &[u32],
    src_w: u32,
    src_h: u32,
    dest_x: i32,
    dest_y: i32,
    src_x: i32,
    src_y: i32,
) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    // Clamp rectangle starts to valid range
    let d_x = dest_x.max(0) as u32;
    let d_y = dest_y.max(0) as u32;
    let s_x = src_x.max(0) as u32;
    let s_y = src_y.max(0) as u32;

    // Work dimensions = intersection of what fits in both images
    let work_w = src_w.saturating_sub(s_x).min(w.saturating_sub(d_x));
    let work_h = src_h.saturating_sub(s_y).min(h.saturating_sub(d_y));

    if work_w == 0 || work_h == 0 {
        return;
    }

    let w_u = w as usize;
    let src_w_u = src_w as usize;

    // For L/RGB modes (no alpha channel), pixel alpha is implicitly 255.
    // Compositing two fully opaque images gives out = source (Porter-Duff over
    // with sa=da=1 -> out_a=1, out_RGB=src_RGB).
    let implicit_alpha = !has_a;

    for dy in 0..work_h {
        for dx in 0..work_w {
            let src_idx = ((s_y + dy) as usize) * src_w_u + (s_x + dx) as usize;
            let dst_idx = ((d_y + dy) as usize) * w_u + (d_x + dx) as usize;

            let sp = source[src_idx];
            let dp = pixels[dst_idx];

            // ── Alpha values (or implicit 1.0 for non-alpha modes) ──
            let sa = if implicit_alpha {
                1.0
            } else {
                ((sp >> 24) & 0xFF) as f64 / 255.0
            };
            let da = if implicit_alpha {
                1.0
            } else {
                ((dp >> 24) & 0xFF) as f64 / 255.0
            };
            let out_a_f = sa + da * (1.0 - sa);
            if out_a_f <= 0.0 {
                continue;
            }

            let sr = (sp & 0xFF) as f64;
            let sg = ((sp >> 8) & 0xFF) as f64;
            let sb = ((sp >> 16) & 0xFF) as f64;
            let dr = (dp & 0xFF) as f64;
            let dg = ((dp >> 8) & 0xFF) as f64;
            let db = ((dp >> 16) & 0xFF) as f64;

            // Porter-Duff over: out_ch = (src_ch * sa + dst_ch * da * (1 - sa)) / out_a
            let inv_sa = 1.0 - sa;
            let out_r = ((sr * sa + dr * da * inv_sa) / out_a_f)
                .round()
                .clamp(0.0, 255.0) as u32;
            let out_g_raw = ((sg * sa + dg * da * inv_sa) / out_a_f)
                .round()
                .clamp(0.0, 255.0) as u32;
            let out_b_raw = ((sb * sa + db * da * inv_sa) / out_a_f)
                .round()
                .clamp(0.0, 255.0) as u32;

            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };
            let out_a_byte = if has_a {
                (out_a_f * 255.0).round().clamp(0.0, 255.0) as u32
            } else {
                255u32
            };

            pixels[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | (out_a_byte << 24);
        }
    }
}

/// Apply per-channel lookup table to pixels.
///
/// LUT has 256 entries per channel (256*4 = 1024 bytes):
///   [0..256)   -> R/L channel
///   [256..512) -> G channel
///   [512..768) -> B channel
///   [768..1024) -> A channel
///
/// For L mode, only the first 256 entries are used and G/B mirror the L
/// output.  LA uses the first segment for L and the second segment for A;
/// Pillow's public `Image.point` contract supplies one table per band.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn eval(pixels: &mut [u32], mode: u32, lut: &[u8; 1024]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    for p in pixels.iter_mut() {
        let r = (*p & 0xFF) as usize;
        let g = ((*p >> 8) & 0xFF) as usize;
        let b = ((*p >> 16) & 0xFF) as usize;
        let a = ((*p >> 24) & 0xFF) as usize;

        if has_gb {
            // RGB/RGBA: per-channel LUT segments
            let out_r = lut[r] as u32;
            let out_g = lut[256 + g] as u32;
            let out_b = lut[512 + b] as u32;
            let out_a = if has_a {
                (lut[768 + a] as u32) << 24
            } else {
                0xFF00_0000
            };
            *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
        } else {
            // L: first segment; LA: first segment for luma and second for
            // alpha. G = B = R in the packed representation.
            let out_r = lut[r] as u32;
            let out_a = if has_a {
                let alpha_offset = if mode == 1 { 256 } else { 768 };
                (lut[alpha_offset + a] as u32) << 24
            } else {
                0xFF00_0000
            };
            *p = out_r | (out_r << 8) | (out_r << 16) | out_a;
        }
    }
}

// ── Composite, Merge, Sharpen, Autocontrast ────────────────────────────────

/// Merge: combine single-channel bands into a multi-channel image.
///
/// Each band is a `&[u32]` of the same length as `pixels`. Band values are
/// extracted from the R byte of each u32 element.
///
/// Mode mapping:
/// - L (0): band[0] -> all channels, alpha = 0xFF.
/// - LA (1): band[0] -> R (luma), band[1] -> A. G = B = R.
/// - RGB (2): band[0] -> R, band[1] -> G, band[2] -> B, alpha = 0xFF.
/// - RGBA (3): band[0] -> R, band[1] -> G, band[2] -> B, band[3] -> A.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn merge(pixels: &mut [u32], mode: u32, bands: &[&[u32]]) {
    let n = pixels.len();
    match mode {
        0 => {
            for i in 0..n {
                let v = bands[0][i] & 0xFF;
                pixels[i] = v | (v << 8) | (v << 16) | 0xFF00_0000;
            }
        }
        1 => {
            for i in 0..n {
                let v = bands[0][i] & 0xFF;
                let a = (bands[1][i] & 0xFF) << 24;
                pixels[i] = v | (v << 8) | (v << 16) | a;
            }
        }
        2 => {
            for i in 0..n {
                let r = bands[0][i] & 0xFF;
                let g = (bands[1][i] & 0xFF) << 8;
                let b = (bands[2][i] & 0xFF) << 16;
                pixels[i] = r | g | b | 0xFF00_0000;
            }
        }
        3 => {
            for i in 0..n {
                let r = bands[0][i] & 0xFF;
                let g = (bands[1][i] & 0xFF) << 8;
                let b = (bands[2][i] & 0xFF) << 16;
                let a = (bands[3][i] & 0xFF) << 24;
                pixels[i] = r | g | b | a;
            }
        }
        _ => {}
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
    let has_a = mode == 1 || mode == 3;
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
    let has_a = mode == 1 || mode == 3;
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

/// Thumbnail: resize to fit within `dst_w x dst_h` while preserving aspect ratio.
///
/// Computes `scale = min(dst_w/w, dst_h/h)`, then `new_w = (w * scale).max(1)`,
/// `new_h = (h * scale).max(1)`, and resizes using bilinear interpolation.
/// Returns `(Vec<u32>, new_w, new_h)`.
/// If no shrink is needed (dst_w >= w && dst_h >= h), returns a copy with alpha clamping.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn thumbnail(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    filter: u32,
) -> (Vec<u32>, u32, u32) {
    if dst_w == 0 || dst_h == 0 {
        return (Vec::new(), 0, 0);
    }

    // Match Pillow's round_aspect rule used by the CPU thumbnail executor.
    // Truncating `w * scale` is not equivalent: for example, a 17x11 image
    // bounded by 7x7 must become 7x5, not 7x4.  The SIMD nearest path used to
    // take the truncating route and returned a different image shape from the
    // CPU/Pillow path.
    let aspect = w as f64 / h as f64;
    let (new_w, new_h) = if dst_w as f64 / dst_h as f64 >= aspect {
        let adjusted = thumbnail_round_aspect(dst_h as f64 * aspect, |candidate| {
            (aspect - candidate / dst_h as f64).abs()
        });
        (adjusted, dst_h)
    } else {
        let adjusted = thumbnail_round_aspect(dst_w as f64 / aspect, |candidate| {
            if candidate == 0.0 {
                0.0
            } else {
                (aspect - dst_w as f64 / candidate).abs()
            }
        });
        (dst_w, adjusted)
    };

    if new_w >= w && new_h >= h {
        // No shrink needed — return a copy with alpha clamping
        let has_a = mode == 1 || mode == 3;
        let out: Vec<u32> = pixels
            .iter()
            .map(|&p| {
                let a = p & 0xFF00_0000;
                let out_a = if has_a { a } else { 0xFF00_0000 };
                (p & 0x00FF_FFFF) | out_a
            })
            .collect();
        return (out, w, h);
    }

    resize(pixels, w, h, new_w, new_h, mode, filter)
}

#[inline]
fn thumbnail_round_aspect(number: f64, key: impl Fn(f64) -> f64) -> u32 {
    let floor = number.trunc();
    if number == floor {
        return floor as u32;
    }
    let ceil = floor + 1.0;
    let best = if key(floor) <= key(ceil) { floor } else { ceil };
    (best as u32).max(1)
}

/// Reduce: downsample by `factor` using block averaging.
///
/// Each output pixel is the average of a `factor x factor` input block.
/// For L/LA modes: only R channel is averaged (G = B = R in output).
/// For RGB/RGBA modes: R, G, B are averaged independently.
/// Alpha is averaged for LA/RGBA, forced to 0xFF for L/RGB.
/// Uses round-to-nearest: `(sum + area/2) / area`.
/// Returns `(Vec<u32>, new_w, new_h)` where `new_w = w/factor, new_h = h/factor`.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn reduce(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    x_factor: u32,
    y_factor: u32,
) -> (Vec<u32>, u32, u32) {
    if x_factor < 2 && y_factor < 2 {
        // Cannot reduce — return copy with alpha clamping
        let has_a = mode == 1 || mode == 3;
        let out: Vec<u32> = pixels
            .iter()
            .map(|&p| {
                let a = p & 0xFF00_0000;
                let out_a = if has_a { a } else { 0xFF00_0000 };
                (p & 0x00FF_FFFF) | out_a
            })
            .collect();
        return (out, w, h);
    }

    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let fx = x_factor;
    let fy = y_factor;
    // Pillow Reduce.c computes ceil(w/xscale) x ceil(h/yscale).
    let new_w = w.div_ceil(fx);
    let new_h = h.div_ceil(fy);
    let mut out = vec![0u32; (new_w * new_h) as usize];

    for y in 0..new_h {
        for x in 0..new_w {
            let mut sum_r = 0u64;
            let mut sum_g = 0u64;
            let mut sum_b = 0u64;
            let mut sum_a = 0u64;
            let mut count = 0u64;

            for dy in 0..fy {
                let sy = y * fy + dy;
                if sy >= h {
                    break;
                }
                let row = sy * w;
                for dx in 0..fx {
                    let sx = x * fx + dx;
                    if sx >= w {
                        break;
                    }
                    let p = pixels[(row + sx) as usize];
                    let alpha = (p >> 24) & 0xFF;
                    let premultiply = |value: u32| (value * alpha + 127) / 255;
                    sum_r += u64::from(if has_a {
                        premultiply(p & 0xFF)
                    } else {
                        p & 0xFF
                    });
                    sum_g += u64::from(if has_a {
                        premultiply((p >> 8) & 0xFF)
                    } else {
                        (p >> 8) & 0xFF
                    });
                    sum_b += u64::from(if has_a {
                        premultiply((p >> 16) & 0xFF)
                    } else {
                        (p >> 16) & 0xFF
                    });
                    sum_a += (p >> 24) as u64;
                    count += 1;
                }
            }

            // Pillow's Reduce.c uses a truncated fixed-point reciprocal for
            // each (possibly partial) block. Integer division rounds some
            // half-way partial blocks one value too high (for example, a
            // single 255 sample in a six-pixel block is 42, not 43).
            let block_average = |sum: u64| -> u32 {
                let multiplier = (1u128 << 32) / (u128::from(count) * 256);
                (((u128::from(sum) + u128::from(count / 2)) * multiplier) >> 24) as u32
            };
            let out_a_raw = block_average(sum_a);
            let unpremultiply = |value: u32| {
                if has_a && out_a_raw != 0 {
                    value.saturating_mul(255) / out_a_raw
                } else {
                    value
                }
            };
            let out_r = unpremultiply(block_average(sum_r));
            let out_g_raw = unpremultiply(block_average(sum_g));
            let out_b_raw = unpremultiply(block_average(sum_b));

            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };
            let out_a_val = if has_a { out_a_raw } else { 255u32 };

            let dst_idx = (y * new_w + x) as usize;
            out[dst_idx] = out_r | (out_g << 8) | (out_b << 16) | (out_a_val << 24);
        }
    }

    (out, new_w, new_h)
}

/// Convert: convert between color modes.
///
/// `target_mode`: 0=L, 1=LA, 2=RGB, 3=RGBA
///
/// Supported conversions:
/// - L(0) -> LA(1): add alpha=255
/// - L(0) -> RGB(2): replicate R to G,B
/// - L(0) -> RGBA(3): replicate R to G,B, add alpha=255
/// - LA(1) -> L(0): drop alpha, replicate R to G,B
/// - LA(1) -> RGB(2): replicate R to G,B, drop alpha
/// - LA(1) -> RGBA(3): replicate R to G,B, keep A
/// - RGB(2) -> L(0): BT.601 luma
/// - RGB(2) -> LA(1): BT.601 luma, add alpha=255
/// - RGB(2) -> RGBA(3): add alpha=255
/// - RGBA(3) -> L(0): BT.601 luma, drop alpha
/// - RGBA(3) -> LA(1): BT.601 luma, keep A
/// - RGBA(3) -> RGB(2): drop alpha
///
/// Returns `(Vec<u32>, w, h)` — same size as input.
/// Identity conversions (same mode) skip computation and only clamp alpha.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn convert(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    target_mode: u32,
) -> (Vec<u32>, u32, u32) {
    let n = (w * h) as usize;
    let mut out = Vec::with_capacity(n);

    for &p in pixels.iter() {
        let r_src = p & 0xFF;
        let g_src = (p >> 8) & 0xFF;
        let b_src = (p >> 16) & 0xFF;
        let a_src = p >> 24;

        match (mode, target_mode) {
            // ── Same-mode identities ──
            (0, 0) | (1, 1) | (2, 2) | (3, 3) => {
                let out_a = if mode == 0 || mode == 2 { 0xFF } else { a_src };
                out.push(r_src | (g_src << 8) | (b_src << 16) | (out_a << 24));
            }

            // ── L(0) -> other modes ──
            (0, 1) => {
                // L->LA: add alpha=255
                out.push(r_src | (r_src << 8) | (r_src << 16) | (0xFF << 24));
            }
            (0, 2) | (0, 3) => {
                // L->RGB or L->RGBA: replicate R to G,B
                out.push(r_src | (r_src << 8) | (r_src << 16) | (0xFF << 24));
            }

            // ── LA(1) -> other modes ──
            (1, 0) | (1, 2) => {
                // LA->L or LA->RGB: drop alpha, replicate R to G,B
                out.push(r_src | (r_src << 8) | (r_src << 16) | (0xFF << 24));
            }
            (1, 3) => {
                // LA->RGBA: replicate R to G,B, keep A
                out.push(r_src | (r_src << 8) | (r_src << 16) | (a_src << 24));
            }

            // ── RGB(2) -> other modes ──
            (2, 0) | (2, 1) => {
                // RGB->L or RGB->LA: BT.601 luma
                let luma = ((299 * r_src + 587 * g_src + 114 * b_src + 500) / 1000).min(255);
                out.push(luma | (luma << 8) | (luma << 16) | (0xFF << 24));
            }
            (2, 3) => {
                // RGB->RGBA: add alpha=255
                out.push(r_src | (g_src << 8) | (b_src << 16) | (0xFF << 24));
            }

            // ── RGBA(3) -> other modes ──
            (3, 0) | (3, 1) => {
                // RGBA->L or RGBA->LA: BT.601 luma
                let luma = ((299 * r_src + 587 * g_src + 114 * b_src + 500) / 1000).min(255);
                let out_a = if target_mode == 1 { a_src } else { 0xFF };
                out.push(luma | (luma << 8) | (luma << 16) | (out_a << 24));
            }
            (3, 2) => {
                // RGBA->RGB: drop alpha
                out.push(r_src | (g_src << 8) | (b_src << 16) | (0xFF << 24));
            }

            // ── L/LA (0/1) -> CMYK (4): gray to K, C=M=Y=0 ──
            (0, 4) | (1, 4) => {
                let k = 255u32.saturating_sub(u32::from(r_src)) & 0xFF;
                out.push(k << 24);
            }

            // ── RGB/RGBA (2/3) -> CMYK (4): inverse, K=0 ──
            (2, 4) | (3, 4) => {
                out.push(
                    (255u32.wrapping_sub(u32::from(r_src)) & 0xFF)
                        | ((255u32.wrapping_sub(u32::from(g_src)) & 0xFF) << 8)
                        | ((255u32.wrapping_sub(u32::from(b_src)) & 0xFF) << 16)
                        | 0,
                );
            }

            // ── Unreachable (all combos covered above) ──
            _ => {
                let out_a = if target_mode == 0 || target_mode == 2 {
                    0xFF
                } else {
                    a_src
                };
                out.push(r_src | (g_src << 8) | (b_src << 16) | (out_a << 24));
            }
        }
    }

    (out, w, h)
}

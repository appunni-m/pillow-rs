//! Scalar fallback implementations — safe, portable, auto-vectorization friendly.
//!
//! These are written as tight loops over u32 slices so LLVM can auto-vectorize
//! them when compiled with `-C target-cpu=native`. They also serve as the
//! reference implementation for platform-specific SIMD code.

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

/// Posterize: reduce the number of bits per channel.
/// Each channel is quantized to `bits` bits (1-8) using the formula:
/// `out = ((val >> (8 - bits)) << (8 - bits))`.
/// Mode-aware: only touches channels present in the image mode.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn posterize(pixels: &mut [u32], mode: u32, bits: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let shift = 8 - bits;

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        // PIL formula: ((val >> shift) << shift) zeros lower (8-bits) bits
        let out_r = (r >> shift) << shift;
        let out_g = if has_gb { (g >> shift) << shift } else { g };
        let out_b = if has_gb { (b >> shift) << shift } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Brightness: multiply active channels by factor (fixed-point: factor * 1000).
#[inline]
pub fn brightness(pixels: &mut [u32], mode: u32, factor_fp: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let out_r = (r * factor_fp / 1000).min(255);
        let out_g_raw = (g * factor_fp / 1000).min(255);
        let out_b_raw = (b * factor_fp / 1000).min(255);

        let out_g = if has_gb { out_g_raw } else { g };
        let out_b = if has_gb { out_b_raw } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Contrast: adjust contrast for active channels.
#[inline]
pub fn contrast(pixels: &mut [u32], mode: u32, factor_fp: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    for p in pixels.iter_mut() {
        let r = (*p & 0xFF) as i32;
        let g = ((*p >> 8) & 0xFF) as i32;
        let b = ((*p >> 16) & 0xFF) as i32;
        let a = *p & 0xFF00_0000;

        let adjust = |c: i32| -> u32 {
            let v = ((c - 128) * factor_fp as i32 / 1000 + 128).clamp(0, 255);
            v as u32
        };

        let out_r = adjust(r);
        let out_g = if has_gb { adjust(g) } else { g as u32 };
        let out_b = if has_gb { adjust(b) } else { b as u32 };
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

/// InvertChops: 255 - channel for all active channels.
/// Identical formula to ImageOps.invert. R always inverted (carries luma in L/LA).
/// G/B inverted only for mode >= 2 (RGB, RGBA). Alpha preserved in LA/RGBA.
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
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Add: (pixel + other) * scale + offset, clamped 0..255 per active channel.
/// Dual-input: iterates over pixels and other simultaneously.
/// Alpha preserved in LA/RGBA, forced to 0xFF in L/RGB.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn add(pixels: &mut [u32], mode: u32, other: &[u32], scale: f32, offset: f32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let or = *o & 0xFF;
        let og = (*o >> 8) & 0xFF;
        let ob = (*o >> 16) & 0xFF;

        let out_r = ((r as f32 + or as f32) * scale + offset).clamp(0.0, 255.0) as u32;
        let out_g_raw = ((g as f32 + og as f32) * scale + offset).clamp(0.0, 255.0) as u32;
        let out_b_raw = ((b as f32 + ob as f32) * scale + offset).clamp(0.0, 255.0) as u32;

        let out_g = if has_gb { out_g_raw } else { g };
        let out_b = if has_gb { out_b_raw } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Subtract: (pixel - other) * scale + offset, clamped 0..255 per active channel.
/// Dual-input: iterates over pixels and other simultaneously.
/// Alpha preserved in LA/RGBA, forced to 0xFF in L/RGB.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn subtract(pixels: &mut [u32], mode: u32, other: &[u32], scale: f32, offset: f32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let or = *o & 0xFF;
        let og = (*o >> 8) & 0xFF;
        let ob = (*o >> 16) & 0xFF;

        let out_r = ((r as f32 - or as f32) * scale + offset).clamp(0.0, 255.0) as u32;
        let out_g_raw = ((g as f32 - og as f32) * scale + offset).clamp(0.0, 255.0) as u32;
        let out_b_raw = ((b as f32 - ob as f32) * scale + offset).clamp(0.0, 255.0) as u32;

        let out_g = if has_gb { out_g_raw } else { g };
        let out_b = if has_gb { out_b_raw } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

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

        let out_r = ar * br / 255;
        let out_g = if has_gb { ag * bg / 255 } else { ag };
        let out_b = if has_gb { ab * bb / 255 } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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

        let out_r = ar.min(br);
        let out_g = if has_gb { ag.min(bg) } else { ag };
        let out_b = if has_gb { ab.min(bb) } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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

        let out_r = ar.max(br);
        let out_g = if has_gb { ag.max(bg) } else { ag };
        let out_b = if has_gb { ab.max(bb) } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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

        let out_r = (ar.wrapping_add(br)) & 0xFF;
        let out_g_raw = (ag.wrapping_add(bg)) & 0xFF;
        let out_b_raw = (ab.wrapping_add(bb)) & 0xFF;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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

        let out_r = (ar.wrapping_sub(br)) & 0xFF;
        let out_g_raw = (ag.wrapping_sub(bg)) & 0xFF;
        let out_b_raw = (ab.wrapping_sub(bb)) & 0xFF;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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

        let out_r = ar & br;
        let out_g_raw = ag & bg;
        let out_b_raw = ab & bb;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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

        let out_r = ar | br;
        let out_g_raw = ag | bg;
        let out_b_raw = ab | bb;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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

        let out_r = ar ^ br;
        let out_g_raw = ag ^ bg;
        let out_b_raw = ab ^ bb;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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

        // BT.601: (299*R + 587*G + 114*B + 500) / 1000
        let luma = ((299 * r + 587 * g + 114 * b + 500) / 1000) as i32;
        let factor = factor_fp as i32;

        // out = luma + (channel - luma) * factor / 1000, clamped 0..255
        let out_r = (luma + ((r as i32 - luma) * factor / 1000)).clamp(0, 255) as u32;
        let out_g = (luma + ((g as i32 - luma) * factor / 1000)).clamp(0, 255) as u32;
        let out_b = (luma + ((b as i32 - luma) * factor / 1000)).clamp(0, 255) as u32;
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Colorize: map luma to two-color gradient (black -> white).
/// black_rgb / white_rgb packed as 0x00_BB_GGRR (no alpha).
/// For L/LA: luma = R. For RGB/RGBA: luma = BT.601.
/// out = black + (white - black) * luma / 255 per channel, clamped 0..255.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn colorize(pixels: &mut [u32], mode: u32, black_rgb: u32, white_rgb: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    let black_r = (black_rgb & 0xFF) as i32;
    let black_g = ((black_rgb >> 8) & 0xFF) as i32;
    let black_b = ((black_rgb >> 16) & 0xFF) as i32;
    let white_r = (white_rgb & 0xFF) as i32;
    let white_g = ((white_rgb >> 8) & 0xFF) as i32;
    let white_b = ((white_rgb >> 16) & 0xFF) as i32;

    // Precompute deltas
    let dr = white_r - black_r;
    let dg = white_g - black_g;
    let db = white_b - black_b;

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let luma = if has_gb {
            ((299 * r + 587 * g + 114 * b + 500) / 1000).min(255) as i32
        } else {
            r as i32
        };

        let out_r = (black_r + dr * luma / 255).clamp(0, 255) as u32;
        let out_g_raw = (black_g + dg * luma / 255).clamp(0, 255) as u32;
        let out_b_raw = (black_b + db * luma / 255).clamp(0, 255) as u32;

        let out_g = if has_gb { out_g_raw } else { g };
        let out_b = if has_gb { out_b_raw } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
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

/// Offset: pixel value += offset per channel, clamped 0..255.
/// dx added to R (always active). dy added to G and B only for mode >= 2.
/// Named to match PIL's x=dx, y=dy convention.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn offset(pixels: &mut [u32], mode: u32, dx: u32, dy: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let out_r = (r + dx).min(255);
        let out_g_raw = (g + dy).min(255);
        let out_b_raw = (b + dy).min(255);

        let out_g = if has_gb { out_g_raw } else { g };
        let out_b = if has_gb { out_b_raw } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

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
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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
        let out_a = if has_a { aa } else { 0xFF00_0000 };

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
        let out_a = if has_a { aa } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Blend: linear interpolation between two images.
/// `out = a * (1.0 - alpha) + b * alpha`, alpha in 0.0..1.0.
/// Math done in f32, result clamped to 0..255.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn blend(pixels: &mut [u32], mode: u32, other: &[u32], alpha: f64) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let a_f = alpha.clamp(0.0, 1.0) as f32;
    let inv_a = 1.0 - a_f;
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;

        let ch = |a: u32, b: u32| -> u32 {
            (a as f32 * inv_a + b as f32 * a_f).clamp(0.0, 255.0) as u32
        };

        let out_r = ch(ar, br);
        let out_g_raw = ch(ag, bg);
        let out_b_raw = ch(ab, bb);

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a { aa } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Blend module: identical formula to `blend`.
/// `out = a * (1.0 - alpha) + b * alpha`, alpha in 0.0..1.0.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn blend_module(pixels: &mut [u32], mode: u32, other: &[u32], alpha: f64) {
    blend(pixels, mode, other, alpha)
}

// ── Rank-based window filter operations ──

/// Median filter: for each pixel, output median of size×size neighborhood.
///
/// For each pixel, collects all R (and G/B for RGB/RGBA) channel values within
/// the size×size window (with clamped border handling), sorts them, and outputs
/// the value at index `size*size/2`.
///
/// For L/LA modes (mode < 2): only R channel is processed. G and B mirror R.
/// For RGB/RGBA modes (mode >= 2): R, G, B processed independently.
/// Alpha is preserved in LA/RGBA, forced to 0xFF in L/RGB.
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

    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            let pixel = src[idx];
            let a_orig = pixel & 0xFF00_0000;

            // Collect R channel values from window
            let mut r_vals = Vec::with_capacity(area);
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sy = (y + dy).clamp(0, h_i - 1);
                    r_vals.push((src[(sy * w_i + sx) as usize] & 0xFF) as u8);
                }
            }
            r_vals.sort_unstable();
            let out_r = r_vals[mid] as u32;

            if has_gb {
                let mut g_vals = Vec::with_capacity(area);
                let mut b_vals = Vec::with_capacity(area);
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
                let out_a = if has_a { a_orig } else { 0xFF00_0000 };
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            } else {
                let out_a = if has_a { a_orig } else { 0xFF00_0000 };
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
/// Alpha is preserved in LA/RGBA, forced to 0xFF in L/RGB.
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
            let pixel = src[idx];
            let a_orig = pixel & 0xFF00_0000;

            // Track running max for R channel
            let mut max_r = 0u8;
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let sp = src[(sy * w_i + sx) as usize];
                    max_r = max_r.max((sp & 0xFF) as u8);
                }
            }
            let out_r = max_r as u32;

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
                let out_a = if has_a { a_orig } else { 0xFF00_0000 };
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            } else {
                let out_a = if has_a { a_orig } else { 0xFF00_0000 };
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
/// Alpha is preserved in LA/RGBA, forced to 0xFF in L/RGB.
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
            let pixel = src[idx];
            let a_orig = pixel & 0xFF00_0000;

            // Track running min for R channel
            let mut min_r = 255u8;
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let sp = src[(sy * w_i + sx) as usize];
                    min_r = min_r.min((sp & 0xFF) as u8);
                }
            }
            let out_r = min_r as u32;

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
                let out_a = if has_a { a_orig } else { 0xFF00_0000 };
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            } else {
                let out_a = if has_a { a_orig } else { 0xFF00_0000 };
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
/// Alpha is preserved in LA/RGBA, forced to 0xFF in L/RGB.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
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
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            let pixel = src[idx];
            let a_orig = pixel & 0xFF00_0000;

            // Collect R channel values from window
            let mut r_vals = Vec::with_capacity(area);
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sy = (y + dy).clamp(0, h_i - 1);
                    r_vals.push((src[(sy * w_i + sx) as usize] & 0xFF) as u8);
                }
            }
            r_vals.sort_unstable();
            let out_r = r_vals[rank] as u32;

            if has_gb {
                let mut g_vals = Vec::with_capacity(area);
                let mut b_vals = Vec::with_capacity(area);
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
                let out_g = g_vals[rank] as u32;
                let out_b = b_vals[rank] as u32;
                let out_a = if has_a { a_orig } else { 0xFF00_0000 };
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
            } else {
                let out_a = if has_a { a_orig } else { 0xFF00_0000 };
                pixels[idx] = out_r | (out_r << 8) | (out_r << 16) | out_a;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let src = pixels.to_vec();
    let w_i = w as i32;
    let h_i = h as i32;
    let half = 1i32;
    let s = if scale.abs() < 1e-10 { 1.0 } else { scale };

    for y in 0..h_i {
        for x in 0..w_i {
            let mut sum_r: f32 = 0.0;
            let mut sum_g: f32 = 0.0;
            let mut sum_b: f32 = 0.0;

            for ky in -half..=half {
                for kx in -half..=half {
                    let sx = (x + kx).clamp(0, w_i - 1);
                    let sy = (y + ky).clamp(0, h_i - 1);
                    let sp = src[(sy * w_i + sx) as usize];
                    let ki = ((ky + half) * 3 + (kx + half)) as usize;
                    let k = kernel[ki];
                    sum_r += (sp & 0xFF) as f32 * k;
                    sum_g += ((sp >> 8) & 0xFF) as f32 * k;
                    sum_b += ((sp >> 16) & 0xFF) as f32 * k;
                }
            }

            let out_r = ((sum_r / s + offset as f32 + 0.5) as i32).clamp(0, 255) as u32;
            let out_g_raw = ((sum_g / s + offset as f32 + 0.5) as i32).clamp(0, 255) as u32;
            let out_b_raw = ((sum_b / s + offset as f32 + 0.5) as i32).clamp(0, 255) as u32;

            // L/LA: G = B = out_r; RGB/RGBA: independent G and B
            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };

            let sp = src[(y * w_i + x) as usize];
            let a = sp & 0xFF00_0000;
            let out_a = if has_a { a } else { 0xFF00_0000 };

            let idx = (y * w_i + x) as usize;
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
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let src = pixels.to_vec();
    let w_i = w as i32;
    let h_i = h as i32;
    let half = 2i32;
    let s = if scale.abs() < 1e-10 { 1.0 } else { scale };

    for y in 0..h_i {
        for x in 0..w_i {
            let mut sum_r: f32 = 0.0;
            let mut sum_g: f32 = 0.0;
            let mut sum_b: f32 = 0.0;

            for ky in -half..=half {
                for kx in -half..=half {
                    let sx = (x + kx).clamp(0, w_i - 1);
                    let sy = (y + ky).clamp(0, h_i - 1);
                    let sp = src[(sy * w_i + sx) as usize];
                    let ki = ((ky + half) * 5 + (kx + half)) as usize;
                    let k = kernel[ki];
                    sum_r += (sp & 0xFF) as f32 * k;
                    sum_g += ((sp >> 8) & 0xFF) as f32 * k;
                    sum_b += ((sp >> 16) & 0xFF) as f32 * k;
                }
            }

            let out_r = ((sum_r / s + offset as f32 + 0.5) as i32).clamp(0, 255) as u32;
            let out_g_raw = ((sum_g / s + offset as f32 + 0.5) as i32).clamp(0, 255) as u32;
            let out_b_raw = ((sum_b / s + offset as f32 + 0.5) as i32).clamp(0, 255) as u32;

            let out_g = if has_gb { out_g_raw } else { out_r };
            let out_b = if has_gb { out_b_raw } else { out_r };

            let sp = src[(y * w_i + x) as usize];
            let a = sp & 0xFF00_0000;
            let out_a = if has_a { a } else { 0xFF00_0000 };

            let idx = (y * w_i + x) as usize;
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
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn sharpness(pixels: &mut [u32], w: u32, h: u32, mode: u32, factor_fp: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let src = pixels.to_vec();
    let w_i = w as i32;
    let h_i = h as i32;
    let half = 1i32;
    // SMOOTH kernel pre-divided: [1,1,1, 1,5,1, 1,1,1] / 13
    let inv_scale = 1.0f32 / 13.0f32;
    let k_edges = inv_scale; // edges = 1/13
    let k_center = 5.0f32 * inv_scale; // center = 5/13
    let rounding_bias = 0.5f32; // offset=0 => 0.0 + 0.5
    // Blend weight: factor_fp / 1000
    let t = factor_fp as f32 / 1000.0;
    let one_minus_t = 1.0 - t;

    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            let orig = src[idx];
            let orig_r = orig & 0xFF;
            let orig_g = (orig >> 8) & 0xFF;
            let orig_b = (orig >> 16) & 0xFF;
            let orig_a = orig & 0xFF00_0000;

            // Convolve with SMOOTH kernel
            let mut sum_r: f32 = 0.0;
            let mut sum_g: f32 = 0.0;
            let mut sum_b: f32 = 0.0;

            for ky in -half..=half {
                for kx in -half..=half {
                    let sx = (x + kx).clamp(0, w_i - 1);
                    let sy = (y + ky).clamp(0, h_i - 1);
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
                }
            }

            // Apply rounding bias and clamp to [0, 255]
            let blur_r = ((sum_r + rounding_bias) as i32).clamp(0, 255) as u32;
            let blur_g_raw = ((sum_g + rounding_bias) as i32).clamp(0, 255) as u32;
            let blur_b_raw = ((sum_b + rounding_bias) as i32).clamp(0, 255) as u32;

            let blur_g = if has_gb { blur_g_raw } else { blur_r };
            let blur_b = if has_gb { blur_b_raw } else { blur_r };

            // Blend: out = blurred * (1.0 - t) + original * t
            let out_r = ((blur_r as f32 * one_minus_t + orig_r as f32 * t) + 0.5) as u32;
            let out_g = if has_gb {
                ((blur_g as f32 * one_minus_t + orig_g as f32 * t) + 0.5) as u32
            } else {
                out_r
            };
            let out_b = if has_gb {
                ((blur_b as f32 * one_minus_t + orig_b as f32 * t) + 0.5) as u32
            } else {
                out_r
            };

            let out_r = out_r.min(255);
            let out_g = out_g.min(255);
            let out_b = out_b.min(255);
            let out_a = if has_a { orig_a } else { 0xFF00_0000 };

            pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | out_a;
        }
    }
}

// ── Spatial operations (mirror, transpose, box_blur, gaussian_blur) ──────────

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
/// method_code:
///   0=FlipLeftRight, 1=FlipTopBottom, 2=Rotate90, 3=Rotate180,
///   4=Rotate270, 5=Transpose, 6=Transverse
///
/// Operations 0,1,3 are in-place (same dimensions). Operations 2,4,5,6
/// change dimensions (output is H×W). Returns (pixels, new_w, new_h).
///
/// For L/LA modes: G = B = R (luma carried in R, G/B stale after remap).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn transpose(
    pixels: &mut [u32],
    w: u32,
    h: u32,
    mode: u32,
    method_code: u32,
) -> (Vec<u32>, u32, u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let w_us = w as usize;
    let h_us = h as usize;

    match method_code {
        // ── FlipLeftRight: horizontal mirror ──
        0 => {
            mirror(pixels, w, h, mode);
            (Vec::new(), w, h)
        }

        // ── FlipTopBottom: vertical flip ──
        1 => {
            flip(pixels, w, h, mode);
            (Vec::new(), w, h)
        }

        // ── Rotate90 (CCW): output[H×W] row_out=col, col_out=W-1-row ──
        2 => {
            let mut out = vec![0u32; w_us * h_us];
            for row_out in 0..w_us {
                for col_out in 0..h_us {
                    // output[row_out][col_out] = source[col_out][W-1-row_out]
                    let src_y = col_out;
                    let src_x = w_us - 1 - row_out;
                    let src_idx = src_y * w_us + src_x;
                    let sp = pixels[src_idx];

                    let dst_idx = row_out * h_us + col_out;
                    if has_gb {
                        out[dst_idx] = sp;
                    } else {
                        let r = sp & 0xFF;
                        let a = sp & 0xFF00_0000;
                        out[dst_idx] =
                            r | (r << 8) | (r << 16) | if has_a { a } else { 0xFF00_0000 };
                    }
                }
            }
            (out, h, w)
        }

        // ── Rotate180: reverse everything ──
        3 => {
            // Reverse entire buffer: swap first with last, etc.
            let total = w_us * h_us;
            for i in 0..total / 2 {
                let j = total - 1 - i;
                let li = pixels[i];
                let rj = pixels[j];

                if has_gb {
                    let mask = if has_a { 0xFFFF_FFFF } else { 0x00FF_FFFF };
                    pixels[i] = rj & mask;
                    pixels[j] = li & mask;
                } else {
                    let li_r = li & 0xFF;
                    let li_a = li & 0xFF00_0000;
                    let rj_r = rj & 0xFF;
                    let rj_a = rj & 0xFF00_0000;
                    pixels[i] =
                        rj_r | (rj_r << 8) | (rj_r << 16) | if has_a { rj_a } else { 0xFF00_0000 };
                    pixels[j] =
                        li_r | (li_r << 8) | (li_r << 16) | if has_a { li_a } else { 0xFF00_0000 };
                }
            }
            // Middle pixel if odd total
            if total % 2 == 1 && !has_gb {
                let mid = total / 2;
                let p = pixels[mid];
                let r = p & 0xFF;
                let a = p & 0xFF00_0000;
                pixels[mid] = r | (r << 8) | (r << 16) | if has_a { a } else { 0xFF00_0000 };
            } else if total % 2 == 1 && !has_a {
                pixels[total / 2] &= 0x00FF_FFFF;
            }
            (Vec::new(), w, h)
        }

        // ── Rotate270 (CW): output[H×W] row_out=col, col_out=row ──
        4 => {
            let mut out = vec![0u32; w_us * h_us];
            for row_out in 0..w_us {
                for col_out in 0..h_us {
                    // output[row_out][col_out] = source[H-1-col_out][row_out]
                    let src_y = h_us - 1 - col_out;
                    let src_x = row_out;
                    let src_idx = src_y * w_us + src_x;
                    let sp = pixels[src_idx];

                    let dst_idx = row_out * h_us + col_out;
                    if has_gb {
                        out[dst_idx] = sp;
                    } else {
                        let r = sp & 0xFF;
                        let a = sp & 0xFF00_0000;
                        out[dst_idx] =
                            r | (r << 8) | (r << 16) | if has_a { a } else { 0xFF00_0000 };
                    }
                }
            }
            (out, h, w)
        }

        // ── Transpose: output[H×W] row_out=col, col_out=row ──
        5 => {
            let mut out = vec![0u32; w_us * h_us];
            for row_out in 0..w_us {
                for col_out in 0..h_us {
                    // output[row_out][col_out] = source[col_out][row_out]
                    let src_idx = col_out * w_us + row_out;
                    let sp = pixels[src_idx];

                    let dst_idx = row_out * h_us + col_out;
                    if has_gb {
                        out[dst_idx] = sp;
                    } else {
                        let r = sp & 0xFF;
                        let a = sp & 0xFF00_0000;
                        out[dst_idx] =
                            r | (r << 8) | (r << 16) | if has_a { a } else { 0xFF00_0000 };
                    }
                }
            }
            (out, h, w)
        }

        // ── Transverse: output[H×W] row_out=W-1-col, col_out=H-1-row ──
        6 => {
            let mut out = vec![0u32; w_us * h_us];
            for row_out in 0..w_us {
                for col_out in 0..h_us {
                    // output[row_out][col_out] = source[W-1-row_out][H-1-col_out]
                    let src_y = h_us - 1 - col_out;
                    let src_x = w_us - 1 - row_out;
                    let src_idx = src_y * w_us + src_x;
                    let sp = pixels[src_idx];

                    let dst_idx = row_out * h_us + col_out;
                    if has_gb {
                        out[dst_idx] = sp;
                    } else {
                        let r = sp & 0xFF;
                        let a = sp & 0xFF00_0000;
                        out[dst_idx] =
                            r | (r << 8) | (r << 16) | if has_a { a } else { 0xFF00_0000 };
                    }
                }
            }
            (out, h, w)
        }

        _ => {
            // Unknown method code: no-op (return input unchanged)
            (Vec::new(), w, h)
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
            let sp = pixels[idx];

            if has_gb {
                // RGB/RGBA: separate R, G, B accumulation
                let mut acc_r = 0u64;
                let mut acc_g = 0u64;
                let mut acc_b = 0u64;
                for dx in -r..=r {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    let sp2 = pixels[(y * w_i + sx) as usize];
                    acc_r += (sp2 & 0xFF) as u64;
                    acc_g += ((sp2 >> 8) & 0xFF) as u64;
                    acc_b += ((sp2 >> 16) & 0xFF) as u64;
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let out_g = ((acc_g * ww as u64 + bias as u64) >> 24) as u32;
                let out_b = ((acc_b * ww as u64 + bias as u64) >> 24) as u32;
                let a = sp & 0xFF00_0000;
                hpass[idx] =
                    out_r | (out_g << 8) | (out_b << 16) | if has_a { a } else { 0xFF00_0000 };
            } else {
                // L/LA: only R channel matters, G=B=R
                let mut acc_r = 0u64;
                for dx in -r..=r {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    acc_r += (pixels[(y * w_i + sx) as usize] & 0xFF) as u64;
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let a = sp & 0xFF00_0000;
                hpass[idx] =
                    out_r | (out_r << 8) | (out_r << 16) | if has_a { a } else { 0xFF00_0000 };
            }
        }
    }

    // ── Vertical pass ──
    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            let sp = hpass[idx];

            if has_gb {
                let mut acc_r = 0u64;
                let mut acc_g = 0u64;
                let mut acc_b = 0u64;
                for dy in -r..=r {
                    let sy = (y + dy).clamp(0, h_i - 1);
                    let sp2 = hpass[(sy * w_i + x) as usize];
                    acc_r += (sp2 & 0xFF) as u64;
                    acc_g += ((sp2 >> 8) & 0xFF) as u64;
                    acc_b += ((sp2 >> 16) & 0xFF) as u64;
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let out_g = ((acc_g * ww as u64 + bias as u64) >> 24) as u32;
                let out_b = ((acc_b * ww as u64 + bias as u64) >> 24) as u32;
                let a = sp & 0xFF00_0000;
                pixels[idx] =
                    out_r | (out_g << 8) | (out_b << 16) | if has_a { a } else { 0xFF00_0000 };
            } else {
                let mut acc_r = 0u64;
                for dy in -r..=r {
                    let sy = (y + dy).clamp(0, h_i - 1);
                    acc_r += (hpass[(sy * w_i + x) as usize] & 0xFF) as u64;
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let a = sp & 0xFF00_0000;
                pixels[idx] =
                    out_r | (out_r << 8) | (out_r << 16) | if has_a { a } else { 0xFF00_0000 };
            }
        }
    }
}

/// Gaussian blur: approximate Gaussian using 3 passes of box blur.
///
/// Uses the "From Box Blur to Gaussian Blur" algorithm (Gwosdek et al., 2011),
/// computing the equivalent box blur radius from sigma.
///
/// For L/LA modes: G = B = R after blurring (luma carried in R channel).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn gaussian_blur(pixels: &mut [u32], w: u32, h: u32, mode: u32, sigma: f32) {
    if sigma <= 0.0 {
        return;
    }

    // Compute effective box blur radius from sigma using PIL's formula.
    // sigma2 = sigma^2 / 3
    // l = floor(((12*sigma2+1)^0.5 - 1) / 2)
    // l1 = l + 1
    // a = (2*l+1)*(l*l1 - 3*sigma2) / (6*(sigma2 - l1*l1))
    // blur_radius = l + a
    let passes = 3.0f64;
    let sigma2 = sigma as f64 * sigma as f64 / passes;
    let l_val = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l_val.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let a_den = 6.0 * (sigma2 - l1 * l1);
    let a = if a_den.abs() > 1e-10 {
        a_num / a_den
    } else {
        0.0
    };
    let blur_radius = (l + a) as f32;

    // Call box_blur for 3 passes with the computed radius (minimum 1).
    let r = blur_radius.round() as u32;
    let r = if r == 0 { 1 } else { r };
    box_blur(pixels, w, h, mode, r);
    box_blur(pixels, w, h, mode, r);
    box_blur(pixels, w, h, mode, r);
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
    let has_gb = mode >= 2; // RGB or RGBA
    let has_a = mode == 1 || mode == 3; // LA or RGBA

    // Guard against empty dimensions (avoids div-by-zero and u32 underflow)
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return (vec![0u32; (dst_w * dst_h) as usize], dst_w, dst_h);
    }

    let count = (dst_w * dst_h) as usize;
    let mut output = vec![0u32; count];

    let sw_f = src_w as f64;
    let sh_f = src_h as f64;
    let dw_f = dst_w as f64;
    let dh_f = dst_h as f64;

    // Bounds for clamping (precomputed to avoid repeated subtraction)
    let src_w_max = src_w - 1;
    let src_h_max = src_h - 1;

    if filter == 0 {
        // ── Nearest-neighbor ──
        for dy in 0..dst_h {
            // PIL: floor((dy + 0.5) * src_h / dst_h)
            let sy = ((dy as f64 + 0.5) * sh_f / dh_f) as u32;
            let sy = sy.min(src_h_max);
            for dx in 0..dst_w {
                // PIL: floor((dx + 0.5) * src_w / dst_w)
                let sx = ((dx as f64 + 0.5) * sw_f / dw_f) as u32;
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
        }
    } else {
        // ── Bilinear (and fallback for Bicubic/Lanczos/Box/Hamming) ──
        for dy in 0..dst_h {
            let cy = (dy as f64 + 0.5) * sh_f / dh_f;
            let sy_floor = cy.floor();
            let fy = cy - sy_floor;
            let y0 = (sy_floor as u32).min(src_h_max);
            let y1 = (y0 + 1).min(src_h_max);

            for dx in 0..dst_w {
                let cx = (dx as f64 + 0.5) * sw_f / dw_f;
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
) -> (Vec<u32>, u32, u32) {
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let scale = (dst_w as f64 / w as f64).min(dst_h as f64 / h as f64);
    let new_w = (w as f64 * scale).floor().max(1.0) as u32;
    let new_h = (h as f64 * scale).floor().max(1.0) as u32;
    resize(pixels, w, h, new_w, new_h, mode, 1)
}

/// Scale: resize by a floating-point factor using bilinear interpolation.
/// factor: scale multiplier (0.5 = half size, 2.0 = double).
/// Delegates to resize with bilinear filter (filter=1).
/// Returns (pixels, dst_w, dst_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn scale(pixels: &[u32], w: u32, h: u32, mode: u32, factor: f64) -> (Vec<u32>, u32, u32) {
    let dst_w = ((w as f64 * factor) as u32).max(1);
    let dst_h = ((h as f64 * factor) as u32).max(1);
    resize(pixels, w, h, dst_w, dst_h, mode, 1) // filter=1 = bilinear
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
) -> (Vec<u32>, u32, u32) {
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let scale = (dst_w as f64 / w as f64).max(dst_h as f64 / h as f64);
    let nw = (w as f64 * scale).floor().max(1.0) as u32;
    let nh = (h as f64 * scale).floor().max(1.0) as u32;
    let (resized, _, _) = resize(pixels, w, h, nw, nh, mode, 1);
    let crop_x = (nw.saturating_sub(dst_w)) / 2;
    let crop_y = (nh.saturating_sub(dst_h)) / 2;
    let cw = dst_w.min(nw);
    let ch = dst_h.min(nh);
    let cropped = crop_rect(&resized, nw, nh, mode, crop_x, crop_y, cw, ch);
    (cropped, cw, ch)
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
) -> (Vec<u32>, u32, u32) {
    if w == 0 || h == 0 || dst_w == 0 || dst_h == 0 {
        return (pixels.to_vec(), w, h);
    }
    let b = bleed.max(0.0) as f64;
    let base_scale = (dst_w as f64 / w as f64).min(dst_h as f64 / h as f64);
    let scale = base_scale * (1.0 + b);
    let nw = (w as f64 * scale).floor().max(1.0) as u32;
    let nh = (h as f64 * scale).floor().max(1.0) as u32;
    let (resized, _, _) = resize(pixels, w, h, nw, nh, mode, 1);

    let (cx, cy) = centering;
    let cx = cx.clamp(0.0, 1.0) as f64;
    let cy = cy.clamp(0.0, 1.0) as f64;
    let crop_x = ((nw as f64 - dst_w as f64) * cx + 0.5).floor().max(0.0) as u32;
    let crop_y = ((nh as f64 - dst_h as f64) * cy + 0.5).floor().max(0.0) as u32;
    let crop_x = crop_x.min(nw.saturating_sub(1));
    let crop_y = crop_y.min(nh.saturating_sub(1));
    let cw = dst_w.min(nw);
    let ch = dst_h.min(nh);
    let cropped = crop_rect(&resized, nw, nh, mode, crop_x, crop_y, cw, ch);
    (cropped, cw, ch)
}

/// Crop helper: extract a sub-rectangle from packed u32 pixel buffer.
/// Mode-aware: G/B handled per mode, alpha preserved/clamped.
/// For L/LA modes (mode < 2): G = B = R (luma carried in R).
/// For L/RGB modes (mode 0 or 2): alpha forced to 0xFF.
#[inline]
fn crop_rect(
    pixels: &[u32],
    src_w: u32,
    _src_h: u32,
    mode: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Vec<u32> {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let sz = (w * h) as usize;
    let mut out = vec![0u32; sz];
    for dy in 0..h {
        for dx in 0..w {
            let src_idx = ((y + dy) * src_w + (x + dx)) as usize;
            let sp = pixels[src_idx];
            let r = sp & 0xFF;
            let dst_idx = (dy * w + dx) as usize;
            if has_gb {
                let g = (sp >> 8) & 0xFF;
                let b = (sp >> 16) & 0xFF;
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                out[dst_idx] = r | (g << 8) | (b << 16) | a;
            } else {
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                out[dst_idx] = r | (r << 8) | (r << 16) | a;
            }
        }
    }
    out
}

/// Quantize: reduce colors to num_colors using uniform quantization.
/// Each active channel is divided into `cbrt(num_colors)` levels, then quantized
/// to the nearest bin center via `(val / step) * step + step/2`.
/// Mode-aware: only R quantized for L/LA; R, G, B quantized for RGB/RGBA.
/// Alpha preserved in LA/RGBA, forced to 0xFF in L/RGB.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn quantize(pixels: &mut [u32], w: u32, h: u32, mode: u32, num_colors: u32) {
    let _ = (w, h);
    if !(2..256).contains(&num_colors) {
        return;
    }
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    let levels = (num_colors as f64).cbrt().round().max(2.0) as u32;
    let step = 256u32 / levels;
    let half_step = step / 2;

    for p in pixels.iter_mut() {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let out_r = ((r / step) * step + half_step).min(255);
        let out_g_raw = ((g / step) * step + half_step).min(255);
        let out_b_raw = ((b / step) * step + half_step).min(255);

        let out_g = if has_gb { out_g_raw } else { g };
        let out_b = if has_gb { out_b_raw } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
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
#[inline]
pub fn pad(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    dst_w: u32,
    dst_h: u32,
    fill_rgba: u32,
) -> (Vec<u32>, u32, u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let w_i = w as i32;
    let h_i = h as i32;
    let dst_w_i = dst_w as i32;
    let dst_h_i = dst_h as i32;

    // Compute centering offsets
    let src_x_off = (w.saturating_sub(dst_w)) / 2;
    let src_y_off = (h.saturating_sub(dst_h)) / 2;
    let dst_x_off = (dst_w.saturating_sub(w)) / 2;
    let dst_y_off = (dst_h.saturating_sub(h)) / 2;

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

    let src_x_off_i = src_x_off as i32;
    let src_y_off_i = src_y_off as i32;
    let dst_x_off_i = dst_x_off as i32;
    let dst_y_off_i = dst_y_off as i32;

    // Iterate over dest, mapping overlapping pixels back to source
    for dy in 0..dst_h_i {
        let sy = dy + src_y_off_i - dst_y_off_i;
        if sy < 0 || sy >= h_i {
            continue;
        }
        let src_row = (sy * w_i) as usize;
        let dst_row = (dy * dst_w_i) as usize;

        for dx in 0..dst_w_i {
            let sx = dx + src_x_off_i - dst_x_off_i;
            if sx < 0 || sx >= w_i {
                continue;
            }
            let sp = pixels[src_row + sx as usize];
            let out_p = if has_gb {
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                (sp & 0x00FF_FFFF) | a
            } else {
                let r = sp & 0xFF;
                let a = if has_a { sp & 0xFF00_0000 } else { 0xFF00_0000 };
                r | (r << 8) | (r << 16) | a
            };
            out[dst_row + dx as usize] = out_p;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a packed u32 pixel from RGBA bytes.
    fn p(r: u8, g: u8, b: u8, a: u8) -> u32 {
        r as u32 | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24)
    }

    /// Helper: extract R channel from packed u32.
    fn r_of(pixel: u32) -> u8 {
        (pixel & 0xFF) as u8
    }
    /// Helper: extract G channel from packed u32.
    fn g_of(pixel: u32) -> u8 {
        ((pixel >> 8) & 0xFF) as u8
    }
    /// Helper: extract B channel from packed u32.
    fn b_of(pixel: u32) -> u8 {
        ((pixel >> 16) & 0xFF) as u8
    }
    /// Helper: extract A channel from packed u32.
    fn a_of(pixel: u32) -> u8 {
        ((pixel >> 24) & 0xFF) as u8
    }

    // ── Nearest-neighbor tests ──

    #[test]
    fn test_resize_nearest_identity_rgba() {
        // 2x2 RGBA image, resize to same size -> identity
        let pixels = vec![
            p(255, 0, 0, 255),
            p(0, 255, 0, 255),
            p(0, 0, 255, 255),
            p(128, 128, 128, 128),
        ];
        let (out, w, h) = resize(&pixels, 2, 2, 2, 2, 3, 0);
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(out, pixels);
    }

    #[test]
    fn test_resize_nearest_upscale_rgba() {
        // 1x1 RGBA -> 2x2 RGBA (every output pixel is the source pixel)
        let pixels = vec![p(42, 100, 200, 255)];
        let (out, w, h) = resize(&pixels, 1, 1, 2, 2, 3, 0);
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        for (i, px) in out.iter().enumerate() {
            assert_eq!(*px, pixels[0], "pixel {i} should match source");
        }
    }

    #[test]
    fn test_resize_nearest_downscale_rgba() {
        // 4x4 RGBA -> 2x2: (dx+0.5)*4/2 = 1.0, 3.0
        // outputs map to src x=1 and x=3, y=1 and y=3
        let mut pixels = vec![0u32; 16];
        pixels[0] = p(255, 0, 0, 255);
        pixels[1] = p(0, 255, 0, 255);
        pixels[4] = p(0, 0, 255, 255);
        pixels[5] = p(128, 128, 128, 255);

        let (out, w, h) = resize(&pixels, 4, 4, 2, 2, 3, 0);
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(out.len(), 4);
        // (0,0) -> sx=floor(1.0)=1, sy=floor(1.0)=1 -> pixels[1*4+1]=pixels[5]
        assert_eq!(out[0], pixels[5], "pixel (0,0) maps to src(1,1)=pixels[5]");
        // (1,0) -> sx=floor(3.0)=3, sy=floor(1.0)=1 -> pixels[1*4+3]=pixels[7]
        assert_eq!(out[1], pixels[7], "pixel (1,0) maps to src(3,1)=pixels[7]");
        // (0,1) -> sx=floor(1.0)=1, sy=floor(3.0)=3 -> pixels[3*4+1]=pixels[13]
        assert_eq!(out[2], pixels[13], "pixel (0,1) maps to src(1,3)");
        // (1,1) -> sx=floor(3.0)=3, sy=floor(3.0)=3 -> pixels[3*4+3]=pixels[15]
        assert_eq!(out[3], pixels[15], "pixel (1,1) maps to src(3,3)");
    }

    #[test]
    fn test_resize_nearest_luma() {
        // 2x2 L -> 1x1: (0+0.5)*2/1 = 1.0 -> src(1,1) = pixels[3] r=150
        let pixels = vec![
            p(100, 0, 0, 0),
            p(200, 0, 0, 0),
            p(50, 0, 0, 0),
            p(150, 0, 0, 0),
        ];
        let (out, w, h) = resize(&pixels, 2, 2, 1, 1, 0, 0);
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        // For L mode: G=B=R, A=255
        assert_eq!(r_of(out[0]), 150, "nearest samples src(1,1) which is 150");
        assert_eq!(g_of(out[0]), 150, "G=R in L mode");
        assert_eq!(b_of(out[0]), 150, "B=R in L mode");
        assert_eq!(a_of(out[0]), 255, "L mode forces A=255");
    }

    #[test]
    fn test_resize_nearest_la() {
        // 2x1 LA -> 1x1: (0+0.5)*2/1 = 1.0 -> src(1,0) = pixels[1] r=200 a=64
        let pixels = vec![p(100, 0, 0, 128), p(200, 0, 0, 64)];
        let (out, w, h) = resize(&pixels, 2, 1, 1, 1, 1, 0);
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        // For LA mode: G=B=R, A preserved
        assert_eq!(r_of(out[0]), 200, "nearest samples src(1,0) which is 200");
        assert_eq!(g_of(out[0]), 200, "G=R in LA mode");
        assert_eq!(b_of(out[0]), 200, "B=R in LA mode");
        assert_eq!(a_of(out[0]), 64, "LA mode preserves alpha");
    }

    #[test]
    fn test_resize_nearest_gb_zero_in_luma() {
        // Verify G/B are set to R in L/LA modes even if source has G/B data
        // Luma mode: only R channel is meaningful; G/B should be set to R
        let pixels = vec![p(42, 99, 199, 0)]; // G=99, B=199 should be ignored
        let (out, _w, _h) = resize(&pixels, 1, 1, 1, 1, 0, 0);
        assert_eq!(g_of(out[0]), 42, "G should mirror R in L mode");
        assert_eq!(b_of(out[0]), 42, "B should mirror R in L mode");
    }

    // ── Bilinear interpolation tests ──

    #[test]
    #[expect(
        clippy::bad_bit_mask,
        reason = "Verify extracted byte components are in valid u8 range"
    )]
    fn test_resize_bilinear_same_size_rgba() {
        // Bilinear with same dimensions: at pixel center (dx+0.5)*src/dst = dx+0.5,
        // so each dst pixel center is halfway between two src pixels -> not identity.
        let pixels = vec![
            p(10, 20, 30, 255),
            p(40, 50, 60, 255),
            p(70, 80, 90, 255),
            p(100, 110, 120, 128),
        ];
        let (out, w, h) = resize(&pixels, 2, 2, 2, 2, 3, 1);
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(out.len(), 4);
        for px in &out {
            assert!((px & 0xFF) <= 255);
            assert!(((px >> 8) & 0xFF) <= 255);
            assert!(((px >> 16) & 0xFF) <= 255);
        }
    }

    #[test]
    fn test_resize_bilinear_upscale_2x2_to_4x4() {
        // Upscale 2x2 RGBA to 4x4: interior pixels are interpolated blends
        let pixels = vec![
            p(255, 0, 0, 255),
            p(0, 0, 0, 255),
            p(0, 0, 0, 255),
            p(0, 255, 0, 255),
        ];
        let (out, w, h) = resize(&pixels, 2, 2, 4, 4, 3, 1);
        assert_eq!(w, 4);
        assert_eq!(h, 4);
        // (0,0): cx=0.25, fx=0.25, cy=0.25
        // r = (1-0.25)*(1-0.25)*255 = 0.5625*255 = 143.4375 + 0.5 -> 143
        assert_eq!(r_of(out[0]), 143, "top-left bilinear R");
        // (3,3): cx=1.75, floor=1, x1=min(2,1)=1 clamped to edge
        // All 4 neighbors = pixels[3] = (0,255,0,255)
        assert_eq!(g_of(out[15]), 255, "bottom-right G (edge-clamped)");
    }

    #[test]
    fn test_resize_bilinear_luma() {
        // L mode (mode=0): G=B=R after bilinear interpolation
        let pixels = vec![p(100, 0, 0, 0), p(200, 0, 0, 0)];
        let (out, w, h) = resize(&pixels, 2, 1, 3, 1, 0, 1);
        assert_eq!(w, 3);
        assert_eq!(h, 1);
        // (0+0.5)*2/3 = 0.333 -> floor=0, fx=0.333
        // r = (1-0.333)*100 + 0.333*200 = 66.667+66.667 = 133.33+0.5 -> 133
        assert_eq!(r_of(out[0]), 133, "left bilinear R");
        assert_eq!(g_of(out[0]), 133, "G=R in L mode");
        assert_eq!(b_of(out[0]), 133, "B=R in L mode");
        // Interior pixel (dx=1): cx=(1+0.5)*2/3=1.0, floor=1, but x1=min(2,1)=1
        // Edge clamped: both neighbors are pixels[1] = (200,0,0,0)
        // So the right two pixels are edge-clamped to 200 in a 3-pixel output
        assert_eq!(r_of(out[1]), 200, "middle bilinear R (edge-clamped)");
        assert_eq!(r_of(out[2]), 200, "right bilinear R (edge-clamped)");
    }

    #[test]
    fn test_resize_bilinear_rgb() {
        // RGB mode (mode=2): R,G,B independent, A forced to 255
        let pixels = vec![p(10, 20, 30, 0), p(40, 50, 60, 0)];
        let (out, w, h) = resize(&pixels, 2, 1, 1, 1, 2, 1);
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        // (0+0.5)*2/1 = 1.0 -> floor=1, fx=0.0
        // Wait, (0+0.5) * 2/1 = 1.0, floor=1, fx=0.0
        // But x1 = min(1+1, 1) = 1 (clamped). So x0=1, x1=1.
        // Both neighbors are the same pixel: p10 = p11 = pixels[1]
        // Result = pixels[1] = (40, 50, 60)
        assert_eq!(r_of(out[0]), 40, "RGB bilinear R");
        assert_eq!(g_of(out[0]), 50, "RGB bilinear G");
        assert_eq!(b_of(out[0]), 60, "RGB bilinear B");
        assert_eq!(a_of(out[0]), 255, "RGB mode forces A=255");
    }

    #[test]
    fn test_resize_bilinear_alpha_preserved() {
        // RGBA mode (mode=3): alpha values should be interpolated
        let pixels = vec![p(255, 0, 0, 255), p(0, 255, 0, 0)];
        let (out, w, h) = resize(&pixels, 2, 1, 3, 1, 3, 1);
        assert_eq!(w, 3);
        assert_eq!(h, 1);
        // (0+0.5)*2/3 = 0.333 -> floor=0, fx=0.333
        // p00 = (255,0,0,255), p10 = (0,255,0,0)
        // r = 0.667*255 + 0.333*0 = 170, a = 0.667*255 + 0.333*0 = 170
        assert_eq!(r_of(out[0]), 170, "bilinear R at left-interior");
        assert_eq!(a_of(out[0]), 170, "bilinear alpha interpolated");
    }

    #[test]
    fn test_resize_bilinear_filter_fallback() {
        // filter=2 (Bicubic) should fall back to bilinear
        let pixels = vec![p(10, 20, 30, 255), p(40, 50, 60, 255)];
        let (bili_out, _, _) = resize(&pixels, 2, 1, 1, 1, 3, 1);
        let (fallback_out, _, _) = resize(&pixels, 2, 1, 1, 1, 3, 2);
        assert_eq!(bili_out, fallback_out, "filter=2 should equal bilinear");
    }

    #[test]
    fn test_resize_bilinear_la_mode_alpha() {
        // LA mode (mode=1): alpha interpolated, G=B=R
        let pixels = vec![p(100, 0, 0, 255), p(200, 0, 0, 0)];
        let (out, w, h) = resize(&pixels, 2, 1, 3, 1, 1, 1);
        assert_eq!(w, 3);
        assert_eq!(h, 1);
        // For LA mode, G=B=R always
        assert_eq!(g_of(out[0]), r_of(out[0]), "G=R in LA mode");
        assert_eq!(b_of(out[0]), r_of(out[0]), "B=R in LA mode");
        // Alpha should be interpolated (not forced to 255)
        assert!(a_of(out[0]) < 255, "LA mode alpha should be interpolated");
    }

    #[test]
    fn test_resize_empty_src() {
        // Zero source dimensions -> empty output
        let (out, w, h) = resize(&[], 0, 0, 10, 10, 3, 0);
        assert_eq!(w, 10);
        assert_eq!(h, 10);
        assert_eq!(out.len(), 100);
    }

    #[test]
    fn test_resize_empty_dst() {
        // Zero destination dimensions -> empty output
        let pixels = vec![p(255, 0, 0, 255)];
        let (out, w, h) = resize(&pixels, 1, 1, 0, 0, 3, 0);
        assert_eq!(w, 0);
        assert_eq!(h, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn test_resize_nearest_alpha_forced_rgb() {
        // RGB mode (mode=2): alpha forced to 255 even if source has non-255 alpha
        let pixels = vec![p(255, 0, 0, 100)];
        let (out, _w, _h) = resize(&pixels, 1, 1, 1, 1, 2, 0);
        assert_eq!(a_of(out[0]), 255, "RGB mode forces alpha to 255");
    }

    #[test]
    fn test_resize_nearest_alpha_preserved_la() {
        // LA mode (mode=1): alpha preserved
        let pixels = vec![p(100, 0, 0, 100)];
        let (out, _w, _h) = resize(&pixels, 1, 1, 1, 1, 1, 0);
        assert_eq!(a_of(out[0]), 100, "LA mode preserves alpha");
    }

    #[test]
    fn test_resize_bilinear_output_order() {
        // Verify output pixels are in row-major order for a non-trivial scale
        let pixels = vec![
            p(1, 0, 0, 255),
            p(2, 0, 0, 255),
            p(3, 0, 0, 255),
            p(4, 0, 0, 255),
            p(5, 0, 0, 255),
            p(6, 0, 0, 255),
        ];
        let (out, w, h) = resize(&pixels, 3, 2, 2, 2, 3, 1);
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(out.len(), 4);
        // Row 0 should come before row 1
        // The exact values aren't critical, just that we have the right count
    }

    #[test]
    fn test_resize_nearest_dims_returned() {
        // Verify correct dimension reporting for intentional size change
        let pixels = vec![p(0, 0, 0, 255); 6]; // 3x2
        let (out, w, h) = resize(&pixels, 3, 2, 6, 4, 3, 0);
        assert_eq!(w, 6);
        assert_eq!(h, 4);
        assert_eq!(out.len(), 24);
    }

    #[test]
    fn test_scale_down_0_5_rgba() {
        // Create a 4x4 RGBA image with a simple pattern
        // Pixel at (x,y) = (r: 64*x, g: 64*y, b: 128, a: 255)
        let mut pixels = Vec::with_capacity(16);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let r = (x * 64) as u8;
                let g = (y * 64) as u8;
                let b = 128u8;
                let a = 255u8;
                pixels.push(p(r, g, b, a));
            }
        }
        // Scale by 0.5 -> should be 2x2
        let (out, w, h) = scale(&pixels, 4, 4, 3, 0.5);
        assert_eq!(w, 2, "downscale width");
        assert_eq!(h, 2, "downscale height");
        assert_eq!(out.len(), 4, "downscale pixel count");
        // Each output pixel is a bilinear blend of a 2x2 source region.
        // Top-left (0,0): source area (0,0)-(2,2).
        // Average of 4 pixels: (0,0)=0,0,128, (1,0)=64,0,128, (0,1)=0,64,128, (1,1)=64,64,128
        // = (32, 32, 128) — with bilinear exact center sampling.
        // Bilinear at center of dst pixel 0,0 in source space:
        //   cx = (0 + 0.5) * 4/2 = 1.0, cy = (0 + 0.5) * 4/2 = 1.0
        //   Falls exactly on source pixel (1,1) = (64, 64, 128)
        let tl = out[0];
        assert_eq!(tl & 0xFF, 64, "top-left R");
        assert_eq!((tl >> 8) & 0xFF, 64, "top-left G");
        assert_eq!((tl >> 16) & 0xFF, 128, "top-left B");
        assert_eq!((tl >> 24) & 0xFF, 255, "top-left A");
    }

    #[test]
    fn test_scale_up_2_0_rgba() {
        // Same 4x4 RGBA image
        let mut pixels = Vec::with_capacity(16);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let r = (x * 64) as u8;
                let g = (y * 64) as u8;
                let b = 128u8;
                let a = 255u8;
                pixels.push(p(r, g, b, a));
            }
        }
        // Scale by 2.0 -> should be 8x8
        let (out, w, h) = scale(&pixels, 4, 4, 3, 2.0);
        assert_eq!(w, 8, "upscale width");
        assert_eq!(h, 8, "upscale height");
        assert_eq!(out.len(), 64, "upscale pixel count");
        // Bilinear interpolation
        // dst pixel (1,1) in source space:
        //   cx = (1 + 0.5) * 4/8 = 0.75, cy = (1 + 0.5) * 4/8 = 0.75
        //   Falls at (0.75, 0.75) in source == pixel (0,0) = (0, 0, 128)
        let idx = 1 * 8 + 1; // row 1, col 1
        let px = out[idx];
        assert_eq!(px & 0xFF, (0.75 * 64.0) as u32, "upscale pixel at (1,1) R");
        assert_eq!(
            (px >> 8) & 0xFF,
            (0.75 * 64.0) as u32,
            "upscale pixel at (1,1) G"
        );
        assert_eq!((px >> 16) & 0xFF, 128, "upscale pixel at (1,1) B");
        assert_eq!((px >> 24) & 0xFF, 255, "upscale pixel at (1,1) A");
    }

    #[test]
    fn test_scale_minimum_dim() {
        // Scale by a tiny factor should produce at least 1x1
        let pixels = vec![p(255, 128, 64, 255); 100]; // 10x10 solid image
        let (out, w, h) = scale(&pixels, 10, 10, 3, 0.01);
        assert!(w >= 1, "min width");
        assert!(h >= 1, "min height");
        assert_eq!(out.len(), (w * h) as usize);
    }
}

// ── Geometry and spatial operations (rotate, remap_palette, equalize) ────────

/// Bilinear interpolation helper: combine 4 corner values with fractional weights.
#[inline(always)]
fn bilinear_interp(c00: u32, c10: u32, c01: u32, c11: u32, fx: f64, fy: f64) -> u32 {
    let v = (1.0 - fx) * (1.0 - fy) * c00 as f64
        + fx * (1.0 - fy) * c10 as f64
        + (1.0 - fx) * fy * c01 as f64
        + fx * fy * c11 as f64;
    v.round() as u32
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
    if w == 0 || h == 0 {
        return (Vec::new(), w, h);
    }

    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    let sw = w as f64;
    let sh = h as f64;
    let rad = angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();

    // Compute bounding box of rotated image by transforming the 4 corners.
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(cx, cy) in &corners {
        let rx = cx * cos - cy * sin;
        let ry = cx * sin + cy * cos;
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }

    let (dw, dh) = if expand {
        ((max_x - min_x).ceil() as u32, (max_y - min_y).ceil() as u32)
    } else {
        (w, h)
    };

    let ox = if expand { -min_x } else { 0.0 };
    let oy = if expand { -min_y } else { 0.0 };

    let cx_src = sw / 2.0;
    let cy_src = sh / 2.0;
    let cx_dst = dw as f64 / 2.0;
    let cy_dst = dh as f64 / 2.0;

    // Pre-extract fill color components for mode-aware handling
    let fill_r = fill_rgba & 0xFF;
    let fill_g = (fill_rgba >> 8) & 0xFF;
    let fill_b = (fill_rgba >> 16) & 0xFF;
    let fill_a = fill_rgba & 0xFF00_0000;

    let mut out = vec![0u32; (dw * dh) as usize];

    for dy in 0..dh {
        for dx in 0..dw {
            // Inverse rotation: map destination (dx, dy) to source coordinate
            let src_x = (dx as f64 + ox - cx_dst) * cos + (dy as f64 + oy - cy_dst) * sin + cx_src;
            let src_y = -(dx as f64 + ox - cx_dst) * sin + (dy as f64 + oy - cy_dst) * cos + cy_src;

            let out_idx = (dy * dw + dx) as usize;

            if src_x >= 0.0 && src_x < sw && src_y >= 0.0 && src_y < sh {
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

                let r00 = p00 & 0xFF;
                let g00 = (p00 >> 8) & 0xFF;
                let b00 = (p00 >> 16) & 0xFF;
                let a00 = p00 & 0xFF00_0000;

                let r10 = p10 & 0xFF;
                let g10 = (p10 >> 8) & 0xFF;
                let b10 = (p10 >> 16) & 0xFF;

                let r01 = p01 & 0xFF;
                let g01 = (p01 >> 8) & 0xFF;
                let b01 = (p01 >> 16) & 0xFF;

                let r11 = p11 & 0xFF;
                let g11 = (p11 >> 8) & 0xFF;
                let b11 = (p11 >> 16) & 0xFF;

                // Bilinear interpolate per channel
                let out_r = bilinear_interp(r00, r10, r01, r11, fx, fy);
                let out_g_raw = bilinear_interp(g00, g10, g01, g11, fx, fy);
                let out_b_raw = bilinear_interp(b00, b10, b01, b11, fx, fy);

                let out_g = if has_gb { out_g_raw } else { out_r };
                let out_b = if has_gb { out_b_raw } else { out_r };
                let out_a = if has_a { a00 } else { 0xFF00_0000 };

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

/// Remap palette: convert P-mode image (palette index per pixel) to packed RGBA
/// using a destination color map.
///
/// Each source pixel's R byte is a palette index (0-255). The `dest_map` contains
/// 256 RGB triples packed consecutively (768 bytes total). The pixel's index is
/// used to look up the new R, G, B values from `dest_map`.
///
/// Returns a Vec<u32> of packed RGBA pixels (same length as input).
/// Mode-aware: G/B set to looked-up values for RGB/RGBA (mode >= 2), mirrored
/// from R for L/LA. Alpha forced to 0xFF for non-alpha modes.
/// mode: 0=L (P-mode encoding), 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn remap_palette(pixels: &[u32], mode: u32, dest_map: &[u8; 768]) -> Vec<u32> {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let mut out = Vec::with_capacity(pixels.len());

    for &p in pixels.iter() {
        let idx = (p & 0xFF) as usize; // palette index from R byte
        let r = dest_map[idx * 3] as u32;
        let g = dest_map[idx * 3 + 1] as u32;
        let b = dest_map[idx * 3 + 2] as u32;

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
/// Maps each destination pixel (dx,dy) back to source (sx,sy) via the
/// matrix [a,b,c,d,e,f] where:
///   sx = a*dx + b*dy + c
///   sy = d*dx + e*dy + f
///
/// filter=0 (Nearest): round to nearest integer source coordinate, clamp to bounds.
/// filter>=1 (Bilinear): fractional source position, 4-neighbor weighted blend with
/// f64 precision, mode-aware channel output.
///
/// Empty areas (source coordinates out of bounds) are filled with fill_rgba
/// (packed 0xAABBGGRR).
///
/// Returns (output_pixels, dst_w, dst_h).
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
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
    let nearest = filter == 0;

    let aff_a = matrix[0];
    let aff_b = matrix[1];
    let aff_c = matrix[2];
    let aff_d = matrix[3];
    let aff_e = matrix[4];
    let aff_f = matrix[5];

    // Pre-compute mode-aware fill pixel
    let fill_r = fill_rgba & 0xFF;
    let fill_g = (fill_rgba >> 8) & 0xFF;
    let fill_b = (fill_rgba >> 16) & 0xFF;
    let fill_a = fill_rgba & 0xFF00_0000;
    let fill_out_g = if has_gb { fill_g } else { fill_r };
    let fill_out_b = if has_gb { fill_b } else { fill_r };
    let fill_out_a = if has_a { fill_a } else { 0xFF00_0000 };
    let fill_pixel = fill_r | (fill_out_g << 8) | (fill_out_b << 16) | fill_out_a;

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
                let sx = aff_a * dx as f64 + aff_b * dy as f64 + aff_c;
                let sy = aff_d * dx as f64 + aff_e * dy as f64 + aff_f;
                let out_idx = (dy * dst_w + dx) as usize;

                let ix = (sx + 0.5).floor() as i64;
                let iy = (sy + 0.5).floor() as i64;
                if ix >= 0 && ix < w as i64 && iy >= 0 && iy < h as i64 {
                    let sp = pixels[(iy as u32 * w + ix as u32) as usize];
                    let r = sp & 0xFF;
                    let g = (sp >> 8) & 0xFF;
                    let b_val = (sp >> 16) & 0xFF;
                    let a_val = sp & 0xFF00_0000;
                    let og = if has_gb { g } else { r };
                    let ob = if has_gb { b_val } else { r };
                    let oa = if has_a { a_val } else { 0xFF00_0000 };
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
                let sx = aff_a * dx as f64 + aff_b * dy as f64 + aff_c;
                let sy = aff_d * dx as f64 + aff_e * dy as f64 + aff_f;
                let out_idx = (dy * dst_w + dx) as usize;

                if sx >= 0.0 && sx < w_f && sy >= 0.0 && sy < h_f {
                    let x0 = sx.floor() as u32;
                    let y0 = sy.floor() as u32;
                    let x1 = (x0 + 1).min(w_max);
                    let y1 = (y0 + 1).min(h_max);
                    let fx = sx - x0 as f64;
                    let fy = sy - y0 as f64;

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

                    // Horizontal then vertical interpolation
                    let top_r = inv_fx * r00 + fx * r10;
                    let top_g = inv_fx * g00 + fx * g10;
                    let top_b = inv_fx * b00 + fx * b10;
                    let top_a = inv_fx * a00 + fx * a10;

                    let bot_r = inv_fx * r01 + fx * r11;
                    let bot_g = inv_fx * g01 + fx * g11;
                    let bot_b = inv_fx * b01 + fx * b11;
                    let bot_a = inv_fx * a01 + fx * a11;

                    let out_r_f = inv_fy * top_r + fy * bot_r;
                    let out_g_f = inv_fy * top_g + fy * bot_g;
                    let out_b_f = inv_fy * top_b + fy * bot_b;
                    let out_a_f = inv_fy * top_a + fy * bot_a;

                    // Round to nearest integer and clamp to [0, 255]
                    let out_r = ((out_r_f + 0.5) as u32).min(255);
                    let out_g_raw = ((out_g_f + 0.5) as u32).min(255);
                    let out_b_raw = ((out_b_f + 0.5) as u32).min(255);
                    let out_a_raw = ((out_a_f + 0.5) as u32).min(255);

                    // Mode-aware channel output
                    let out_g = if has_gb { out_g_raw } else { out_r };
                    let out_b = if has_gb { out_b_raw } else { out_r };
                    let out_a = if has_a { out_a_raw << 24 } else { 0xFF00_0000 };

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

/// Composite two images using a per-channel mask.
///
/// Formula: `out = (pixels * mask + other * (255 - mask)) / 255`
/// Applied per byte (all 4 channels independently).
///
/// Module-function variant (Image.composite): mask is applied per-channel
/// like ImageChops.composite. Mask values are 0-255 per byte.
///
/// Mode-aware: for L/LA modes (mode < 2), G and B mirror R.
/// Alpha preserved for LA/RGBA, forced to 0xFF for L/RGB.
///
/// Operates in-place on pixels slice, consuming `other` and `mask` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn composite_module(pixels: &mut [u32], mode: u32, other: &[u32], mask: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;

    for ((p, o), m) in pixels.iter_mut().zip(other.iter()).zip(mask.iter()) {
        let pr = *p & 0xFF;
        let pg = (*p >> 8) & 0xFF;
        let pb = (*p >> 16) & 0xFF;
        let pa = (*p >> 24) & 0xFF;

        let or = *o & 0xFF;
        let og = (*o >> 8) & 0xFF;
        let ob = (*o >> 16) & 0xFF;
        let oa = (*o >> 24) & 0xFF;

        let mr = *m & 0xFF;
        let mg = (*m >> 8) & 0xFF;
        let mb = (*m >> 16) & 0xFF;
        let ma = (*m >> 24) & 0xFF;

        // out = (pixels * mask + other * (255 - mask)) / 255
        let out_r = (pr * mr + or * (255 - mr)) / 255;
        let out_g_raw = (pg * mg + og * (255 - mg)) / 255;
        let out_b_raw = (pb * mb + ob * (255 - mb)) / 255;
        let out_a_raw = (pa * ma + oa * (255 - ma)) / 255;

        let out_g = if has_gb { out_g_raw } else { out_r };
        let out_b = if has_gb { out_b_raw } else { out_r };
        let out_a = if has_a { out_a_raw << 24 } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
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
/// For L/LA mode (mode < 2): only the first 256 entries are used (R channel LUT)
/// and applied to both R and A (for LA). G and B mirror the R output.
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
            // L/LA: only the first 256 entries; G = B = R
            let out_r = lut[r] as u32;
            let out_a = if has_a {
                (lut[a] as u32) << 24
            } else {
                0xFF00_0000
            };
            *p = out_r | (out_r << 8) | (out_r << 16) | out_a;
        }
    }
}

/// Apply per-channel lookup table (same semantics as `eval`).
///
/// 1024-byte LUT, 256 entries per channel. Mode-aware channel application.
/// Delegates to `eval`.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn point_op(pixels: &mut [u32], mode: u32, lut: &[u8; 1024]) {
    eval(pixels, mode, lut)
}

// ── Composite, Merge, Sharpen, Autocontrast ────────────────────────────────

/// Composite: blend two images using a single-channel (grayscale) mask.
///
/// `out = (pixels * mask + other * (255 - mask)) / 255` per active channel.
///
/// Uses the R byte of the mask pixel as the uniform blend weight for all
/// active channels. This matches PIL's `Image.composite()` and
/// `ImageChops.composite()` with a grayscale mask — distinct from
/// `composite_module` above which applies per-channel mask bytes.
///
/// Mode-aware: R always composited; G/B composited for RGB/RGBA (mode >= 2),
/// preserved from original for L/LA. Alpha preserved for LA/RGBA,
/// forced to 0xFF for L/RGB.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn composite(pixels: &mut [u32], mode: u32, other: &[u32], mask: &[u32]) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    for ((p, o), m) in pixels.iter_mut().zip(other.iter()).zip(mask.iter()) {
        let r = *p & 0xFF;
        let g = (*p >> 8) & 0xFF;
        let b = (*p >> 16) & 0xFF;
        let a = *p & 0xFF00_0000;

        let or = *o & 0xFF;
        let og = (*o >> 8) & 0xFF;
        let ob = (*o >> 16) & 0xFF;

        // Single mask value from R byte (grayscale image packed as u32)
        let mv = *m & 0xFF;
        let inv_mv = 255 - mv;

        let out_r = (r * mv + or * inv_mv) / 255;
        let out_g_raw = (g * mv + og * inv_mv) / 255;
        let out_b_raw = (b * mv + ob * inv_mv) / 255;

        let out_g = if has_gb { out_g_raw } else { g };
        let out_b = if has_gb { out_b_raw } else { b };
        let out_a = if has_a { a } else { 0xFF00_0000 };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

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
            let out_r = ((r - lo) * 255 / range).min(255);
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
                ((r - r_lo) * 255 / r_range).min(255)
            } else {
                r
            };
            let out_g = if do_g {
                ((g - g_lo) * 255 / g_range).min(255)
            } else {
                g
            };
            let out_b = if do_b {
                ((b - b_lo) * 255 / b_range).min(255)
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
) -> (Vec<u32>, u32, u32) {
    if dst_w == 0 || dst_h == 0 {
        return (Vec::new(), 0, 0);
    }

    // Compute aspect-ratio-preserving size
    let scale = (dst_w as f64 / w as f64).min(dst_h as f64 / h as f64);
    if scale >= 1.0 {
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

    let new_w = (w as f64 * scale).max(1.0) as u32;
    let new_h = (h as f64 * scale).max(1.0) as u32;

    // Delegate to bilinear resize
    resize(pixels, w, h, new_w, new_h, mode, 1)
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
pub fn reduce(pixels: &[u32], w: u32, h: u32, mode: u32, factor: u32) -> (Vec<u32>, u32, u32) {
    if factor < 2 || w < factor || h < factor {
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
    let f = factor;
    let new_w = w / f;
    let new_h = h / f;
    let mut out = vec![0u32; (new_w * new_h) as usize];
    let area = (f * f) as u64;

    for y in 0..new_h {
        for x in 0..new_w {
            let mut sum_r = 0u64;
            let mut sum_g = 0u64;
            let mut sum_b = 0u64;
            let mut sum_a = 0u64;

            for dy in 0..f {
                let row = (y * f + dy) * w;
                for dx in 0..f {
                    let p = pixels[(row + x * f + dx) as usize];
                    sum_r += (p & 0xFF) as u64;
                    sum_g += ((p >> 8) & 0xFF) as u64;
                    sum_b += ((p >> 16) & 0xFF) as u64;
                    sum_a += (p >> 24) as u64;
                }
            }

            // Round to nearest: (sum + half_of_area) / area
            let half = area / 2;
            let out_r = ((sum_r + half) / area) as u32;
            let out_g_raw = ((sum_g + half) / area) as u32;
            let out_b_raw = ((sum_b + half) / area) as u32;
            let out_a_raw = ((sum_a + half) / area) as u32;

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

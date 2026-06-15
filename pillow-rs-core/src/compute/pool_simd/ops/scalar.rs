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
        let r = (*p & 0x0000_00FF) as u32;
        let g = ((*p >> 8) & 0xFF) as u32;
        let b = ((*p >> 16) & 0xFF) as u32;
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

        let out_r = ((r * factor_fp / 1000).min(255)) as u32;
        let out_g_raw = ((g * factor_fp / 1000).min(255)) as u32;
        let out_b_raw = ((b * factor_fp / 1000).min(255)) as u32;

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

/// Sharpen filter on packed u32 RGBA pixels.
///
/// Fixed 3x3 sharpen kernel (PIL ImageFilter.SHARPEN):
///   [-2, -2, -2,
///    -2, 32, -2,
///    -2, -2, -2]
///   scale=16, offset=0
///
/// `factor_fp` is a fixed-point factor (factor * 1000) that interpolates
/// between the original image and the sharpened result:
///   result = lerp(original, sharpened, factor_fp / 1000.0)
/// - factor_fp = 0     -> original image (no sharpen)
/// - factor_fp = 1000  -> full sharpen effect
/// - factor_fp = 2000  -> double sharpness (extrapolation)
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn sharpen(pixels: &mut [u32], w: u32, h: u32, mode: u32, factor_fp: u32) {
    let has_gb = mode >= 2;
    let has_a = mode == 1 || mode == 3;
    let src = pixels.to_vec();
    let w_i = w as i32;
    let h_i = h as i32;
    let half = 1i32;
    // Fixed sharpen kernel (PIL ImageFilter.SHARPEN)
    let kernel: [f32; 9] = [
        -2.0, -2.0, -2.0, //
        -2.0, 32.0, -2.0, //
        -2.0, -2.0, -2.0,
    ];
    let scale = 16.0;
    // Interpolation weight: factor_fp / 1000
    let t = factor_fp as f32 / 1000.0;

    for y in 0..h_i {
        for x in 0..w_i {
            let idx = (y * w_i + x) as usize;
            let orig = src[idx];
            let orig_r = orig & 0xFF;
            let orig_g = (orig >> 8) & 0xFF;
            let orig_b = (orig >> 16) & 0xFF;
            let orig_a = orig & 0xFF00_0000;

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

            let sharp_r = ((sum_r / scale + 0.5) as i32).clamp(0, 255) as u32;
            let sharp_g_raw = ((sum_g / scale + 0.5) as i32).clamp(0, 255) as u32;
            let sharp_b_raw = ((sum_b / scale + 0.5) as i32).clamp(0, 255) as u32;

            let sharp_g = if has_gb { sharp_g_raw } else { sharp_r };
            let sharp_b = if has_gb { sharp_b_raw } else { sharp_r };

            // Lerp: original * (1 - t) + sharpened * t
            let out_r = ((orig_r as f32 * (1.0 - t) + sharp_r as f32 * t) + 0.5) as u32;
            let out_g = if has_gb {
                ((orig_g as f32 * (1.0 - t) + sharp_g as f32 * t) + 0.5) as u32
            } else {
                out_r
            };
            let out_b = if has_gb {
                ((orig_b as f32 * (1.0 - t) + sharp_b as f32 * t) + 0.5) as u32
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

/// Effect spread: randomly scatter pixels within a given distance.
///
/// For each pixel at (x, y), computes a random offset (dx, dy) in
/// [-distance/2, distance/2] and swaps the pixel values — matching PIL's
/// ImagingEffectSpread. Uses a deterministic LCG seeded by pixel position
/// for reproducible results across platforms.
///
/// Border pixels are handled by clamping the random offset to image bounds.
/// When the random offset lands out of bounds, the pixel stays at its
/// original position. Multiple pixels mapping to the same destination
/// follow PIL's last-write-wins semantics.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn effect_spread(pixels: &mut [u32], w: u32, h: u32, mode: u32, distance: u32) {
    if distance == 0 {
        return;
    }
    let d = distance as i32;
    let half_d = d / 2;
    let has_a = mode == 1 || mode == 3;
    let src = pixels.to_vec();
    let w_i = w as i32;
    let h_i = h as i32;

    for y in 0..h_i {
        for x in 0..w_i {
            // Deterministic LCG: glibc-style rand() seeded by pixel position.
            // Two iterations give two independent random values for dx and dy.
            let mut rng = (y * w_i + x) as u64;
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let rand1 = (rng >> 16) as i32 & 0x7FFF;
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let rand2 = (rng >> 16) as i32 & 0x7FFF;

            let xx = x + (rand1 % d) - half_d;
            let yy = y + (rand2 % d) - half_d;

            if xx >= 0 && xx < w_i && yy >= 0 && yy < h_i {
                let src_idx = (y * w_i + x) as usize;
                let dst_idx = (yy * w_i + xx) as usize;
                // Swap: current pixel goes to random offset, random pixel
                // comes to current position (PIL ImagingEffectSpread behavior).
                let cur = src[src_idx];
                pixels[dst_idx] = cur;
                pixels[src_idx] = src[dst_idx];
            }
            // Out of bounds: pixel stays at original position (no write needed)
        }
    }

    // Clamp alpha for non-alpha modes (swapped pixels may carry garbage alpha)
    if !has_a {
        for p in pixels.iter_mut() {
            *p |= 0xFF00_0000;
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
                let l_out = l_r | (l_r << 8) | (l_r << 16)
                    | if has_a { l_a } else { 0xFF00_0000 };
                let r_out = r_r | (r_r << 8) | (r_r << 16)
                    | if has_a { r_a } else { 0xFF00_0000 };
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
                pixels[mid] = r | (r << 8) | (r << 16)
                    | if has_a { a } else { 0xFF00_0000 };
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
                        out[dst_idx] = r | (r << 8) | (r << 16)
                            | if has_a { a } else { 0xFF00_0000 };
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
                    pixels[i] = rj_r | (rj_r << 8) | (rj_r << 16)
                        | if has_a { rj_a } else { 0xFF00_0000 };
                    pixels[j] = li_r | (li_r << 8) | (li_r << 16)
                        | if has_a { li_a } else { 0xFF00_0000 };
                }
            }
            // Middle pixel if odd total
            if total % 2 == 1 && !has_gb {
                let mid = total / 2;
                let p = pixels[mid];
                let r = p & 0xFF;
                let a = p & 0xFF00_0000;
                pixels[mid] = r | (r << 8) | (r << 16)
                    | if has_a { a } else { 0xFF00_0000 };
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
                        out[dst_idx] = r | (r << 8) | (r << 16)
                            | if has_a { a } else { 0xFF00_0000 };
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
                        out[dst_idx] = r | (r << 8) | (r << 16)
                            | if has_a { a } else { 0xFF00_0000 };
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
                        out[dst_idx] = r | (r << 8) | (r << 16)
                            | if has_a { a } else { 0xFF00_0000 };
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
                hpass[idx] = out_r | (out_g << 8) | (out_b << 16) | if has_a { a } else { 0xFF00_0000 };
            } else {
                // L/LA: only R channel matters, G=B=R
                let mut acc_r = 0u64;
                for dx in -r..=r {
                    let sx = (x + dx).clamp(0, w_i - 1);
                    acc_r += (pixels[(y * w_i + sx) as usize] & 0xFF) as u64;
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let a = sp & 0xFF00_0000;
                hpass[idx] = out_r | (out_r << 8) | (out_r << 16)
                    | if has_a { a } else { 0xFF00_0000 };
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
                pixels[idx] = out_r | (out_g << 8) | (out_b << 16) | if has_a { a } else { 0xFF00_0000 };
            } else {
                let mut acc_r = 0u64;
                for dy in -r..=r {
                    let sy = (y + dy).clamp(0, h_i - 1);
                    acc_r += (hpass[(sy * w_i + x) as usize] & 0xFF) as u64;
                }
                let out_r = ((acc_r * ww as u64 + bias as u64) >> 24) as u32;
                let a = sp & 0xFF00_0000;
                pixels[idx] = out_r | (out_r << 8) | (out_r << 16)
                    | if has_a { a } else { 0xFF00_0000 };
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

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

//! Scalar fallback for pointwise ImageChops arithmetic and blends.

/// InvertChops: 255 - every channel in the image.
///
/// Unlike ImageOps.invert, ImageChops.invert includes the alpha byte. The
/// previous SIMD implementation reused the ImageOps alpha-preserving rule,
/// which first diverged for LA/RGBA inputs with non-opaque alpha.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn invert_chops(pixels: &mut [u32], mode: u32) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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

/// Logical AND: nonzero inputs produce 255, otherwise zero, per channel.
/// Dual-input: operates on `pixels` and `other` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn logical_and(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = u32::from(ar != 0 && br != 0) * 255;
        let out_g_raw = u32::from(ag != 0 && bg != 0) * 255;
        let out_b_raw = u32::from(ab != 0 && bb != 0) * 255;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            (u32::from(aa != 0 && ba != 0) * 255) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Logical OR: either nonzero input produces 255, otherwise zero, per channel.
/// Dual-input: operates on `pixels` and `other` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn logical_or(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = u32::from((ar | br) != 0) * 255;
        let out_g_raw = u32::from((ag | bg) != 0) * 255;
        let out_b_raw = u32::from((ab | bb) != 0) * 255;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            (u32::from(((aa >> 24) | ba) != 0) * 255) << 24
        } else {
            0xFF00_0000
        };

        *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
    }
}

/// Logical XOR: differing input truths produce 255, otherwise zero, per channel.
/// Dual-input: operates on `pixels` and `other` element-wise.
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn logical_xor(pixels: &mut [u32], mode: u32, other: &[u32]) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    for (p, o) in pixels.iter_mut().zip(other.iter()) {
        let ar = *p & 0xFF;
        let ag = (*p >> 8) & 0xFF;
        let ab = (*p >> 16) & 0xFF;
        let aa = *p & 0xFF00_0000;
        let br = *o & 0xFF;
        let bg = (*o >> 8) & 0xFF;
        let bb = (*o >> 16) & 0xFF;
        let ba = (*o >> 24) & 0xFF;

        let out_r = u32::from((ar != 0) ^ (br != 0)) * 255;
        let out_g_raw = u32::from((ag != 0) ^ (bg != 0)) * 255;
        let out_b_raw = u32::from((ab != 0) ^ (bb != 0)) * 255;

        let out_g = if has_gb { out_g_raw } else { ag };
        let out_b = if has_gb { out_b_raw } else { ab };
        let out_a = if has_a {
            (u32::from((aa != 0) ^ (ba != 0)) * 255) << 24
        } else {
            0xFF00_0000
        };

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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
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

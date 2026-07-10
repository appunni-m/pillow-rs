//! Fixed-point arithmetic compatible with FreeType's integer helpers.
//!
//! These routines mirror the fixed-point contracts used by FreeType 2.14.3:
//! 16.16 multiplication and division, 26.6 pixel snapping, wrapping
//! two's-complement arithmetic, and C-style sentinel values for division by
//! zero. They avoid floating point so the scaler, auto-hinter, and TrueType VM
//! share one deterministic arithmetic model.

/// Wrapping 32-bit addition used by FreeType's `ADD_LONG` macro.
#[inline]
fn add_long(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

/// Wrapping two's-complement negation used by FreeType's `NEG_LONG` macro.
#[inline]
fn neg_long(a: i32) -> i32 {
    0i32.wrapping_sub(a)
}

use crate::casts::{i32_from_i64, i32_from_u64};

/// FreeType `FT_MulDiv` with rounded sign-stripped integer division.
///
/// Sign-stripping: converts to unsigned magnitudes, does unsigned multiply +
/// add half-divisor + divide, then restores sign with XOR of sign bits.
/// Returns FreeType's `0x7fff_ffff` sentinel when `c` is zero.
#[inline]
pub fn ft_mul_div(a: i32, b: i32, c: i32) -> i32 {
    if c == 0 {
        return 0x7FFFFFFF;
    }
    let ua: u64 = (a as i64).unsigned_abs();
    let ub: u64 = (b as i64).unsigned_abs();
    let uc: u64 = (c as i64).unsigned_abs();
    let d = (ua.wrapping_mul(ub) + (uc >> 1)) / uc;
    let d32 = i32_from_u64(d);
    let negate = ((a < 0) ^ (b < 0)) ^ (c < 0);
    if negate { 0i32.wrapping_sub(d32) } else { d32 }
}

/// FT_MulDiv_No_Round — matches C's sign-stripped truncating division.
///
/// C reference: `src/base/ftcalc.c:187-207`. TrueType `DIV[]` uses this
/// no-round variant, unlike `MUL[]`, which uses rounded `FT_MulDiv`.
#[inline]
pub fn ft_mul_div_no_round(a: i32, b: i32, c: i32) -> i32 {
    if c == 0 {
        return 0x7FFFFFFF;
    }
    let ua: u64 = (a as i64).unsigned_abs();
    let ub: u64 = (b as i64).unsigned_abs();
    let uc: u64 = (c as i64).unsigned_abs();
    let d32 = i32_from_u64(ua.wrapping_mul(ub) / uc);
    let negate = ((a < 0) ^ (b < 0)) ^ (c < 0);
    if negate { 0i32.wrapping_sub(d32) } else { d32 }
}

/// FreeType `FT_MulFix`: multiply by a 16.16 fixed-point factor.
///
/// `(ab + 0x8000 + (ab >> 63)) >> 16` with symmetric rounding.
/// The `ab >> 63` term is -1 for negative products, 0 for positive —
/// giving FreeType's rounded-toward-infinity behavior for both sign cases.
#[inline]
pub fn ft_mul_fix(a: i32, b: i32) -> i32 {
    let ab = (a as i64).wrapping_mul(b as i64);
    let rounded = ab.wrapping_add(0x8000).wrapping_add(ab >> 63);
    i32_from_i64(rounded >> 16)
}

/// FreeType `FT_DivFix`: divide and return a 16.16 fixed-point quotient.
///
/// C uses sign-stripping: `((|a|<<16) + (|b|>>1)) / |b|` as unsigned,
/// then negates if input signs differ. This differs from signed division
/// `((a<<16)+(b>>1))/b` when the numerator magnitude isn't evenly divisible
/// — signed truncation-toward-zero produces a different result than
/// unsigned-floor-then-negate.
/// Returns FreeType's `0x7fff_ffff` sentinel when `b` is zero.
#[inline]
pub fn ft_div_fix(a: i32, b: i32) -> i32 {
    // C's FT_DivFix (ftcalc.c:233, INT64 path) uses sign-stripping:
    //   FT_MOVE_SIGN(a) → ua, s; FT_MOVE_SIGN(b) → ub, s;
    //   q = ((ua << 16) + (ub >> 1)) / ub (unsigned division);
    //   return s < 0 ? -q : q;
    // This is NOT the same as signed ((a<<16)+(b>>1))/b because
    // signed division truncates toward zero vs floor behavior of
    // the sign-stripped unsigned path.
    if b == 0 {
        return 0x7FFFFFFF;
    }
    let ua: u64 = (a as i64).unsigned_abs();
    let ub: u64 = (b as i64).unsigned_abs();
    let q = ((ua << 16) + (ub >> 1)) / ub;
    let q32 = i32_from_u64(q);
    let negate = (a < 0) ^ (b < 0);
    if negate { 0i32.wrapping_sub(q32) } else { q32 }
}

/// FreeType `FT_RoundFix`: round a 16.16 value to an integral 16.16 value.
///
/// `ADD_LONG(a, 0x8000L - (a < 0)) & ~0xFFFFL`.
#[inline]
pub fn ft_round_fix(a: i32) -> i32 {
    let bias = 0x8000i32 - i32::from(a < 0);
    add_long(a, bias) & !0xFFFFi32
}

/// FreeType `FT_CeilFix`: ceiling a 16.16 value to an integral 16.16 value.
///
/// Reference: `ftcalc.c:84`. `ADD_LONG(a, 0xFFFFL) & ~0xFFFFL`.
#[inline]
pub fn ft_ceil_fix(a: i32) -> i32 {
    add_long(a, 0xFFFF) & !0xFFFFi32
}

/// FreeType `FT_FloorFix`: floor a 16.16 value to an integral 16.16 value.
///
/// Reference: `ftcalc.c:93`. `a & ~0xFFFFL`.
#[inline]
pub fn ft_floor_fix(a: i32) -> i32 {
    a & !0xFFFFi32
}

#[inline]
fn ft_msb(value: u32) -> i32 {
    31 - value.leading_zeros() as i32
}

/// FreeType `FT_Vector_Length` for a signed 32-bit vector.
///
/// C reference: `fttrigon.c:417-448`.  FreeType uses a fixed-point CORDIC
/// approximation here, so an IEEE-754 hypotenuse can differ by one unit.
pub fn ft_vector_length(mut x: i32, mut y: i32) -> i32 {
    if x == 0 {
        return y.wrapping_abs();
    }
    if y == 0 {
        return x.wrapping_abs();
    }

    let msb = ft_msb(x.wrapping_abs() as u32 | y.wrapping_abs() as u32);
    let shift = if msb <= 29 {
        let shift = 29 - msb;
        x = ((x as u32) << shift) as i32;
        y = ((y as u32) << shift) as i32;
        shift
    } else {
        let shift = msb - 29;
        x >>= shift;
        y >>= shift;
        -shift
    };

    if y > x {
        if y > x.wrapping_neg() {
            let old_x = x;
            x = y;
            y = old_x.wrapping_neg();
        } else {
            x = x.wrapping_neg();
            y = y.wrapping_neg();
        }
    } else if y < x.wrapping_neg() {
        let old_x = x;
        x = y.wrapping_neg();
        y = old_x;
    }

    let mut bias = 1i32;
    for i in 1..23 {
        let old_x = x;
        if y > 0 {
            x = x.wrapping_add(y.wrapping_add(bias) >> i);
            y = y.wrapping_sub(old_x.wrapping_add(bias) >> i);
        } else {
            x = x.wrapping_sub(y.wrapping_add(bias) >> i);
            y = y.wrapping_add(old_x.wrapping_add(bias) >> i);
        }
        bias = bias.wrapping_shl(1);
    }

    let magnitude = x.wrapping_abs() as u64;
    let downscaled = ((magnitude * 0xDBD9_5B16 + 0x4000_0000) >> 32) as i32;
    if shift > 0 {
        downscaled.wrapping_add(1 << (shift - 1)) >> shift
    } else {
        ((downscaled as u32) << -shift) as i32
    }
}

/// FreeType `FT_Vector_NormLen` for a signed 32-bit vector.
///
/// Returns the normalized 16.16 vector and its original length.  C reference:
/// `ftcalc.c:787-877`.
pub fn ft_vector_norm_len(vx: i32, vy: i32) -> ((i32, i32), u32) {
    let mut sx = 1i32;
    let mut sy = 1i32;
    let mut x = if vx < 0 {
        sx = -1;
        (0u32).wrapping_sub(vx as u32)
    } else {
        vx as u32
    };
    let mut y = if vy < 0 {
        sy = -1;
        (0u32).wrapping_sub(vy as u32)
    } else {
        vy as u32
    };

    if x == 0 {
        return ((0, if y > 0 { sy * 0x10000 } else { 0 }), y);
    }
    if y == 0 {
        return (((if x > 0 { sx * 0x10000 } else { 0 }), 0), x);
    }

    let mut length = if x > y { x + (y >> 1) } else { y + (x >> 1) };
    let mut shift = 31 - ft_msb(length);
    shift -= 15 + i32::from(length >= (0xAAAA_AAAAu32 >> shift));

    if shift > 0 {
        x <<= shift;
        y <<= shift;
        length = if x > y { x + (y >> 1) } else { y + (x >> 1) };
    } else {
        x >>= -shift;
        y >>= -shift;
        length >>= -shift;
    }

    let mut b = 0x10000i32.wrapping_sub(length as i32);
    let x_i = x as i32;
    let y_i = y as i32;
    let (u, v) = loop {
        let u = x_i.wrapping_add((x_i.wrapping_mul(b)) >> 16) as u32;
        let v = y_i.wrapping_add((y_i.wrapping_mul(b)) >> 16) as u32;
        let mut z =
            (u.wrapping_mul(u).wrapping_add(v.wrapping_mul(v)) as i32).wrapping_neg() / 0x200;
        z = z.wrapping_mul((0x10000i32.wrapping_add(b)) >> 8) / 0x10000;
        if z <= 0 {
            break (u, v);
        }
        b = b.wrapping_add(z);
    };

    let normalized = (
        if sx < 0 {
            0i32.wrapping_sub(u as i32)
        } else {
            u as i32
        },
        if sy < 0 {
            0i32.wrapping_sub(v as i32)
        } else {
            v as i32
        },
    );
    length = 0x10000u32
        .wrapping_add((u.wrapping_mul(x).wrapping_add(v.wrapping_mul(y)) as i32 / 0x10000) as u32);
    if shift > 0 {
        length = length.wrapping_add(1 << (shift - 1)) >> shift;
    } else {
        length <<= -shift;
    }
    (normalized, length)
}

/// Normalize a TrueType vector to a 2.14 unit vector.
///
/// C reference: `Normalize` in `ttinterp.c:2326-2345`, which calls
/// `FT_Vector_NormLen` in `ftcalc.c:787-877` and then divides the 16.16
/// normalized vector by 4.  This intentionally avoids floating point; one-unit
/// cbox differences can appear if SPVFS/SFVFS or line-vector opcodes use a
/// host `sqrt` instead of FreeType's fixed Newton iteration.
#[inline]
pub fn ft_normalize_2dot14(vx: i32, vy: i32) -> Option<(i32, i32)> {
    if vx == 0 && vy == 0 {
        return None;
    }
    let ((x_norm, y_norm), _) = ft_vector_norm_len(vx, vy);
    Some((x_norm / 4, y_norm / 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_fix_identity() {
        // 1.0 * 1.0 = 1.0  in 16.16
        assert_eq!(ft_mul_fix(0x1_0000, 0x1_0000), 0x1_0000);
    }

    #[test]
    fn mul_fix_half_by_two() {
        // 0.5 * 2.0 = 1.0
        assert_eq!(ft_mul_fix(0x8000, 0x2_0000), 0x1_0000);
    }

    #[test]
    fn mul_fix_negative_rounding() {
        // -0.5 (16.16) * 2.0 = -1.0; verifies sign-extension rounding.
        assert_eq!(ft_mul_fix(-0x8000, 0x2_0000), -0x1_0000);
    }

    #[test]
    fn div_fix_scale_factor() {
        // 16ppem in 26.6 (0x400) over UPM 2048 → 16.16 scale 0x8000 (0.5).
        let ppem_26dot6: i32 = 16 << 6;
        assert_eq!(ft_div_fix(ppem_26dot6, 2048), 0x8000);
    }

    #[test]
    fn div_fix_negative_divisor() {
        // C's FT_DivFix sign-stripping: (|1024|<<16 + |2048|>>1) / |2048| = 32768
        // negate (a,b signs differ) → -32768
        let r = ft_div_fix(16 << 6, -2048);
        assert_eq!(r, -0x8000);
    }

    #[test]
    fn round_fix_half_up() {
        // 0.5 (16.16 = 0x8000) rounds to 1.0 (0x10000).
        assert_eq!(ft_round_fix(0x8000), 0x1_0000);
    }

    #[test]
    fn ceil_fix_rounds_near_zero_up() {
        assert_eq!(ft_ceil_fix(0x1), 0x1_0000);
    }

    #[test]
    fn floor_fix_truncates_sub_pixel() {
        assert_eq!(ft_floor_fix(0x_FFFF), 0x0);
    }

    #[test]
    fn mul_div_basic() {
        // (3 * 5 + 1) / 2 = 8
        assert_eq!(ft_mul_div(3, 5, 2), 8);
    }

    #[test]
    fn mul_div_negative() {
        // C sign-stripping: (|3|*|5| + |2|>>1) / |2| = (15+1)/2 = 8
        // negate((a<0)^(b<0)^(c<0)) = negate(true) = -8
        assert_eq!(ft_mul_div(-3, 5, 2), -8);
    }
}

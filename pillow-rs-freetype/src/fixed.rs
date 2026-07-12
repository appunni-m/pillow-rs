//! Fixed-point arithmetic compatible with FreeType's integer helpers.
//!
//! These routines mirror the fixed-point contracts used by FreeType 2.14.3:
//! 16.16 multiplication and division, 26.6 pixel snapping, wrapping
//! two's-complement arithmetic, and C-style sentinel values for division by
//! zero. They avoid floating point so the scaler, auto-hinter, and TrueType VM
//! share one deterministic arithmetic model.

/// Wrapping native-long addition used by FreeType's `ADD_LONG` macro.
#[inline]
pub(crate) fn ft_add_long(a: i64, b: i64) -> i64 {
    (a as u64).wrapping_add(b as u64) as i64
}

/// Wrapping native-long two's-complement negation used by FreeType's `NEG_LONG` macro.
#[inline]
pub(crate) fn ft_neg_long(a: i64) -> i64 {
    0u64.wrapping_sub(a as u64) as i64
}

#[inline]
fn i32_wrap_from_i64(x: i64) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    {
        x as i32
    }
}

#[inline]
fn move_long_sign(value: i64, sign: i32) -> (u64, i32) {
    if value < 0 {
        (0u64.wrapping_sub(value as u64), -sign)
    } else {
        (value as u64, sign)
    }
}

/// FreeType `FT_MulDiv` over the public `FT_Long` domain.
#[inline]
pub(crate) fn ft_mul_div_long(a: i64, b: i64, c: i64) -> i64 {
    let (a, sign) = move_long_sign(a, 1);
    let (b, sign) = move_long_sign(b, sign);
    let (c, sign) = move_long_sign(c, sign);
    let d = a
        .wrapping_mul(b)
        .wrapping_add(c >> 1)
        .checked_div(c)
        .unwrap_or(0x7FFF_FFFF) as i64;
    if sign < 0 { ft_neg_long(d) } else { d }
}

/// FreeType `FT_MulDiv_No_Round` over the public `FT_Long` domain.
#[inline]
pub(crate) fn ft_mul_div_no_round_long(a: i64, b: i64, c: i64) -> i64 {
    let (a, sign) = move_long_sign(a, 1);
    let (b, sign) = move_long_sign(b, sign);
    let (c, sign) = move_long_sign(c, sign);
    let d = a.wrapping_mul(b).checked_div(c).unwrap_or(0x7FFF_FFFF) as i64;
    if sign < 0 { ft_neg_long(d) } else { d }
}

/// FreeType `FT_MulFix` over the public `FT_Long` domain.
#[inline]
pub(crate) fn ft_mul_fix_long(a: i64, b: i64) -> i64 {
    let ab = (a as u64).wrapping_mul(b as u64) as i64;
    ft_add_long(ft_add_long(ab, 0x8000), ab >> 63) >> 16
}

/// FreeType `FT_DivFix` over the public `FT_Long` domain.
#[inline]
pub(crate) fn ft_div_fix_long(a: i64, b: i64) -> i64 {
    let (a, sign) = move_long_sign(a, 1);
    let (b, sign) = move_long_sign(b, sign);
    let q = a
        .wrapping_shl(16)
        .wrapping_add(b >> 1)
        .checked_div(b)
        .unwrap_or(0x7FFF_FFFF) as i64;
    if sign < 0 { ft_neg_long(q) } else { q }
}

/// FreeType `FT_RoundFix` over the public `FT_Fixed` domain.
#[inline]
pub(crate) fn ft_round_fix_long(a: i64) -> i64 {
    ft_add_long(a, 0x8000 - i64::from(a < 0)) & !0xFFFF
}

/// FreeType `FT_CeilFix` over the public `FT_Fixed` domain.
#[inline]
pub(crate) fn ft_ceil_fix_long(a: i64) -> i64 {
    ft_add_long(a, 0xFFFF) & !0xFFFF
}

/// FreeType `FT_FloorFix` over the public `FT_Fixed` domain.
#[inline]
pub(crate) fn ft_floor_fix_long(a: i64) -> i64 {
    a & !0xFFFF
}

/// FreeType `FT_MulDiv` with rounded sign-stripped integer division.
///
/// Sign-stripping: converts to unsigned magnitudes, does unsigned multiply +
/// add half-divisor + divide, then restores sign with XOR of sign bits.
/// Returns FreeType's `0x7fff_ffff` sentinel when `c` is zero.
#[inline]
pub fn ft_mul_div(a: i32, b: i32, c: i32) -> i32 {
    i32_wrap_from_i64(ft_mul_div_long(i64::from(a), i64::from(b), i64::from(c)))
}

/// FT_MulDiv_No_Round — matches C's sign-stripped truncating division.
///
/// C reference: `src/base/ftcalc.c:187-207`. TrueType `DIV[]` uses this
/// no-round variant, unlike `MUL[]`, which uses rounded `FT_MulDiv`.
#[inline]
pub fn ft_mul_div_no_round(a: i32, b: i32, c: i32) -> i32 {
    i32_wrap_from_i64(ft_mul_div_no_round_long(
        i64::from(a),
        i64::from(b),
        i64::from(c),
    ))
}

/// FreeType `FT_MulFix`: multiply by a 16.16 fixed-point factor.
///
/// `(ab + 0x8000 + (ab >> 63)) >> 16` with symmetric rounding.
/// The `ab >> 63` term is -1 for negative products, 0 for positive —
/// giving FreeType's rounded-toward-infinity behavior for both sign cases.
#[inline]
pub fn ft_mul_fix(a: i32, b: i32) -> i32 {
    i32_wrap_from_i64(ft_mul_fix_long(i64::from(a), i64::from(b)))
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
    i32_wrap_from_i64(ft_div_fix_long(i64::from(a), i64::from(b)))
}

/// FreeType `FT_RoundFix`: round a 16.16 value to an integral 16.16 value.
///
/// `FT_Fixed` is a native signed long in FreeType, so this keeps the same
/// native-long domain as `ftcalc.c:76-78`.
#[inline]
pub fn ft_round_fix(a: i64) -> i64 {
    ft_round_fix_long(a)
}

/// FreeType `FT_CeilFix`: ceiling a 16.16 value to an integral 16.16 value.
///
/// `FT_Fixed` is a native signed long in FreeType, so this keeps the same
/// native-long domain as `ftcalc.c:85-87`.
#[inline]
pub fn ft_ceil_fix(a: i64) -> i64 {
    ft_ceil_fix_long(a)
}

/// FreeType `FT_FloorFix`: floor a 16.16 value to an integral 16.16 value.
///
/// `FT_Fixed` is a native signed long in FreeType, so this keeps the same
/// native-long domain as `ftcalc.c:94-96`.
#[inline]
pub fn ft_floor_fix(a: i64) -> i64 {
    ft_floor_fix_long(a)
}

#[inline]
fn ft_msb(value: u32) -> i32 {
    31 - value.leading_zeros() as i32
}

const FT_TRIG_SCALE: u64 = 0xDBD9_5B16;
const FT_TRIG_SAFE_MSB: i32 = 29;
const FT_TRIG_MAX_ITERS: i32 = 23;

#[inline]
fn ft_abs_long(value: i64) -> i64 {
    if value < 0 { ft_neg_long(value) } else { value }
}

#[inline]
fn ft_trig_downscale_long(value: i64) -> i64 {
    let (value, sign) = move_long_sign(value, 1);
    let value = ((u128::from(value) * u128::from(FT_TRIG_SCALE) + 0x4000_0000) >> 32) as i64;
    if sign < 0 { ft_neg_long(value) } else { value }
}

#[inline]
fn ft_trig_prenorm_long(x: &mut i64, y: &mut i64) -> i32 {
    let old_x = *x;
    let old_y = *y;
    let mut shift = ft_msb((ft_abs_long(old_x) as u32) | (ft_abs_long(old_y) as u32));
    if shift <= FT_TRIG_SAFE_MSB {
        shift = FT_TRIG_SAFE_MSB - shift;
        *x = (old_x as u64).wrapping_shl(shift as u32) as i64;
        *y = (old_y as u64).wrapping_shl(shift as u32) as i64;
    } else {
        shift -= FT_TRIG_SAFE_MSB;
        *x = old_x >> shift;
        *y = old_y >> shift;
        shift = -shift;
    }
    shift
}

#[inline]
fn ft_trig_pseudo_polarize_length_long(x: &mut i64, y: &mut i64) {
    if *y > *x {
        if *y > ft_neg_long(*x) {
            let old_x = *x;
            *x = *y;
            *y = ft_neg_long(old_x);
        } else {
            *x = ft_neg_long(*x);
            *y = ft_neg_long(*y);
        }
    } else if *y < ft_neg_long(*x) {
        let old_x = *x;
        *x = ft_neg_long(*y);
        *y = old_x;
    }

    let mut bias = 1i64;
    for i in 1..FT_TRIG_MAX_ITERS {
        let old_x = *x;
        if *y > 0 {
            *x += (*y + bias) >> i;
            *y -= (old_x + bias) >> i;
        } else {
            *x -= (*y + bias) >> i;
            *y += (old_x + bias) >> i;
        }
        bias <<= 1;
    }
}

/// FreeType `FT_Vector_Length` over the public `FT_Long` domain.
///
/// C reference: `fttrigon.c:417-448`. FreeType normalizes with the low
/// 32 bits of the absolute vector components, then runs fixed-point CORDIC.
pub(crate) fn ft_vector_length_long(mut x: i64, mut y: i64) -> i64 {
    if x == 0 {
        return ft_abs_long(y);
    }
    if y == 0 {
        return ft_abs_long(x);
    }

    let shift = ft_trig_prenorm_long(&mut x, &mut y);
    ft_trig_pseudo_polarize_length_long(&mut x, &mut y);
    x = ft_trig_downscale_long(x);

    if shift > 0 {
        (x + (1i64 << (shift - 1))) >> shift
    } else {
        (x as u64).wrapping_shl((-shift) as u32) as i64
    }
}

/// FreeType `FT_Vector_Length` for a signed 32-bit vector.
///
/// C reference: `fttrigon.c:417-448`.  FreeType uses a fixed-point CORDIC
/// approximation here, so an IEEE-754 hypotenuse can differ by one unit.
pub fn ft_vector_length(x: i32, y: i32) -> i32 {
    i32_wrap_from_i64(ft_vector_length_long(i64::from(x), i64::from(y)))
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

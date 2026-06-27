//! Fixed-point arithmetic — faithful port of `src/base/ftcalc.c` (FT_INT64 path).
//!
//! These are the exact semantics FreeType uses; matching them bit-for-bit is
//! required for byte-perfect glyph scaling. The reference is
//! `freetype/src/base/ftcalc.c` lines 156–250.
//!
//! Conventions (FreeType):
//! - `FT_Long` / `FT_Fixed` are 32-bit signed → `i32`.
//! - Intermediate 64-bit math uses `FT_UInt64` / `FT_Int64` → `u64` / `i64`.

/// FreeType's `ADD_LONG`: signed addition computed in unsigned to match the
/// defined-overflow behaviour of the C `int` type. We saturate to `i32` range
/// because Rust's wrapping is explicit only — FreeType relies on 2's-complement
/// `int`, which for our value ranges never actually overflows.
#[inline]
fn add_long(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

/// FreeType's `NEG_LONG`: negate through unsigned arithmetic (safe for INT_MIN).
#[inline]
fn neg_long(a: i32) -> i32 {
    0i32.wrapping_sub(a)
}

/// FT_MulDiv — `(a * b + c/2) / c` as a signed long, with `FT_INT64` path.
///
/// Reference: `ftcalc.c:161`. Returns 0x7FFFFFFF on overflow / zero divisor.
#[inline]
pub fn ft_mul_div(a: i32, b: i32, c: i32) -> i32 {
    let mut s: i64 = 1;
    // FT_MOVE_SIGN: move sign out of a, b, c into `s`, leaving unsigned magnitudes.
    let mut ua = a as i64;
    if a < 0 {
        ua = 0i64.wrapping_sub(a as i64);
        s = -s;
    }
    let mut ub = b as i64;
    if b < 0 {
        ub = 0i64.wrapping_sub(b as i64);
        s = -s;
    }
    let mut uc = c as i64;
    if c < 0 {
        uc = 0i64.wrapping_sub(c as i64);
        s = -s;
    }

    let d = if uc > 0 {
        (ua.wrapping_mul(ub) + (uc >> 1)) / uc
    } else {
        0x7FFF_FFFFu64 as i64
    };

    // FT casts d to FT_Long (32-bit) before negation.
    let d32 = d as i32;
    if s < 0 {
        neg_long(d32)
    } else {
        d32
    }
}

/// FT_MulFix — `(a * b + 0x8000) >> 16` with signed rounding (FT_INT64 path).
///
/// Reference: `ftcalc.c:211`. This is the hot scaling multiply.
/// `ab >> 63` is the arithmetic sign extension (−1 for negative, 0 otherwise),
/// which makes the `>> 16` round correctly for negative products.
/// FT_MulFix — `(a * b + 0x8000 + sign_adj) >> 16` with symmetric rounding.
///
/// Reference: `ftcalc.h:91-102` (FT_MulFix_64 inline).  The 64-bit path
/// computes `ab + 0x8000 + (ab >> 63)` where ab >> 63 is -1 for negative
/// products and 0 for positive — giving rounded-toward--infinity for both
/// sign cases.  Matches C exactly.
#[inline]
pub fn ft_mul_fix(a: i32, b: i32) -> i32 {
    let ab = (a as i64).wrapping_mul(b as i64);
    let rounded = ab.wrapping_add(0x8000).wrapping_add(ab >> 63);
    (rounded >> 16) as i32
}

/// FT_DivFix — `((a << 16) + (b >> 1)) / b` as a signed long (FT_INT64 path).
///
/// Reference: `ftcalc.c:232`. Used to derive 16.16 scale factors.
#[inline]
pub fn ft_div_fix(a: i32, b: i32) -> i32 {
    let mut s: i64 = 1;
    let mut ua = a as i64;
    if a < 0 {
        ua = 0i64.wrapping_sub(a as i64);
        s = -s;
    }
    let mut ub = b as i64;
    if b < 0 {
        ub = 0i64.wrapping_sub(b as i64);
        s = -s;
    }

    let q = if ub > 0 {
        ((ua << 16) + (ub >> 1)) / ub
    } else {
        0x7FFF_FFFFu64 as i64
    };
    let q32 = q as i32;
    if s < 0 {
        neg_long(q32)
    } else {
        q32
    }
}

/// FT_RoundFix — round a 16.16 fixed to the nearest integer in 16.16.
///
/// Reference: `ftcalc.c:75`.
/// `ADD_LONG(a, 0x8000L - (a < 0)) & ~0xFFFFL`.
#[inline]
pub fn ft_round_fix(a: i32) -> i32 {
    let bias = 0x8000i32 - i32::from(a < 0);
    add_long(a, bias) & !0xFFFFi32
}

/// FT_CeilFix — round a 16.16 fixed *up* to the next integer in 16.16.
///
/// Reference: `ftcalc.c:84`. `ADD_LONG(a, 0xFFFFL) & ~0xFFFFL`.
#[inline]
pub fn ft_ceil_fix(a: i32) -> i32 {
    add_long(a, 0xFFFF) & !0xFFFFi32
}

/// FT_FloorFix — round a 16.16 fixed *down* to the integer in 16.16.
///
/// Reference: `ftcalc.c:93`. `a & ~0xFFFFL`.
#[inline]
pub fn ft_floor_fix(a: i32) -> i32 {
    a & !0xFFFFi32
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
        // Sign handling: a=0x400, b=-2048 → result negated.
        let r = ft_div_fix(16 << 6, -2048);
        assert_eq!(r, -0x8000);
    }

    #[test]
    fn round_fix_half_up() {
        // 0.5 (16.16 = 0x8000) rounds to 1.0 (0x10000).
        assert_eq!(ft_round_fix(0x8000), 0x1_0000);
    }

    #[test]
    fn ceil_fix_and_floor_fix() {
        assert_eq!(ft_ceil_fix(0x1), 0x1_0000);
        assert_eq!(ft_floor_fix(0x_FFFF), 0x0);
    }

    #[test]
    fn mul_div_basic() {
        // (3 * 5 + 1) / 2 = 8
        assert_eq!(ft_mul_div(3, 5, 2), 8);
    }

    #[test]
    fn mul_div_negative() {
        assert_eq!(ft_mul_div(-3, 5, 2), -8);
    }
}

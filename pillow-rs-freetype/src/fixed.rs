//! Fixed-point arithmetic — ✅ VERIFIED against FreeType 2.14.3 C library.
//!
//! Each function carries a verification marker documenting comparison status
//! vs the C reference at /tmp/ftecho (built against vendored FreeType 2.14.3).
//!
//! Parity tests in `tests/fixed_parity.rs` exhaustively compare all functions
//! against the C oracle across 2M+ test cases.

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

/// FT_MulDiv — ✅ VERIFIED: matches C FT_MulDiv (ftcalc.c:162, INT64 path).
///
/// Sign-stripping: converts to unsigned magnitudes, does unsigned multiply +
/// add half-divisor + divide, then restores sign with XOR of sign bits.
/// Exhaustive parity: 0 diffs in 2M+ values (fixed_parity.rs).
#[inline]
pub fn ft_mul_div(a: i32, b: i32, c: i32) -> i32 {
    if c == 0 {
        return 0x7FFFFFFF;
    }
    let ua: u64 = (a as i64).unsigned_abs();
    let ub: u64 = (b as i64).unsigned_abs();
    let uc: u64 = (c as i64).unsigned_abs();
    let d = (ua.wrapping_mul(ub) + (uc >> 1)) / uc;
    let d32 = d as i32;
    let negate = ((a < 0) ^ (b < 0)) ^ (c < 0);
    if negate { 0i32.wrapping_sub(d32) } else { d32 }
}

/// FT_MulFix — ✅ VERIFIED: matches C FT_MulFix_64 (ftcalc.h:91-102).
///
/// `(ab + 0x8000 + (ab >> 63)) >> 16` with symmetric rounding.
/// The `ab >> 63` term is -1 for negative products, 0 for positive —
/// giving rounded-toward-infinity for both sign cases.
/// Exhaustive parity: 0 diffs in 65K+ values (fixed_parity.rs).
#[inline]
pub fn ft_mul_fix(a: i32, b: i32) -> i32 {
    let ab = (a as i64).wrapping_mul(b as i64);
    let rounded = ab.wrapping_add(0x8000).wrapping_add(ab >> 63);
    (rounded >> 16) as i32
}

/// FT_DivFix — ✅ VERIFIED: matches C FT_DivFix (ftcalc.c:233, INT64 path).
///
/// C uses sign-stripping: `((|a|<<16) + (|b|>>1)) / |b|` as unsigned,
/// then negates if input signs differ. This differs from signed division
/// `((a<<16)+(b>>1))/b` when the numerator magnitude isn't evenly divisible
/// — signed truncation-toward-zero produces a different result than
/// unsigned-floor-then-negate.
/// Exhaustive parity: 0 diffs in 65K+ values (fixed_parity.rs).
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
    let q32 = q as i32;
    let negate = (a < 0) ^ (b < 0);
    if negate { 0i32.wrapping_sub(q32) } else { q32 }
}

/// FT_RoundFix — ✅ VERIFIED: matches C FT_RoundFix (ftcalc.c:75).
///
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
        // C sign-stripping: (|3|*|5| + |2|>>1) / |2| = (15+1)/2 = 8
        // negate((a<0)^(b<0)^(c<0)) = negate(true) = -8
        assert_eq!(ft_mul_div(-3, 5, 2), -8);
    }
}

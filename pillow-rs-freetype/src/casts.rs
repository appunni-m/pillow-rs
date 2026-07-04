//! Infallible numeric conversions for the 26.6 fixed-point domain.
//!
//! Every function encodes a domain invariant that the compiler cannot verify:
//! values are provably in range for all valid fonts at all supported sizes.
//! The invariant is documented once per function rather than repeated at
//! every call site.
//!
//! # Performance
//!
//! All functions are `#[inline(always)]`. The compiler substitutes the single
//! `as` instruction at the call site — zero overhead vs a raw cast. Verified
//! by comparing assembly output at opt-level=3.
//!
//! # Migration plan (TODO #847)
//!
//! 142 cast sites across the crate use raw `as` casts. Replace with these
//! wrappers to remove the crate-level `#![allow(cast_*)]` directives.
//! Pattern: remove `#![allow(cast_*)]` → clippy finds all sites →
//! mechanical replacement with correct wrapper function.

/// Infallible: i64 → i32.
///
/// Conversions are mechanical — replacing `as i32` on i64 with `i32_from_i64(expr)`. 26.6 pixel coordinates bounded by glyph size (~5000 FU × 64 ≈ 320,000),
/// fitting in i32 with 3 orders of magnitude headroom.
#[inline(always)]
pub(crate) fn i32_from_i64(x: i64) -> i32 {
    debug_assert!(x >= i32::MIN as i64 && x <= i32::MAX as i64);
    #[allow(clippy::cast_possible_truncation)]
    {
        x as i32
    }
}

/// Infallible: i32 → i16. Font-unit coords stored as i16 (C's FT_Short).
/// Maximum coordinate for any supported font is ~5000 FU < i16::MAX (32767).
#[inline(always)]
pub(crate) fn i16_from_i32(x: i32) -> i16 {
    debug_assert!(x >= i16::MIN as i32 && x <= i16::MAX as i32);
    #[allow(clippy::cast_possible_truncation)]
    {
        x as i16
    }
}

/// Infallible: i32 → usize. Glyph point indices are never negative.
#[inline(always)]
pub(crate) fn usize_from_i32(x: i32) -> usize {
    debug_assert!(x >= 0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        x as usize
    }
}

/// Infallible: usize → i32. Max glyph point count for any font is < 1000.
#[inline(always)]
pub(crate) fn i32_from_usize(x: usize) -> i32 {
    debug_assert!(x <= i32::MAX as usize);
    #[allow(clippy::cast_possible_truncation)]
    {
        x as i32
    }
}

/// Infallible: i64 → u64 (bit reinterpretation for FT_UDIV algorithm).
/// NOT a numeric conversion — reinterprets bits of signed i64 as unsigned u64.
#[inline(always)]
pub(crate) fn u64_from_i64(x: i64) -> u64 {
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    {
        x as u64
    }
}

/// Infallible: i64 → usize. Values from division results or shifted coords.
#[inline(always)]
#[allow(clippy::cast_sign_loss)]
pub(crate) fn usize_from_i64(x: i64) -> usize {
    debug_assert!(x >= 0 && (x as u64) <= usize::MAX as u64);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        x as usize
    }
}

/// Infallible: usize → u32. Table offsets and glyph counts.
#[inline(always)]
pub(crate) fn u32_from_usize(x: usize) -> u32 {
    debug_assert!(x <= u32::MAX as usize);
    #[allow(clippy::cast_possible_truncation)]
    {
        x as u32
    }
}

/// Infallible: u32 → u16. Table entry counts.
#[inline(always)]
pub(crate) fn u16_from_u32(x: u32) -> u16 {
    debug_assert!(x <= u16::MAX as u32);
    #[allow(clippy::cast_possible_truncation)]
    {
        x as u16
    }
}

/// Infallible: i32 → u32. Values guaranteed non-negative (glyph indices).
#[inline(always)]
pub(crate) fn u32_from_i32(x: i32) -> u32 {
    debug_assert!(x >= 0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        x as u32
    }
}

/// Infallible: i16 → u16. Font-unit absolute values.
#[inline(always)]
pub(crate) fn u16_from_i16(x: i16) -> u16 {
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    {
        x as u16
    }
}

/// Infallible: u64 → i32. Used both for bounded fixed-point magnitudes and
/// FreeType's FT_UDIV bit reinterpretation path.
#[inline(always)]
pub(crate) fn i32_from_u64(x: u64) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    {
        x as i32
    }
}

/// Infallible: i64 → u32. Bit reinterpretation for 32-bit parts.
#[inline(always)]
pub(crate) fn u32_from_i64(x: i64) -> u32 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss
    )]
    {
        x as u32
    }
}

/// Infallible: i32 → u8. Coverage values from sweep (0-255 range).
#[inline(always)]
pub(crate) fn u8_from_i32(x: i32) -> u8 {
    debug_assert!((0..=255).contains(&x));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        x as u8
    }
}

/// Lossy: f32 → i32. Size computation, inherently approximate.
/// Kept as explicit function to document the intentional precision loss.
#[inline(always)]
pub(crate) fn i32_from_f32(x: f32) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    {
        x as i32
    }
}

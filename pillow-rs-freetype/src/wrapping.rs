//! Wrapping arithmetic helpers for the 26.6 fixed-point domain.
//!
//! FreeType's C source uses 2's-complement integer arithmetic whose overflow
//! behavior is defined (unlike Rust's debug panics).  These wrappers give
//! the same 2's-complement result as C on both debug and release builds.
//!
//! Saturating variants are intentionally NOT provided — `i32::saturating_add`
//! clamps at i32::MAX, which differs from C's 2's-complement wrap for large
//! values.  When in doubt about domain bounds, use `#[allow]` at module level.
//!
//! # Naming
//!
//! Short names for readability at 700+ call sites: `add(a,b)`, `sub(a,b)`,
//! `mul(a,b)`, `neg(a)`.

#![allow(clippy::arithmetic_side_effects)]

/// 2's-complement i32 addition — matches C's `a + b`.
#[inline(always)]
pub(crate) fn add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

/// 2's-complement i32 subtraction — matches C's `a - b`.
#[inline(always)]
pub(crate) fn sub(a: i32, b: i32) -> i32 {
    a.wrapping_sub(b)
}

/// 2's-complement i32 multiplication — matches C's `a * b`.
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn mul(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}

/// 2's-complement i32 negation — matches C's `-a`.
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn neg(a: i32) -> i32 {
    a.wrapping_neg()
}

//! SIMD-accelerated operation implementations.
//!
//! The portable scalar implementation is the reference and fallback path.
//!
//! `scalar` is always compiled as the reference + fallback.

pub(crate) mod adapters;
pub(crate) mod native_channel;
pub(crate) mod native_filter;
pub(crate) mod native_merge;

#[inline(always)]
fn mode_has_green_blue(mode: u32) -> bool {
    mode >= 2
}

#[inline(always)]
fn mode_has_alpha(mode: u32) -> bool {
    mode == 1 || mode == 3
}

#[inline(always)]
fn mode_is_rgba(mode: u32) -> bool {
    mode == 3
}

// Packed scalar helpers are retained for adapters that are not admitted by
// `simd_supports_for_image` yet. The contextual gate routes those public
// inputs to CPU before execution; they are not a hidden SIMD fallback. Keep
// the module available while each unsupported family is replaced by a native
// vector kernel or removed.
#[allow(dead_code)]
mod scalar;

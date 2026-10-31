//! Shared vector helpers for straight RGBA8 Convert strategies.

pub(crate) mod opaque_padding;
pub(crate) mod premultiply;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2"),
    all(target_arch = "aarch64", target_feature = "neon")
))]
pub(crate) mod luma_alpha;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2"),
    all(target_arch = "aarch64", target_feature = "neon")
))]
pub(crate) mod vector;

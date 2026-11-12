//! SSE2-compiled portable-vector palette remapping.

pub(crate) mod remap_palette_impl;

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
))]
pub(crate) mod convert_impl;

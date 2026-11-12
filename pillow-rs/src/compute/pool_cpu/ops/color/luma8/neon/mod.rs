//! NEON-compiled portable-vector palette remapping.

pub(crate) mod remap_palette_impl;

#[cfg(target_arch = "aarch64")]
pub(crate) mod convert_impl;

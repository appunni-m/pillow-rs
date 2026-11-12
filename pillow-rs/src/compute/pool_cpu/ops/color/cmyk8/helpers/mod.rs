//! Native CMYK Convert helpers.

#[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "wasm32"))]
pub(crate) mod to_rgb;

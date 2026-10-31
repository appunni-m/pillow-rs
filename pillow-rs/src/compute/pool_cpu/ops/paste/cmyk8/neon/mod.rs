#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod cmyk_row;

pub(crate) mod paste_impl;

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod la_row;

pub(crate) mod paste_impl;

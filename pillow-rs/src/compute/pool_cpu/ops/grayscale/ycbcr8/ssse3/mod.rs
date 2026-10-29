#[cfg(target_arch = "x86_64")]
pub(crate) mod grayscale_impl;
#[cfg(all(target_arch = "x86", target_feature = "ssse3"))]
pub(crate) mod grayscale_wide_impl;

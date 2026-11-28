#[cfg(target_arch = "x86_64")]
pub(crate) mod resize_impl;
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx",
    not(target_feature = "fma")
))]
pub(crate) mod vector_resize_impl;

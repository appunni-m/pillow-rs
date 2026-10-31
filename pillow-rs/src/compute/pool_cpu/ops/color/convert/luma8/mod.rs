pub(crate) mod default;
pub(crate) mod helpers;
#[cfg(target_arch = "aarch64")]
pub(crate) mod neon;
pub(crate) mod ops;
#[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "sse2")))]
pub(crate) mod sse2;

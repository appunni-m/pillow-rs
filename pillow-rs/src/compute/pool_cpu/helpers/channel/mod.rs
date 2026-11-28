pub(crate) mod rgba;

pub(crate) mod gather;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon;
#[cfg(target_arch = "x86_64")]
pub(crate) mod ssse3;
pub(crate) mod wide;

#[cfg(target_arch = "x86_64")]
mod avx2;
#[cfg(target_arch = "x86_64")]
mod avx512f;
mod default;
mod helpers;
#[cfg(target_arch = "aarch64")]
mod neon;
pub(crate) mod ops;

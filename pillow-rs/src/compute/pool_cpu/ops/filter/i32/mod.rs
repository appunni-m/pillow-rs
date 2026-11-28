#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx;
pub(crate) mod avx2;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx512f;
mod default;
mod helpers;
pub(crate) mod ops;
#[cfg(target_arch = "x86_64")]
mod x86_64;

pub(crate) mod ops;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx;
pub(crate) mod avx2;
mod default;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx512f;
mod default;
mod helpers;
pub(crate) mod ops;

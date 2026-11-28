#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx512f;
pub(crate) mod default;
pub(crate) mod ops;

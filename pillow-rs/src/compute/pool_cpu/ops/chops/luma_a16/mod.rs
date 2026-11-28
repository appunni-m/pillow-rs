#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(super) mod avx512f;
pub(super) mod default;
pub(super) mod ops;

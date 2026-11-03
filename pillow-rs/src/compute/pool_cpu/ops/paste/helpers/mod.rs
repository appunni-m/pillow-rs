pub(crate) mod bytes;
pub(crate) mod contracts;
pub(crate) mod i16;
pub(crate) mod i16_vector;
pub(crate) mod layout;
pub(crate) mod plan;
pub(crate) mod region;
pub(crate) mod strategy;
pub(crate) mod typed;
pub(crate) mod vector;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod kernels;

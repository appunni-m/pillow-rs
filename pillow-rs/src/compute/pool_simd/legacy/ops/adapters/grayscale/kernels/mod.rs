//! Mode-specific grayscale kernels; orchestration remains in the parent adapter.

use super::super::simd_pack_u16x16;

mod channels;
mod cmyk;
mod rgb_avx2;
mod ycbcr;
#[cfg(target_arch = "x86_64")]
mod ycbcr_ssse3;

pub(in crate::compute::pool_simd::ops::adapters) use self::channels::native_grayscale_bytes;
pub(in crate::compute::pool_simd::ops::adapters) use self::cmyk::native_cmyk_grayscale_bytes;
pub(in crate::compute::pool_simd::ops::adapters) use self::ycbcr::{
    native_ycbcr_grayscale_bytes, native_ycbcr_grayscale_path,
};

//! Transpose kernel modules, including architecture-specific specializations.

pub(super) mod portable;
#[cfg(target_arch = "x86_64")]
pub(super) mod rgba_avx2;

pub(super) use self::portable::transpose_gathered_row;
pub(in crate::compute::pool_simd::ops::adapters) use self::portable::transpose_native_tiled;
#[cfg(all(test, feature = "parallel"))]
pub(in crate::compute::pool_simd::ops::adapters) use self::portable::{
    TransposeNativeCollectCounters, TransposeNativeOddCollectCounters, TransposeNativeOddRowCache,
    TransposeNativeRowCache,
};
#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) use self::portable::{
    transpose_native_collect, transpose_native_collect_admitted, transpose_native_odd_collect,
    transpose_native_odd_collect_admitted,
};

#[cfg(target_arch = "x86_64")]
#[inline]
pub(super) fn transpose_rgba(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    reverse_x: bool,
    reverse_y: bool,
) -> Option<(u64, u64)> {
    rgba_avx2::transpose_rgba(source, output, width, height, reverse_x, reverse_y)
}

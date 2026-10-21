mod byte_layouts;
mod l;
mod la;
#[cfg(not(feature = "parallel"))]
mod rgb_blur_ring;
mod uniform_rgb;

pub(super) use self::byte_layouts::native_filter_5x5_rows;
pub(super) use self::l::{
    native_filter_5x5_binomial_luma_rows, native_filter_5x5_uniform_luma_rows,
};
pub(super) use self::la::native_filter_5x5_single_outlier_la_rows;
pub(super) use crate::compute::pool_simd::ops::native_filter::native_filter_5x5_binomial_la_rows;
pub(super) use crate::compute::pool_simd::ops::native_filter::native_filter_5x5_i32_rows;

pub(super) const SMALL_IMAGE_LIMIT_PIXELS: usize = uniform_rgb::MAX_SMALL_PIXELS;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ScanEvidence {
    pub(super) is_uniform: bool,
    pub(super) vector_blocks: u64,
    pub(super) scalar_tail_pixels: u64,
}

pub(super) fn scan_uniform_rgb(raw: &[u8], width: usize, height: usize) -> Option<ScanEvidence> {
    let evidence = uniform_rgb::scan_uniform_rgb(raw, width, height)?;
    Some(ScanEvidence {
        is_uniform: evidence.is_uniform,
        vector_blocks: evidence.vector_blocks,
        scalar_tail_pixels: evidence.scalar_tail_pixels,
    })
}

#[cfg(not(feature = "parallel"))]
pub(super) fn apply_rgb_blur_ring(
    raw: &[u8],
    width: usize,
    height: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    rgb_blur_ring::apply(raw, width, height)
}

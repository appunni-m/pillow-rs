//! Architecture-specific Transpose kernels.

pub(super) mod rgba_avx2;

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

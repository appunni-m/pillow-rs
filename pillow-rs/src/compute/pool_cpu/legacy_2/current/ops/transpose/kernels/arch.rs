//! Architecture-specific transpose orchestration.

use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;

#[cfg(all(target_arch = "x86_64", not(feature = "parallel")))]
pub(super) fn try_rotate270_rgba_tiled(
    image: &DynamicImage,
    method: &TransposeMethod,
    output: &mut [u8],
    width: u32,
    height: u32,
    channels: usize,
) -> bool {
    if channels != 4
        || !matches!(method, TransposeMethod::Rotate270)
        || !super::should_tile_transpose(width, height)
        || width % 4 != 0
        || height % 4 != 0
    {
        return false;
    }
    super::transpose_rgba_x86_64::rotate270_tiled(image.as_bytes(), output, width, height);
    true
}

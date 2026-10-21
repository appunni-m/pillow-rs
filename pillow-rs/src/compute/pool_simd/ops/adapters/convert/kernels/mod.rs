//! Architecture-specific native layout conversion kernels.

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod rgba_rgb_neon;
#[cfg(target_arch = "x86_64")]
mod rgba_rgb_ssse3;

use crate::raster::DynamicImage;

/// Convert RGBA bytes to RGB with the architecture kernel selected at build
/// time and runtime feature support checked by that kernel.
pub(in crate::compute::pool_simd::ops::adapters) fn native_rgba_to_rgb_bytes(
    img: &DynamicImage,
) -> Option<(Vec<u8>, u64, u64, &'static str)> {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    if let Some((output, vector_blocks, scalar_tail)) = rgba_rgb_neon::native_rgba_to_rgb_bytes(img)
    {
        return Some((output, vector_blocks, scalar_tail, "neon-rgba-to-rgb"));
    }

    #[cfg(target_arch = "x86_64")]
    if let Some((output, vector_blocks, scalar_tail)) =
        rgba_rgb_ssse3::native_rgba_to_rgb_bytes(img)
    {
        return Some((output, vector_blocks, scalar_tail, "ssse3-rgba-to-rgb"));
    }

    let _ = img;
    None
}

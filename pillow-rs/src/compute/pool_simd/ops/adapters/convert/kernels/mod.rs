//! SIMD kernels for native color conversion and architecture-specific paths.

mod color_space;
mod dispatch;
mod layout;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod rgba_rgb_neon;
#[cfg(target_arch = "x86_64")]
mod rgba_rgb_ssse3;

use self::color_space::{
    native_convert_cmyk_bytes, native_convert_hsv_bytes, native_convert_ycbcr_bytes,
};
pub(in crate::compute::pool_simd::ops::adapters) use self::dispatch::native_convert_bytes;
#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) use self::layout::SIMD_RGB_TO_RGBA_PARALLEL_PIXEL_THRESHOLD;
pub(in crate::compute::pool_simd::ops::adapters) use self::layout::{
    native_convert_layout_block, native_luma_to_rgb_bytes, native_luma_to_rgba_bytes,
    native_rgb_to_rgba_bytes, native_rgba_color_degenerate,
};
use self::layout::{native_luma_alpha_to_rgba_bytes, native_rgba_to_la_bytes};

use crate::raster::DynamicImage;
use crate::compute::{Architecture, ExecutionContext};

/// Convert RGBA bytes to RGB with the architecture kernel selected at build
/// time and runtime feature support checked by that kernel.
pub(in crate::compute::pool_simd::ops::adapters) fn native_rgba_to_rgb_bytes(
    context: &ExecutionContext,
    img: &DynamicImage,
) -> Option<(Vec<u8>, u64, u64, &'static str)> {
    match context.architecture() {
        Architecture::Neon => {
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            {
                return rgba_rgb_neon::native_rgba_to_rgb_bytes(img)
                    .map(|(output, blocks, tail)| (output, blocks, tail, "neon-rgba-to-rgb"));
            }
            #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
            None
        }
        Architecture::Ssse3 | Architecture::Avx2 => {
            #[cfg(target_arch = "x86_64")]
            {
                return rgba_rgb_ssse3::native_rgba_to_rgb_bytes(context, img)
                    .map(|(output, blocks, tail)| (output, blocks, tail, "ssse3-rgba-to-rgb"));
            }
            #[cfg(not(target_arch = "x86_64"))]
            None
        }
        Architecture::Scalar
        | Architecture::PortableSimd
        | Architecture::Wgpu
        | Architecture::Cuda
        | Architecture::TileLang => None,
    }
}

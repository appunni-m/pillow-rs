//! SSSE3 RGBX-to-Luma8 and RGB Convert routes.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "ssse3",
    not(target_feature = "avx2")
))]
pub(crate) fn execute_luma(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let kernel = crate::compute::pool_cpu::ops::grayscale::rgbx8::default::grayscale_impl::prepare(
        source,
    )?;
    let output = crate::compute::pool_cpu::ops::grayscale::rgbx8::ssse3::grayscale_impl::execute(
        kernel,
    );
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        output,
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}

#[cfg(all(target_arch = "x86_64", target_feature = "ssse3", not(target_feature = "avx2")))]
/// Converts the native RGBX layout to RGB by dropping its padding byte.
///
/// # Safety
///
/// The caller must use the SSSE3 strategy selected for this source/target.
pub(crate) unsafe fn execute_rgb(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    // SAFETY: the strategy is selected only when SSSE3 is available.
    let output = unsafe {
        crate::compute::pool_cpu::ops::color::convert::rgba8::ssse3::convert_impl::execute(source)?
    };
    Ok(DynamicImage::ImageRgb8(output))
}

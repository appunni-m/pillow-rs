//! CPU implementation for Pillow's 5x5 convolution filter.

use super::uniform_image::{SMALL_UNIFORM_BYTE_MAX_PIXELS, native_small_uniform_byte_image};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::DynamicImage;

#[path = "filter5x5/int32.rs"]
mod int32;
#[path = "filter5x5/native_bytes.rs"]
mod native_bytes;
use int32::filter_5x5_i32;

#[cfg(not(feature = "parallel"))]
#[path = "rgb_blur_ring.rs"]
mod rgb_blur_ring;
#[cfg(not(feature = "parallel"))]
#[path = "uniform_box.rs"]
mod uniform_box;
#[cfg(all(test, not(feature = "parallel")))]
#[path = "uniform_box_tests.rs"]
mod uniform_box_tests;

pub(crate) use int32::uniform_i32_filter5x5_is_identity;
#[cfg(all(test, not(feature = "parallel")))]
pub(crate) use native_bytes::filter_5x5_byte_row;
pub(crate) use native_bytes::la_single_outlier5x5_parameters_match;
#[cfg(not(feature = "parallel"))]
pub(crate) use native_bytes::rgb_blur_ring5x5_parameters_match;
pub(crate) use native_bytes::{binomial5x5_parameters_match, uniform5x5_parameters_match};
#[cfg(feature = "parallel")]
use native_bytes::{filter_5x5_binomial_la_parallel_rows, filter_5x5_binomial_luma_parallel_rows};
#[cfg(not(feature = "parallel"))]
use native_bytes::{
    filter_5x5_binomial_la_rows, filter_5x5_binomial_luma_rows, filter_5x5_la_single_outlier_rows,
    filter_5x5_uniform_rgba_identity, filter_5x5_uniform_rgba_rows,
};
use native_bytes::{filter_5x5_byte_rows, filter_5x5_constant_bytes, filter_5x5_uniform_luma_rows};

/// Execute a 5x5 convolution filter.
pub fn execute_filter5x5(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // I-mode: operate directly on int32 pixel values (no [0,255] clipping)
    if explicit_mode == Some("I") {
        return filter_5x5_i32(img, kernel, scale, offset);
    }
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (w_u32, h_u32) = (img.width(), img.height());
    let (w, h) = (w_u32 as i32, h_u32 as i32);
    // Normalize once, outside the pixel loop. The row helper keeps all five
    // tap groups in the same order as the original scalar implementation.
    let normalized_kernel: [f32; 25] = std::array::from_fn(|index| kernel[index] / scale);
    let rounding_bias = offset + 0.5;
    #[cfg(not(feature = "parallel"))]
    if matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(explicit_mode, None | Some("RGB"))
        && rgb_blur_ring::parameters_match(kernel, scale, offset)
        && let Some(out) = rgb_blur_ring::apply(raw, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(img, DynamicImage::ImageRgba8(_))
        && matches!(explicit_mode, None | Some("RGBA"))
        && uniform5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_uniform_rgba_identity(
            raw,
            w_u32 as usize,
            h_u32 as usize,
            &normalized_kernel,
            rounding_bias,
        )
    {
        let result = raw_bytes_to_image(w_u32, h_u32, raw.to_vec(), channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(feature = "parallel")]
    if matches!(explicit_mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && binomial5x5_parameters_match(kernel, scale, offset)
    {
        if let Some(out) =
            filter_5x5_binomial_luma_parallel_rows(raw, w_u32 as usize, h_u32 as usize)
        {
            let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
            return Ok(preserve_mode(img, result));
        }
    }
    #[cfg(feature = "parallel")]
    if matches!(explicit_mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && binomial5x5_parameters_match(kernel, scale, offset)
    {
        if let Some(out) = filter_5x5_binomial_la_parallel_rows(raw, w_u32 as usize, h_u32 as usize)
        {
            let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
            return Ok(preserve_mode(img, result));
        }
    }
    let mut out = raw.to_vec();
    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && la_single_outlier5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_la_single_outlier_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(img, DynamicImage::ImageRgba8(_))
        && matches!(explicit_mode, None | Some("RGBA"))
        && uniform5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_uniform_rgba_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    let native_byte_layout = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_layout
        && native_small_uniform_byte_image(img, channels, SMALL_UNIFORM_BYTE_MAX_PIXELS)
    {
        filter_5x5_constant_bytes(
            raw,
            &mut out,
            w_u32 as usize,
            h_u32 as usize,
            channels,
            &normalized_kernel,
            rounding_bias,
        );
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    if matches!(explicit_mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && uniform5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_uniform_luma_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && binomial5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_binomial_luma_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(not(feature = "parallel"))]
    #[cfg(target_arch = "aarch64")]
    #[cfg(target_feature = "neon")]
    if matches!(img, DynamicImage::ImageLumaA8(_))
        && w_u32 >= 13
        && h_u32 >= 5
        && binomial5x5_parameters_match(kernel, scale, offset)
    {
        // Reuse the exact packed-LA vector kernel for serial CPU on AArch64.
        // Keep this out of Parallel CPU so Rayon remains a separate profile.
        let mut out = raw.to_vec();
        if let Some((vector_blocks, scalar_tail)) =
            crate::compute::pool_simd::ops::native_filter::native_filter_5x5_binomial_la_rows(
                raw,
                &mut out,
                w_u32 as usize,
                h_u32 as usize,
            )
        {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
            let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
            return Ok(preserve_mode(img, result));
        }
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && binomial5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_binomial_la_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    filter_5x5_byte_rows(
        raw,
        &mut out,
        w,
        h,
        channels,
        &normalized_kernel,
        rounding_bias,
    );
    let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
    Ok(preserve_mode(img, result))
}

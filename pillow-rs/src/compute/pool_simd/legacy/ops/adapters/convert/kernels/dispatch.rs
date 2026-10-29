//! Native SIMD conversion dispatch and vector arithmetic.

mod layout;
mod luma;
mod typed;

use super::super::{NativeConvertLayout, native_convert_pixels_per_vector};
use super::{
    native_convert_cmyk_bytes, native_convert_hsv_bytes, native_convert_ycbcr_bytes,
    native_luma_alpha_to_rgba_bytes, native_luma_to_rgb_bytes, native_luma_to_rgba_bytes,
    native_rgba_to_la_bytes,
};
use crate::raster::DynamicImage;

pub(in crate::compute::pool_simd::ops::adapters) fn native_convert_bytes(
    img: &DynamicImage,
    layout: NativeConvertLayout,
) -> Option<(Vec<u8>, u64, u64)> {
    if layout.target_is_cmyk {
        return native_convert_cmyk_bytes(img, layout);
    }
    if layout.target_is_hsv {
        return native_convert_hsv_bytes(img, layout);
    }
    if layout.target_is_ycbcr {
        return native_convert_ycbcr_bytes(img, layout);
    }
    if layout.source_is_luma && layout.source_channels == 1 && layout.target_channels == 3 {
        return native_luma_to_rgb_bytes(img);
    }
    // I/F also occupy four bytes, but each pixel is one typed scalar rather
    // than four color samples. Keep their widening separate from RGBA.
    if layout.source_is_luma
        && layout.source_channels == 1
        && layout.target_channels == 4
        && !layout.target_is_integer
        && !layout.target_is_float
    {
        return native_luma_to_rgba_bytes(img);
    }
    if layout.source_is_luma
        && layout.source_channels == 2
        && layout.target_channels == 4
        && !layout.target_is_integer
        && !layout.target_is_float
    {
        return native_luma_alpha_to_rgba_bytes(img);
    }
    if !layout.source_is_luma
        && layout.source_channels == 4
        && layout.target_channels == 2
        && !layout.source_is_rgbx
    {
        return native_rgba_to_la_bytes(img);
    }

    let source = img.as_bytes();
    let pixel_count = (img.width() as usize).checked_mul(img.height() as usize)?;
    let source_len = pixel_count.checked_mul(layout.source_channels)?;
    let output_len = pixel_count.checked_mul(layout.target_channels)?;
    if source.len() != source_len {
        return None;
    }

    let mut output = Vec::with_capacity(output_len);
    let pixels_per_vector = native_convert_pixels_per_vector(layout);
    let vector_blocks = if layout.target_is_integer {
        typed::append_integer_pixels(source, pixel_count, pixels_per_vector, layout, &mut output)
    } else if layout.target_is_float {
        typed::append_float_pixels(source, pixel_count, pixels_per_vector, layout, &mut output)
    } else if !layout.source_is_luma && layout.target_is_luma {
        luma::append_rgb_luma_pixels(source, pixel_count, layout, &mut output)
    } else {
        layout::append_layout_pixels(source, pixel_count, pixels_per_vector, layout, &mut output)?
    };

    Some((output, vector_blocks, 0))
}

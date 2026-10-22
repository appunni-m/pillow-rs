//! Native SIMD conversion dispatch and vector arithmetic.

use super::super::{NativeConvertLayout, native_convert_pixels_per_vector};
use super::{
    native_convert_cmyk_bytes, native_convert_hsv_bytes, native_convert_layout_block,
    native_convert_ycbcr_bytes, native_luma_alpha_to_rgba_bytes, native_luma_to_rgb_bytes,
    native_luma_to_rgba_bytes, native_rgba_to_la_bytes,
};
use crate::raster::DynamicImage;
use wide::{f32x8, u8x16, u32x8};

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
    // than four color samples. Keep their widening below separate from RGBA.
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
    let expected_source_bytes = pixel_count.checked_mul(layout.source_channels)?;
    let expected_output_bytes = pixel_count.checked_mul(layout.target_channels)?;
    if source.len() != expected_source_bytes {
        return None;
    }
    let mut output = Vec::with_capacity(expected_output_bytes);
    let pixels_per_vector = native_convert_pixels_per_vector(layout);
    let mut vector_blocks = 0u64;

    if layout.target_is_integer {
        for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
            let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
            let red = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels]
                } else {
                    0
                }
            });
            let values = if layout.source_is_luma {
                u32x8::new(red.map(u32::from))
                    .to_array()
                    .map(|value| value as i32)
            } else {
                let green = std::array::from_fn(|lane| {
                    if lane < active_pixels {
                        source[(start_pixel + lane) * layout.source_channels + 1]
                    } else {
                        0
                    }
                });
                let blue = std::array::from_fn(|lane| {
                    if lane < active_pixels {
                        source[(start_pixel + lane) * layout.source_channels + 2]
                    } else {
                        0
                    }
                });
                (u32x8::new(red.map(u32::from)) * u32x8::splat(19595)
                    + u32x8::new(green.map(u32::from)) * u32x8::splat(38470)
                    + u32x8::new(blue.map(u32::from)) * u32x8::splat(7471)
                    + u32x8::splat(32768))
                .to_array()
                .map(|value| (value >> 16) as i32)
            };
            for value in values.into_iter().take(active_pixels) {
                output.extend_from_slice(&value.to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    } else if layout.target_is_float {
        for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
            let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
            let red = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels]
                } else {
                    0
                }
            });
            let values = if layout.source_is_luma {
                f32x8::new(red.map(f32::from)).to_array()
            } else {
                let green = std::array::from_fn(|lane| {
                    if lane < active_pixels {
                        source[(start_pixel + lane) * layout.source_channels + 1]
                    } else {
                        0
                    }
                });
                let blue = std::array::from_fn(|lane| {
                    if lane < active_pixels {
                        source[(start_pixel + lane) * layout.source_channels + 2]
                    } else {
                        0
                    }
                });
                let sum = u32x8::new(red.map(u32::from)) * u32x8::splat(299)
                    + u32x8::new(green.map(u32::from)) * u32x8::splat(587)
                    + u32x8::new(blue.map(u32::from)) * u32x8::splat(114);
                (f32x8::new(sum.to_array().map(|value| value as f32)) / f32x8::splat(1000.0))
                    .to_array()
            };
            for value in values.into_iter().take(active_pixels) {
                output.extend_from_slice(&value.to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }

    if !layout.target_is_integer
        && !layout.target_is_float
        && !layout.source_is_luma
        && layout.target_is_luma
    {
        // RGB/RGBA to L/LA: the byte gather is scalar because `wide` has no
        // portable byte-gather instruction, but the complete luma arithmetic
        // runs in eight u32 lanes. Alpha is only gathered for the LA result.
        for start_pixel in (0..pixel_count).step_by(8) {
            let active_pixels = (pixel_count - start_pixel).min(8);
            let red = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels]
                } else {
                    0
                }
            });
            let green = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels + 1]
                } else {
                    0
                }
            });
            let blue = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels + 2]
                } else {
                    0
                }
            });
            let luma = (u32x8::new(red.map(u32::from)) * u32x8::splat(19595)
                + u32x8::new(green.map(u32::from)) * u32x8::splat(38470)
                + u32x8::new(blue.map(u32::from)) * u32x8::splat(7471)
                + u32x8::splat(32768))
                >> 16u32;
            let gray = luma.to_array().map(|value| value.min(255) as u8);
            if layout.target_channels == 1 {
                let mut packed = [0u8; 16];
                packed[..8].copy_from_slice(&gray);
                output.extend_from_slice(&u8x16::new(packed).to_array()[..active_pixels]);
            } else {
                let alpha: [u8; 8] = std::array::from_fn(|lane| {
                    if lane < active_pixels && layout.source_channels == 4 && !layout.source_is_rgbx
                    {
                        source[(start_pixel + lane) * layout.source_channels + 3]
                    } else {
                        u8::MAX
                    }
                });
                let mut gray_lanes = [0u8; 16];
                gray_lanes[..8].copy_from_slice(&gray);
                let mut alpha_lanes = [0u8; 16];
                alpha_lanes[..8].copy_from_slice(&alpha);
                let duplicate = u8x16::new([0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7]);
                let even = u8x16::new([
                    u8::MAX,
                    0,
                    u8::MAX,
                    0,
                    u8::MAX,
                    0,
                    u8::MAX,
                    0,
                    u8::MAX,
                    0,
                    u8::MAX,
                    0,
                    u8::MAX,
                    0,
                    u8::MAX,
                    0,
                ]);
                let odd = u8x16::splat(u8::MAX) ^ even;
                let interleaved = (u8x16::new(gray_lanes).swizzle_relaxed(duplicate) & even)
                    | (u8x16::new(alpha_lanes).swizzle_relaxed(duplicate) & odd);
                output.extend_from_slice(&interleaved.to_array()[..active_pixels * 2]);
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    } else if !layout.target_is_integer && !layout.target_is_float {
        for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
            let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
            let source_start = start_pixel * layout.source_channels;
            let input_len = active_pixels * layout.source_channels;
            let mut padded = [0u8; 16];
            padded[..input_len].copy_from_slice(&source[source_start..source_start + input_len]);
            let block = native_convert_layout_block(&padded, layout, pixels_per_vector)?;
            let output_len = active_pixels * layout.target_channels;
            output.extend_from_slice(&block[..output_len]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Some((output, vector_blocks, 0))
}

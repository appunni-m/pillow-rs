//! Typed integer and floating-point conversion lanes.

use super::super::super::NativeConvertLayout;
use wide::{f32x8, u32x8};

pub(super) fn append_integer_pixels(
    source: &[u8],
    pixel_count: usize,
    pixels_per_vector: usize,
    layout: NativeConvertLayout,
    output: &mut Vec<u8>,
) -> u64 {
    let mut vector_blocks = 0u64;
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
    vector_blocks
}

pub(super) fn append_float_pixels(
    source: &[u8],
    pixel_count: usize,
    pixels_per_vector: usize,
    layout: NativeConvertLayout,
    output: &mut Vec<u8>,
) -> u64 {
    let mut vector_blocks = 0u64;
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
            (f32x8::new(sum.to_array().map(|value| value as f32)) / f32x8::splat(1000.0)).to_array()
        };
        for value in values.into_iter().take(active_pixels) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    vector_blocks
}

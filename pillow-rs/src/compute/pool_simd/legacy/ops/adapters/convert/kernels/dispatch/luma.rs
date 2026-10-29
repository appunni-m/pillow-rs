//! RGB and RGBA byte conversion to native L and LA output.

use super::super::super::NativeConvertLayout;
use wide::{u8x16, u32x8};

pub(super) fn append_rgb_luma_pixels(
    source: &[u8],
    pixel_count: usize,
    layout: NativeConvertLayout,
    output: &mut Vec<u8>,
) -> u64 {
    let mut vector_blocks = 0u64;
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
                if lane < active_pixels && layout.source_channels == 4 && !layout.source_is_rgbx {
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
    vector_blocks
}

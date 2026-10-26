//! Native SIMD kernels for CMYK, YCbCr, and HSV conversion.

use super::super::NativeConvertLayout;
use crate::raster::DynamicImage;
use std::sync::OnceLock;
use wide::{f32x8, f64x8, i32x8, u8x16};

type NativeYcbcrTables = (
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
);

/// Convert native byte luma/RGB-family samples to Pillow's four-byte CMYK
/// representation. Convert.c's RGB inverse is C=255-R, M=255-G, Y=255-B,
/// K=0; its luma branch is C=M=Y=0, K=255-L. Keep the input and output in
/// their native interleaved layouts and vectorize the per-channel subtraction.
pub(super) fn native_convert_cmyk_bytes(
    img: &DynamicImage,
    layout: NativeConvertLayout,
) -> Option<(Vec<u8>, u64, u64)> {
    let source = img.as_bytes();
    let pixel_count = (img.width() as usize).checked_mul(img.height() as usize)?;
    let expected_source_bytes = pixel_count.checked_mul(layout.source_channels)?;
    let expected_output_bytes = pixel_count.checked_mul(4)?;
    if source.len() != expected_source_bytes {
        return None;
    }

    let pixels_per_vector = 4usize;
    let mut output = Vec::with_capacity(expected_output_bytes);
    let mut vector_blocks = 0u64;
    for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
        let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
        let mut first = [0u8; 16];
        let mut second = [0u8; 16];
        let mut third = [0u8; 16];
        for lane in 0..pixels_per_vector {
            if lane < active_pixels {
                let source_start = (start_pixel + lane) * layout.source_channels;
                first[lane] = source[source_start];
                if !layout.source_is_luma {
                    second[lane] = source[source_start + 1];
                    third[lane] = source[source_start + 2];
                }
            }
        }
        let first = (u8x16::splat(255) - u8x16::new(first)).to_array();
        let second = (u8x16::splat(255) - u8x16::new(second)).to_array();
        let third = (u8x16::splat(255) - u8x16::new(third)).to_array();
        for lane in 0..active_pixels {
            let source_start = (start_pixel + lane) * layout.source_channels;
            let k = if layout.source_is_luma {
                255u8.saturating_sub(source[source_start])
            } else {
                0
            };
            let pixel = if layout.source_is_luma {
                [0, 0, 0, k]
            } else {
                [first[lane], second[lane], third[lane], k]
            };
            output.extend_from_slice(&pixel);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}

fn native_ycbcr_table(coeff: f64) -> [i32; 256] {
    let mut table = [0i32; 256];
    for (value, output) in table.iter_mut().enumerate() {
        *output = (value as f64 * coeff * 64.0 + 0.5) as i32;
    }
    table
}

fn native_ycbcr_tables() -> NativeYcbcrTables {
    static Y_R: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_G: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_R: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_B: OnceLock<[i32; 256]> = OnceLock::new();

    (
        Y_R.get_or_init(|| native_ycbcr_table(0.299)),
        Y_G.get_or_init(|| native_ycbcr_table(0.587)),
        Y_B.get_or_init(|| native_ycbcr_table(0.114)),
        CB_R.get_or_init(|| native_ycbcr_table(-0.16874)),
        CB_G.get_or_init(|| native_ycbcr_table(-0.33126)),
        CB_B.get_or_init(|| native_ycbcr_table(0.5)),
        CR_G.get_or_init(|| native_ycbcr_table(-0.41869)),
        CR_B.get_or_init(|| native_ycbcr_table(-0.08131)),
    )
}

/// Convert native luma/RGB-family bytes to Pillow's YCbCr storage. The table
/// gathers are scalar because portable `wide` has no byte-gather primitive;
/// the fixed-point additions, shifts, and per-lane result arithmetic remain
/// vector operations. Luma sources use Pillow's direct neutral-chroma path.
pub(super) fn native_convert_ycbcr_bytes(
    img: &DynamicImage,
    layout: NativeConvertLayout,
) -> Option<(Vec<u8>, u64, u64)> {
    if !layout.target_is_ycbcr || layout.target_channels != 3 {
        return None;
    }
    let source = img.as_bytes();
    let pixel_count = (img.width() as usize).checked_mul(img.height() as usize)?;
    let expected_source_bytes = pixel_count.checked_mul(layout.source_channels)?;
    let expected_output_bytes = pixel_count.checked_mul(3)?;
    if source.len() != expected_source_bytes {
        return None;
    }

    let pixels_per_vector = 8usize;
    let mut output = Vec::with_capacity(expected_output_bytes);
    let mut vector_blocks = 0u64;
    let (y_r, y_g, y_b, cb_r, cb_g, cb_b, cr_g, cr_b) = native_ycbcr_tables();

    for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
        let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
        let red_values = std::array::from_fn(|lane| {
            if lane < active_pixels {
                source[(start_pixel + lane) * layout.source_channels]
            } else {
                0
            }
        });
        let (y, cb, cr) = if layout.source_is_luma {
            let gray = i32x8::new(red_values.map(i32::from));
            (gray, i32x8::splat(128), i32x8::splat(128))
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
            let red = i32x8::new(red_values.map(|value| y_r[usize::from(value)]));
            let green_y = i32x8::new(green.map(|value| y_g[usize::from(value)]));
            let blue_y = i32x8::new(blue.map(|value| y_b[usize::from(value)]));
            let y = (red + green_y + blue_y) >> 6u32;

            let red_cb = i32x8::new(std::array::from_fn(|lane| {
                cb_r[usize::from(red_values[lane])]
            }));
            let green_cb = i32x8::new(green.map(|value| cb_g[usize::from(value)]));
            let blue_cb = i32x8::new(blue.map(|value| cb_b[usize::from(value)]));
            let cb = ((red_cb + green_cb + blue_cb) >> 6u32) + i32x8::splat(128);

            let red_cr = i32x8::new(std::array::from_fn(|lane| {
                cb_b[usize::from(red_values[lane])]
            }));
            let green_cr = i32x8::new(green.map(|value| cr_g[usize::from(value)]));
            let blue_cr = i32x8::new(blue.map(|value| cr_b[usize::from(value)]));
            let cr = ((red_cr + green_cr + blue_cr) >> 6u32) + i32x8::splat(128);
            (y, cb, cr)
        };
        let y = y.to_array().map(|value| value.clamp(0, 255) as u8);
        let cb = cb.to_array().map(|value| value.clamp(0, 255) as u8);
        let cr = cr.to_array().map(|value| value.clamp(0, 255) as u8);
        for lane in 0..active_pixels {
            output.extend_from_slice(&[y[lane], cb[lane], cr[lane]]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}

/// Convert native luma/RGB-family bytes to Pillow's HSV storage. The max/min
/// and ratio arithmetic uses wide floating-point lanes; per-lane max-channel
/// selection is scalar control, matching Pillow's tie-breaking order exactly.
pub(super) fn native_convert_hsv_bytes(
    img: &DynamicImage,
    layout: NativeConvertLayout,
) -> Option<(Vec<u8>, u64, u64)> {
    if !layout.target_is_hsv || layout.target_channels != 3 {
        return None;
    }
    let source = img.as_bytes();
    let pixel_count = (img.width() as usize).checked_mul(img.height() as usize)?;
    let expected_source_bytes = pixel_count.checked_mul(layout.source_channels)?;
    let expected_output_bytes = pixel_count.checked_mul(3)?;
    if source.len() != expected_source_bytes {
        return None;
    }

    let pixels_per_vector = 8usize;
    let mut output = Vec::with_capacity(expected_output_bytes);
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
        let max_bytes = if layout.source_is_luma {
            red
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
            let red_vector = f32x8::new(red.map(f32::from));
            let green_vector = f32x8::new(green.map(f32::from));
            let blue_vector = f32x8::new(blue.map(f32::from));
            let max_vector = red_vector.max(green_vector.max(blue_vector));
            let min_vector = red_vector.min(green_vector.min(blue_vector));
            let delta_vector = max_vector - min_vector;
            let safe_delta = delta_vector
                .simd_eq(f32x8::splat(0.0))
                .select(f32x8::splat(1.0), delta_vector);
            let rc = (max_vector - red_vector) / safe_delta;
            let gc = (max_vector - green_vector) / safe_delta;
            let bc = (max_vector - blue_vector) / safe_delta;

            // PIL applies the sector constants in double precision after the
            // f32 ratios have been computed. Build all three candidates in
            // f64 lanes, then select the candidate per lane below.
            let red_h =
                f64x8::new(bc.to_array().map(f64::from)) - f64x8::new(gc.to_array().map(f64::from));
            let green_h = f64x8::splat(2.0) + f64x8::new(rc.to_array().map(f64::from))
                - f64x8::new(bc.to_array().map(f64::from));
            let blue_h = f64x8::splat(4.0) + f64x8::new(gc.to_array().map(f64::from))
                - f64x8::new(rc.to_array().map(f64::from));
            let red_h = red_h.to_array();
            let green_h = green_h.to_array();
            let blue_h = blue_h.to_array();
            let saturation = (delta_vector
                / max_vector
                    .simd_eq(f32x8::splat(0.0))
                    .select(f32x8::splat(1.0), max_vector))
            .to_array();
            let max_values = max_vector.to_array().map(|value| value as u8);
            let delta_values = delta_vector.to_array();
            let red_values = red;
            let green_values = green;
            let selected_h = std::array::from_fn(|lane| {
                if red_values[lane] == max_values[lane] {
                    red_h[lane]
                } else if green_values[lane] == max_values[lane] {
                    green_h[lane]
                } else {
                    blue_h[lane]
                }
            });
            // Pillow stores the sector result in a C `float` before applying
            // the hue wrap. Preserve that narrowing step per lane before the
            // vectorized f64 divide/add; otherwise values such as 0.8 become
            // 0.799999997 in the wider lanes and truncate one HSV byte low.
            let selected_h = selected_h.map(|value| (value as f32) as f64);
            // fmod(x, 1.0) is equivalent here to subtracting floor(x): the
            // selected hue is bounded by Pillow's RGB sector construction.
            let hue = f64x8::new(selected_h) / f64x8::splat(6.0) + f64x8::splat(1.0);
            let hue = (hue - hue.floor()).to_array();
            for lane in 0..active_pixels {
                if delta_values[lane] == 0.0 {
                    output.extend_from_slice(&[0, 0, max_values[lane]]);
                    continue;
                }
                let h_stored = hue[lane] as f32;
                let hue_byte = (f64::from(h_stored) * 255.0) as u8;
                let saturation_byte = (f64::from(saturation[lane]) * 255.0) as u8;
                output.extend_from_slice(&[hue_byte, saturation_byte, max_values[lane]]);
            }
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        };

        // L/LA -> HSV is Pillow's RGB normalization followed by the gray
        // branch: H=0, S=0, V=L. The lane construction above is unnecessary
        // for this mode, but the block still uses one vectorized byte group.
        for value in max_bytes.into_iter().take(active_pixels) {
            output.extend_from_slice(&[0, 0, value]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}

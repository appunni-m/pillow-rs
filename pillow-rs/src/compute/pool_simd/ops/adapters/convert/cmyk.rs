//! Native SIMD CMYK projection into Pillow RGB bytes.

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use super::super::{native_channel, simd_pack_u16x16};
use crate::raster::DynamicImage;
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use wide::{u8x16, u16x16};

#[allow(unsafe_code)]
pub(super) fn native_cmyk_to_rgb_bytes(img: &DynamicImage) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if source.as_raw().len() != pixels.checked_mul(4)? {
        return None;
    }
    let output_capacity = pixels.checked_mul(3)?;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        native_cmyk_to_rgb_neon(source, pixels, output_capacity)
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    {
        native_cmyk_to_rgb_wide(source, output_capacity)
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
fn native_cmyk_to_rgb_neon(
    source: &crate::raster::RgbaImage,
    pixels: usize,
    output_capacity: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    use core::arch::aarch64 as neon;

    #[inline(always)]
    fn muldiv255_neon(value: neon::uint8x16_t, ink: neon::uint8x16_t) -> neon::uint8x16_t {
        // SAFETY: this helper is called only inside the AArch64 NEON
        // specialization selected for a target compiled with NEON.
        unsafe {
            let round = |product: neon::uint16x8_t| {
                let value = neon::vaddq_u16(product, neon::vdupq_n_u16(128));
                neon::vshrq_n_u16(neon::vaddq_u16(neon::vshrq_n_u16(value, 8), value), 8)
            };
            let low = round(neon::vmull_u8(
                neon::vget_low_u8(value),
                neon::vget_low_u8(ink),
            ));
            let high = round(neon::vmull_u8(
                neon::vget_high_u8(value),
                neon::vget_high_u8(ink),
            ));
            neon::vcombine_u8(neon::vqmovn_u16(low), neon::vqmovn_u16(high))
        }
    }

    let mut output = vec![0u8; output_capacity];
    let vector_pixels = pixels / 16 * 16;
    for pixel_start in (0..vector_pixels).step_by(16) {
        let source_start = pixel_start * 4;
        let output_start = pixel_start * 3;
        // SAFETY: the checked four-byte source length leaves 64 readable
        // bytes for every 16-pixel block; the output allocation leaves
        // 48 writable bytes at the matching three-byte destination.
        let (red, green, blue) = unsafe {
            let samples = neon::vld4q_u8(source.as_raw().as_ptr().add(source_start));
            let (cyan, magenta, yellow, black) = (samples.0, samples.1, samples.2, samples.3);
            let ink = neon::vsubq_u8(neon::vdupq_n_u8(255), black);
            let red = neon::vsubq_u8(ink, muldiv255_neon(cyan, ink));
            let green = neon::vsubq_u8(ink, muldiv255_neon(magenta, ink));
            let blue = neon::vsubq_u8(ink, muldiv255_neon(yellow, ink));
            (red, green, blue)
        };
        let mut red_lanes = [0u8; 16];
        let mut green_lanes = [0u8; 16];
        let mut blue_lanes = [0u8; 16];
        // SAFETY: each local lane array has exactly sixteen writable
        // bytes, matching the vector store width.
        unsafe {
            neon::vst1q_u8(red_lanes.as_mut_ptr(), red);
            neon::vst1q_u8(green_lanes.as_mut_ptr(), green);
            neon::vst1q_u8(blue_lanes.as_mut_ptr(), blue);
        }
        for (lane, rgb) in output[output_start..output_start + 48]
            .chunks_exact_mut(3)
            .enumerate()
        {
            rgb[0] = red_lanes[lane];
            rgb[1] = green_lanes[lane];
            rgb[2] = blue_lanes[lane];
        }
    }
    for pixel in vector_pixels..pixels {
        let source_start = pixel * 4;
        let output_start = pixel * 3;
        let cmyk = &source.as_raw()[source_start..source_start + 4];
        let ink = 255u32 - u32::from(cmyk[3]);
        output[output_start] = (ink - crate::color::muldiv255(u32::from(cmyk[0]), ink)) as u8;
        output[output_start + 1] = (ink - crate::color::muldiv255(u32::from(cmyk[1]), ink)) as u8;
        output[output_start + 2] = (ink - crate::color::muldiv255(u32::from(cmyk[2]), ink)) as u8;
    }
    Some((
        output,
        (vector_pixels / 16) as u64,
        (pixels - vector_pixels) as u64,
    ))
}

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
fn native_cmyk_to_rgb_wide(
    source: &crate::raster::RgbaImage,
    output_capacity: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    let mut output = Vec::with_capacity(output_capacity);
    let mut vector_blocks = 0u64;
    #[cfg(target_arch = "x86_64")]
    let ssse3_available = std::is_x86_feature_detected!("ssse3");
    for source_block in source.as_raw().chunks(64) {
        let active_pixels = source_block.len() / 4;
        #[cfg(target_arch = "x86_64")]
        let channels = if active_pixels == 16 && ssse3_available {
            let full_block: &[u8; 64] = source_block.try_into().ok()?;
            // SAFETY: the complete block is exactly 64 bytes and the
            // runtime check above establishes SSSE3 support.
            Some(unsafe { native_channel::deinterleave_cmyk_16_ssse3(full_block).map(u8x16::new) })
        } else {
            None
        };
        #[cfg(not(target_arch = "x86_64"))]
        let channels: Option<[u8x16; 4]> = None;
        let (c, m, y, k) = if let Some([c, m, y, k]) = channels {
            (c, m, y, k)
        } else {
            let mut padded = [0u8; 64];
            padded[..source_block.len()].copy_from_slice(source_block);
            let blocks = [
                u8x16::new(padded[0..16].try_into().ok()?),
                u8x16::new(padded[16..32].try_into().ok()?),
                u8x16::new(padded[32..48].try_into().ok()?),
                u8x16::new(padded[48..64].try_into().ok()?),
            ];
            (
                native_channel::gather_interleaved_channel::<4, 0>(&blocks),
                native_channel::gather_interleaved_channel::<4, 1>(&blocks),
                native_channel::gather_interleaved_channel::<4, 2>(&blocks),
                native_channel::gather_interleaved_channel::<4, 3>(&blocks),
            )
        };
        let c = u16x16::from(c);
        let m = u16x16::from(m);
        let y = u16x16::from(y);
        let k = u16x16::from(k);
        let ink = u16x16::splat(255) - k;
        let muldiv255 = |value: u16x16| {
            let value = value + u16x16::splat(128);
            ((value >> 8u32) + value) >> 8u32
        };
        let red = simd_pack_u16x16(ink - muldiv255(c * ink)).to_array();
        let green = simd_pack_u16x16(ink - muldiv255(m * ink)).to_array();
        let blue = simd_pack_u16x16(ink - muldiv255(y * ink)).to_array();
        #[cfg(target_arch = "x86_64")]
        if active_pixels == 16 && ssse3_available {
            // SAFETY: the runtime feature check above establishes SSSE3;
            // all three lane arrays contain sixteen initialized samples.
            let packed = unsafe {
                super::super::super::native_merge::rgb16_interleave_ssse3(&red, &green, &blue)
            };
            output.extend_from_slice(&packed);
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        }
        for pixel in 0..active_pixels {
            output.extend_from_slice(&[red[pixel], green[pixel], blue[pixel]]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    (output.len() == output_capacity).then_some((output, vector_blocks, 0))
}

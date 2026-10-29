//! Native SIMD CMYK projection into Pillow RGB bytes.

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use super::super::{native_channel, simd_pack_u16x16};
use crate::raster::DynamicImage;
use crate::compute::{Architecture, ExecutionContext};
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use fearless_simd::prelude::{SimdBase, SimdInterleaved, SimdNarrow, SimdWiden};
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use fearless_simd::{Level, Neon, u8x16 as SafeU8x16, u16x8 as SafeU16x8};
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use wide::{u8x16, u16x16};

pub(super) fn native_cmyk_to_rgb_bytes(
    context: &ExecutionContext,
    img: &DynamicImage,
) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if source.as_raw().len() != pixels.checked_mul(4)? {
        return None;
    }
    let output_capacity = pixels.checked_mul(3)?;
    match context.architecture() {
        Architecture::Neon => {
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            {
                native_cmyk_to_rgb_neon(source, pixels, output_capacity)
            }
            #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
            {
                None
            }
        }
        Architecture::Avx2 | Architecture::Ssse3 => {
            #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
            {
                native_cmyk_to_rgb_wide(source, output_capacity, true)
            }
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            {
                None
            }
        }
        Architecture::PortableSimd => {
            #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
            {
                native_cmyk_to_rgb_wide(source, output_capacity, false)
            }
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            {
                None
            }
        }
        Architecture::Scalar
        | Architecture::Wgpu
        | Architecture::Cuda
        | Architecture::TileLang => None,
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn native_cmyk_to_rgb_neon(
    source: &crate::raster::RgbaImage,
    pixels: usize,
    output_capacity: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    let neon = Level::new()
        .as_neon()
        .expect("AArch64 NEON target configuration has a NEON execution token");
    let vector_pixels = pixels / 16 * 16;
    let vector_source_len = vector_pixels.checked_mul(4)?;
    let vector_output_len = vector_pixels.checked_mul(3)?;
    let mut output = vec![0u8; output_capacity];
    native_cmyk_to_rgb_neon_kernel(
        neon,
        &source.as_raw()[..vector_source_len],
        &mut output[..vector_output_len],
    );
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

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline(always)]
fn muldiv255_neon(neon: Neon, value: SafeU8x16<Neon>, ink: SafeU8x16<Neon>) -> SafeU8x16<Neon> {
    let (value_low, value_high) = value.widen();
    let (ink_low, ink_high) = ink.widen();
    let round = |product: SafeU16x8<Neon>| {
        let value = product + SafeU16x8::<Neon>::splat(neon, 128);
        ((value >> 8u32) + value) >> 8u32
    };
    round(value_low * ink_low).narrow(round(value_high * ink_high))
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fearless_simd::kernel!(
    #[inline(always)]
    fn native_cmyk_to_rgb_neon_kernel(neon: Neon, source: &[u8], output: &mut [u8]) {
        let (source_blocks, source_tail) = source.as_chunks::<64>();
        let (output_blocks, output_tail) = output.as_chunks_mut::<48>();
        debug_assert!(source_tail.is_empty());
        debug_assert!(output_tail.is_empty());
        for (source_block, output_block) in source_blocks.iter().zip(output_blocks) {
            let channels = SafeU8x16::<Neon>::load_four_interleaved(neon, source_block);
            let ink = SafeU8x16::<Neon>::splat(neon, 255) - channels[3];
            let red = ink - muldiv255_neon(neon, channels[0], ink);
            let green = ink - muldiv255_neon(neon, channels[1], ink);
            let blue = ink - muldiv255_neon(neon, channels[2], ink);
            let mut red_lanes = [0; 16];
            let mut green_lanes = [0; 16];
            let mut blue_lanes = [0; 16];
            red.store_array(&mut red_lanes);
            green.store_array(&mut green_lanes);
            blue.store_array(&mut blue_lanes);
            for (lane, rgb) in output_block.chunks_exact_mut(3).enumerate() {
                rgb[0] = red_lanes[lane];
                rgb[1] = green_lanes[lane];
                rgb[2] = blue_lanes[lane];
            }
        }
    }
);

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
#[allow(unsafe_code)] // SSSE3 calls require the context-checked full-block path below.
fn native_cmyk_to_rgb_wide(
    source: &crate::raster::RgbaImage,
    output_capacity: usize,
    use_ssse3: bool,
) -> Option<(Vec<u8>, u64, u64)> {
    let mut output = Vec::with_capacity(output_capacity);
    let mut vector_blocks = 0u64;
    for source_block in source.as_raw().chunks(64) {
        let active_pixels = source_block.len() / 4;
        #[cfg(target_arch = "x86_64")]
        let channels = if active_pixels == 16 && use_ssse3 {
            let full_block: &[u8; 64] = source_block.try_into().ok()?;
            // SAFETY: the complete block is exactly 64 bytes and the
            // ExecutionContext established SSSE3 support before selecting this kernel.
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

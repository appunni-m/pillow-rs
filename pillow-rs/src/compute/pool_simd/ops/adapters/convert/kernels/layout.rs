//! Native SIMD kernels for byte-layout conversion.

use super::super::super::{native_channel, simd_pack_u16x16};
use super::super::NativeConvertLayout;
use crate::raster::DynamicImage;
use wide::{u8x16, u16x16};

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_convert_layout_block(
    input: &[u8],
    layout: NativeConvertLayout,
    pixels: usize,
) -> Option<[u8; 16]> {
    let input_len = pixels.checked_mul(layout.source_channels)?;
    let output_len = pixels.checked_mul(layout.target_channels)?;
    if input.len() < input_len || input_len > 16 || output_len > 16 {
        return None;
    }

    let mut padded = [0u8; 16];
    padded[..input_len].copy_from_slice(&input[..input_len]);
    let opaque_index = 15usize;
    if (layout.target_channels == 2 && layout.source_is_luma && layout.source_channels == 1)
        || (layout.target_channels == 4
            && ((layout.source_is_luma && layout.source_channels == 1)
                || (!layout.source_is_luma && layout.source_channels == 3)
                || layout.source_is_rgbx))
    {
        // Every block needing an inserted alpha has fewer than sixteen input
        // bytes, so lane 15 is outside the source payload.
        padded[opaque_index] = u8::MAX;
    }

    let mut indices = [0u8; 16];
    for pixel in 0..pixels {
        for channel in 0..layout.target_channels {
            let source_index = if layout.source_is_luma {
                if channel == 0 || (layout.target_channels == 3 && channel < 3) {
                    pixel * layout.source_channels
                } else if channel == 3 || (layout.target_channels == 2 && channel == 1) {
                    if layout.source_channels == 2 {
                        pixel * layout.source_channels + 1
                    } else {
                        opaque_index
                    }
                } else {
                    pixel * layout.source_channels
                }
            } else if channel < 3 {
                pixel * layout.source_channels + channel
            } else if layout.source_channels == 4 && !layout.source_is_rgbx {
                pixel * layout.source_channels + 3
            } else {
                opaque_index
            };
            indices[pixel * layout.target_channels + channel] = u8::try_from(source_index).ok()?;
        }
    }
    Some(
        u8x16::new(padded)
            .swizzle_relaxed(u8x16::new(indices))
            .to_array(),
    )
}

const NATIVE_LUMA_TO_RGB_BYTES_0: u8x16 =
    u8x16::new([0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5]);
const NATIVE_LUMA_TO_RGB_BYTES_1: u8x16 =
    u8x16::new([5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10]);
const NATIVE_LUMA_TO_RGB_BYTES_2: u8x16 = u8x16::new([
    10, 11, 11, 11, 12, 12, 12, 13, 13, 13, 14, 14, 14, 15, 15, 15,
]);
const NATIVE_LUMA_TO_RGBA_BYTES_0: u8x16 =
    u8x16::new([0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3]);
const NATIVE_LUMA_TO_RGBA_BYTES_1: u8x16 =
    u8x16::new([4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7]);
const NATIVE_LUMA_TO_RGBA_BYTES_2: u8x16 =
    u8x16::new([8, 8, 8, 8, 9, 9, 9, 9, 10, 10, 10, 10, 11, 11, 11, 11]);
const NATIVE_LUMA_TO_RGBA_BYTES_3: u8x16 = u8x16::new([
    12, 12, 12, 12, 13, 13, 13, 13, 14, 14, 14, 14, 15, 15, 15, 15,
]);
const NATIVE_RGBA_ALPHA_LANES: u8x16 =
    u8x16::new([0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
const NATIVE_RGBA_COLOR_LANES: u8x16 = u8x16::new([
    255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0,
]);
const NATIVE_LA_TO_RGBA_BYTES_0: u8x16 =
    u8x16::new([0, 0, 0, 1, 2, 2, 2, 3, 4, 4, 4, 5, 6, 6, 6, 7]);
const NATIVE_LA_TO_RGBA_BYTES_1: u8x16 =
    u8x16::new([8, 8, 8, 9, 10, 10, 10, 11, 12, 12, 12, 13, 14, 14, 14, 15]);
const NATIVE_LA_DUPLICATE_0: u8x16 = u8x16::new([0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7]);
const NATIVE_LA_DUPLICATE_1: u8x16 =
    u8x16::new([8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15]);
const NATIVE_LA_LUMA_LANES: u8x16 = u8x16::new([
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
const NATIVE_LA_ALPHA_LANES: u8x16 = u8x16::new([
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
    u8::MAX,
]);
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
const NATIVE_RGB_TO_RGBA_BYTES: u8x16 =
    u8x16::new([0, 1, 2, 15, 3, 4, 5, 15, 6, 7, 8, 15, 9, 10, 11, 15]);

/// Expand native L samples to packed RGB with fixed 16-pixel byte shuffles.
/// The three vectors concatenate into 48 consecutive output bytes, avoiding
/// the generic layout path's per-block shuffle-index construction.
pub(in crate::compute::pool_simd::ops::adapters) fn native_luma_to_rgb_bytes(
    img: &DynamicImage,
) -> Option<(Vec<u8>, u64, u64)> {
    if !matches!(img, DynamicImage::ImageLuma8(_)) {
        return None;
    }

    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let output_bytes = pixels.checked_mul(3)?;
    let source = img.as_bytes();
    if source.len() != pixels {
        return None;
    }

    let mut blocks = Vec::<[[u8; 16]; 3]>::with_capacity(pixels.div_ceil(16));
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(16) {
        let active_pixels = (pixels - start).min(16);
        let mut lanes = [0u8; 16];
        lanes[..active_pixels].copy_from_slice(&source[start..start + active_pixels]);
        let input = u8x16::new(lanes);
        let bytes_0 = input.swizzle_relaxed(NATIVE_LUMA_TO_RGB_BYTES_0).to_array();
        let bytes_1 = input.swizzle_relaxed(NATIVE_LUMA_TO_RGB_BYTES_1).to_array();
        let bytes_2 = input.swizzle_relaxed(NATIVE_LUMA_TO_RGB_BYTES_2).to_array();
        blocks.push([bytes_0, bytes_1, bytes_2]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    // The three byte vectors form one contiguous 48-byte RGB block. Flatten
    // their nested arrays without copying, then discard only the padded tail.
    let mut output = blocks.into_flattened().into_flattened();
    output.truncate(output_bytes);
    Some((output, vector_blocks, 0))
}

/// Expand native L samples to packed RGBA with four fixed 16-pixel byte
/// shuffles. Alpha lanes are filled with 255; the input remains one byte per
/// pixel until this output kernel writes the required four-byte destination.
pub(in crate::compute::pool_simd::ops::adapters) fn native_luma_to_rgba_bytes(
    img: &DynamicImage,
) -> Option<(Vec<u8>, u64, u64)> {
    if !matches!(img, DynamicImage::ImageLuma8(_)) {
        return None;
    }

    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let output_bytes = pixels.checked_mul(4)?;
    let source = img.as_bytes();
    if source.len() != pixels {
        return None;
    }

    let mut blocks = Vec::<[[u8; 16]; 4]>::with_capacity(pixels.div_ceil(16));
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(16) {
        let active_pixels = (pixels - start).min(16);
        let mut lanes = [0u8; 16];
        lanes[..active_pixels].copy_from_slice(&source[start..start + active_pixels]);
        let input = u8x16::new(lanes);
        let bytes = [
            input
                .swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_0)
                .to_array(),
            input
                .swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_1)
                .to_array(),
            input
                .swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_2)
                .to_array(),
            input
                .swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_3)
                .to_array(),
        ];
        blocks.push(bytes.map(|bytes| {
            ((u8x16::new(bytes) & NATIVE_RGBA_COLOR_LANES) | NATIVE_RGBA_ALPHA_LANES).to_array()
        }));
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let mut output = blocks.into_flattened().into_flattened();
    output.truncate(output_bytes);
    Some((output, vector_blocks, 0))
}

/// Expand native LA pixels to RGBA with fixed four-pixel shuffles.
///
/// ImageEnhance.Color retains an LA-derived base in the source mode. For
/// RGBA, each LA pixel becomes `[L, L, L, A]`; processing four pixels per
/// block used to rebuild a sixteen-byte shuffle index for every block. These
/// constants express that mapping directly and cover sixteen pixels with four
/// fixed vector shuffles while keeping both input and output in their native
/// formats.
pub(super) fn native_luma_alpha_to_rgba_bytes(img: &DynamicImage) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageLumaA8(source) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if source.as_raw().len() != pixels.checked_mul(2)? {
        return None;
    }
    let output_bytes = pixels.checked_mul(4)?;
    let mut output = Vec::with_capacity(output_bytes);
    let source_blocks = source.as_raw().chunks(32);
    for source_block in source_blocks {
        let active_pixels = (pixels - output.len() / 4).min(16);
        let mut padded = [0u8; 32];
        padded[..source_block.len()].copy_from_slice(source_block);
        let low = u8x16::new(padded[..16].try_into().ok()?);
        let high = u8x16::new(padded[16..].try_into().ok()?);
        let vectors = [
            low.swizzle_relaxed(NATIVE_LA_TO_RGBA_BYTES_0).to_array(),
            low.swizzle_relaxed(NATIVE_LA_TO_RGBA_BYTES_1).to_array(),
            high.swizzle_relaxed(NATIVE_LA_TO_RGBA_BYTES_0).to_array(),
            high.swizzle_relaxed(NATIVE_LA_TO_RGBA_BYTES_1).to_array(),
        ];
        for (block_index, vector) in vectors.into_iter().enumerate() {
            let block_pixels = active_pixels.saturating_sub(block_index * 4).min(4);
            output.extend_from_slice(&vector[..block_pixels * 4]);
        }
    }
    Some((output, pixels.div_ceil(4) as u64, 0))
}

/// Convert RGBA pixels to Pillow's LA layout with sixteen-lane grayscale and
/// alpha deinterleaving. The generic converter gathered each color channel
/// scalarly for every eight-pixel block; this path shuffles four loaded byte
/// vectors, computes the exact rounded BT.601 luma, and interleaves eight LA
/// pixels per output vector.
pub(super) fn native_rgba_to_la_bytes(img: &DynamicImage) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let output_bytes = pixels.checked_mul(2)?;
    if source.as_raw().len() != pixels.checked_mul(4)? {
        return None;
    }
    let mut output = Vec::with_capacity(output_bytes);
    for source_block in source.as_raw().chunks(64) {
        let active_pixels = source_block.len() / 4;
        let mut padded = [0u8; 64];
        padded[..source_block.len()].copy_from_slice(source_block);
        let load = |offset| {
            u8x16::new(
                padded[offset..offset + 16]
                    .try_into()
                    .expect("complete RGBA-to-LA vector"),
            )
        };
        let blocks = [load(0), load(16), load(32), load(48)];
        let red = u16x16::from(native_channel::gather_interleaved_channel::<4, 0>(&blocks));
        let green = u16x16::from(native_channel::gather_interleaved_channel::<4, 1>(&blocks));
        let blue = u16x16::from(native_channel::gather_interleaved_channel::<4, 2>(&blocks));
        let base = red * const { u16x16::splat(77) }
            + green * const { u16x16::splat(150) }
            + blue * const { u16x16::splat(29) };
        let residual = green * const { u16x16::splat(70) } + blue * const { u16x16::splat(47) }
            - red * const { u16x16::splat(117) }
            + const { u16x16::splat(32768) };
        let luma = u8x16::new(simd_pack_u16x16((base + (residual >> 8u32)) >> 8u32).to_array());
        let alpha = native_channel::gather_interleaved_channel::<4, 3>(&blocks);
        let la_vectors = [NATIVE_LA_DUPLICATE_0, NATIVE_LA_DUPLICATE_1].map(|indices| {
            ((luma.swizzle_relaxed(indices) & NATIVE_LA_LUMA_LANES)
                | (alpha.swizzle_relaxed(indices) & NATIVE_LA_ALPHA_LANES))
                .to_array()
        });
        let first_pixels = active_pixels.min(8);
        output.extend_from_slice(&la_vectors[0][..first_pixels * 2]);
        let second_pixels = active_pixels.saturating_sub(8).min(8);
        output.extend_from_slice(&la_vectors[1][..second_pixels * 2]);
    }
    Some((output, pixels.div_ceil(8) as u64, 0))
}

/// Build Color's RGBA degenerate snapshot directly as `[L, L, L, A]`.
/// Reuse the exact native RGBA-to-LA luma calculation, but shuffle the
/// sixteen luma and alpha lanes straight into the final RGBA vectors instead
/// of allocating and rereading an intermediate LA image.
pub(in crate::compute::pool_simd::ops::adapters) fn native_rgba_color_degenerate(
    img: &DynamicImage,
) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let output_bytes = pixels.checked_mul(4)?;
    if source.as_raw().len() != output_bytes {
        return None;
    }

    let mut output = Vec::with_capacity(output_bytes);
    for source_block in source.as_raw().chunks(64) {
        let active_pixels = source_block.len() / 4;
        let mut padded = [0u8; 64];
        padded[..source_block.len()].copy_from_slice(source_block);
        let load = |offset| {
            u8x16::new(
                padded[offset..offset + 16]
                    .try_into()
                    .expect("complete RGBA Color-degenerate vector"),
            )
        };
        let blocks = [load(0), load(16), load(32), load(48)];
        let red = u16x16::from(native_channel::gather_interleaved_channel::<4, 0>(&blocks));
        let green = u16x16::from(native_channel::gather_interleaved_channel::<4, 1>(&blocks));
        let blue = u16x16::from(native_channel::gather_interleaved_channel::<4, 2>(&blocks));
        let base = red * const { u16x16::splat(77) }
            + green * const { u16x16::splat(150) }
            + blue * const { u16x16::splat(29) };
        let residual = green * const { u16x16::splat(70) } + blue * const { u16x16::splat(47) }
            - red * const { u16x16::splat(117) }
            + const { u16x16::splat(32768) };
        let luma = u8x16::new(simd_pack_u16x16((base + (residual >> 8u32)) >> 8u32).to_array());
        let alpha = native_channel::gather_interleaved_channel::<4, 3>(&blocks);
        let output_vectors = [
            ((luma.swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_0) & NATIVE_RGBA_COLOR_LANES)
                | (alpha.swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_0) & NATIVE_RGBA_ALPHA_LANES))
                .to_array(),
            ((luma.swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_1) & NATIVE_RGBA_COLOR_LANES)
                | (alpha.swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_1) & NATIVE_RGBA_ALPHA_LANES))
                .to_array(),
            ((luma.swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_2) & NATIVE_RGBA_COLOR_LANES)
                | (alpha.swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_2) & NATIVE_RGBA_ALPHA_LANES))
                .to_array(),
            ((luma.swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_3) & NATIVE_RGBA_COLOR_LANES)
                | (alpha.swizzle_relaxed(NATIVE_LUMA_TO_RGBA_BYTES_3) & NATIVE_RGBA_ALPHA_LANES))
                .to_array(),
        ];
        for (block_index, vector) in output_vectors.into_iter().enumerate() {
            let block_pixels = active_pixels.saturating_sub(block_index * 4).min(4);
            output.extend_from_slice(&vector[..block_pixels * 4]);
        }
    }
    let image =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4).ok()?;
    Some((image, pixels.div_ceil(4) as u64, 0))
}

#[inline]
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
fn native_rgb_to_rgba_block(source: &[u8], start_pixel: usize, active_pixels: usize) -> [u8; 16] {
    debug_assert!((1..=4).contains(&active_pixels));
    let source_start = start_pixel * 3;
    let active_bytes = active_pixels * 3;
    let mut input = [0u8; 16];
    input[..active_bytes].copy_from_slice(&source[source_start..source_start + active_bytes]);
    input[15] = u8::MAX;
    u8x16::new(input)
        .swizzle_relaxed(NATIVE_RGB_TO_RGBA_BYTES)
        .to_array()
}

/// Expand native RGB pixels with one NEON structure load/store per sixteen
/// pixels, or one portable vector shuffle per four pixels. The conversion
/// writes directly into its final allocation: collecting fixed-size blocks and
/// flattening them afterward copied the entire expanded image a second time.
#[allow(unsafe_code)]
pub(in crate::compute::pool_simd::ops::adapters) fn native_rgb_to_rgba_bytes(
    img: &DynamicImage,
) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageRgb8(rgb) = img else {
        return None;
    };

    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let source_bytes = pixels.checked_mul(3)?;
    let output_bytes = pixels.checked_mul(4)?;
    let source = rgb.as_raw();
    if source.len() != source_bytes {
        return None;
    }
    if output_bytes > isize::MAX as usize {
        return None;
    }

    let mut output = Vec::new();
    output.try_reserve_exact(output_bytes).ok()?;

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let vector_pixels = pixels / 16 * 16;
        let vectors = unsafe { native_rgb_to_rgba_neon(source, &mut output, vector_pixels) };
        for pixel in vector_pixels..pixels {
            let source_offset = pixel * 3;
            output.extend_from_slice(&source[source_offset..source_offset + 3]);
            output.push(u8::MAX);
        }
        Some((
            output,
            u64::try_from(vectors).ok()?,
            u64::try_from(pixels - vector_pixels).ok()?,
        ))
    }

    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    {
        let vector_pixels = pixels / 4 * 4;
        for start_pixel in (0..vector_pixels).step_by(4) {
            let block = native_rgb_to_rgba_block(source, start_pixel, 4);
            output.extend_from_slice(&block);
        }
        for pixel in vector_pixels..pixels {
            let source_offset = pixel * 3;
            output.extend_from_slice(&source[source_offset..source_offset + 3]);
            output.push(u8::MAX);
        }
        Some((
            output,
            u64::try_from(vector_pixels / 4).ok()?,
            u64::try_from(pixels - vector_pixels).ok()?,
        ))
    }
}

/// Interleave sixteen packed RGB pixels with an opaque alpha vector directly
/// into the spare capacity of `output`.
///
/// # Safety
/// The caller passes a multiple of sixteen no larger than the validated source
/// pixel count, and `source` contains at least `pixels * 3` initialized bytes.
/// `output` must be empty and have capacity for at least `pixels * 4` bytes. The
/// source and destination allocations must be distinct. Every iteration reads
/// exactly 48 source bytes and writes exactly 64 bytes in the reserved output;
/// this function sets the output length only after it initializes the complete
/// vector prefix.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
unsafe fn native_rgb_to_rgba_neon(source: &[u8], output: &mut Vec<u8>, pixels: usize) -> usize {
    use core::arch::aarch64 as neon;

    debug_assert!(pixels.is_multiple_of(16));
    let alpha;
    // SAFETY: the function contract proves each RGB structure load and RGBA
    // structure store is within the validated source and reserved destination
    // allocations. Each loop iteration covers sixteen pixels.
    unsafe {
        alpha = neon::vdupq_n_u8(u8::MAX);
        for pixel in (0..pixels).step_by(16) {
            let rgb = neon::vld3q_u8(source.as_ptr().add(pixel * 3));
            let rgba = neon::uint8x16x4_t(rgb.0, rgb.1, rgb.2, alpha);
            neon::vst4q_u8(output.as_mut_ptr().add(pixel * 4), rgba);
        }
    }

    let initialized_bytes = pixels * 4;
    // SAFETY: every byte in the vector prefix was written by the complete
    // sixteen-pixel stores above; the caller appends the scalar tail afterward.
    unsafe { output.set_len(initialized_bytes) };
    pixels / 16
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_simd::ops::adapters) const SIMD_RGB_TO_RGBA_PARALLEL_PIXEL_THRESHOLD:
    usize = 512 * 512;

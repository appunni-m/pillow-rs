//! Native byte kernels for SIMD PutAlpha.

#[cfg(feature = "parallel")]
use super::super::simd_rows_mut_serial;
use super::super::{NATIVE_BYTE_RGBA_MASK, native_byte_transform_copy_bytes, simd_pack_u16x16};
use crate::checked_dims::CheckedDims;
use wide::{u8x16, u16x16};

#[inline]
fn simd_put_alpha_block(
    source_block: [u8; 16],
    mask_block: [u8; 16],
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
) -> Option<[u8; 16]> {
    match (source_channels, output_channels) {
        (1, 2) => {
            let mut inputs = [0u8; 16];
            inputs[..8].copy_from_slice(&source_block[..8]);
            inputs[8..].copy_from_slice(&mask_block[..8]);
            Some(
                u8x16::new(inputs)
                    .swizzle_relaxed(u8x16::new([
                        0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15,
                    ]))
                    .to_array(),
            )
        }
        (2, 2) => {
            let values = u8x16::new(source_block);
            let alpha = u8x16::new(mask_block)
                .swizzle_relaxed(u8x16::new([0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7]));
            let alpha_mask = u8x16::new([
                0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
            ]);
            let preserved = values & (u8x16::splat(255) ^ alpha_mask);
            Some((preserved | (alpha & alpha_mask)).to_array())
        }
        (3, 4) => {
            let mut inputs = source_block;
            inputs[12..16].copy_from_slice(&mask_block[..4]);
            Some(
                u8x16::new(inputs)
                    .swizzle_relaxed(u8x16::new([
                        0, 1, 2, 12, 3, 4, 5, 13, 6, 7, 8, 14, 9, 10, 11, 15,
                    ]))
                    .to_array(),
            )
        }
        (4, 4) if cmyk_source => {
            // Pillow converts C/M/Y/K to RGB with MULDIV255 before writing
            // the new alpha sample. Keep the native CMYK bytes until this
            // vector block so the promotion uses the same integer rounding.
            let values = u8x16::new(source_block);
            let c = u16x16::from(values.swizzle_relaxed(u8x16::new([
                0, 4, 8, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])));
            let m = u16x16::from(values.swizzle_relaxed(u8x16::new([
                1, 5, 9, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])));
            let y = u16x16::from(values.swizzle_relaxed(u8x16::new([
                2, 6, 10, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])));
            let k = u16x16::from(values.swizzle_relaxed(u8x16::new([
                3, 7, 11, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])));
            let nk = u16x16::splat(255) - k;
            let muldiv255 = |value: u16x16| {
                let value = value + u16x16::splat(128);
                ((value >> 8u32) + value) >> 8u32
            };
            let red = simd_pack_u16x16(nk - muldiv255(c * nk)).to_array();
            let green = simd_pack_u16x16(nk - muldiv255(m * nk)).to_array();
            let blue = simd_pack_u16x16(nk - muldiv255(y * nk)).to_array();
            Some(
                u8x16::new([
                    red[0],
                    red[1],
                    red[2],
                    red[3],
                    green[0],
                    green[1],
                    green[2],
                    green[3],
                    blue[0],
                    blue[1],
                    blue[2],
                    blue[3],
                    mask_block[0],
                    mask_block[1],
                    mask_block[2],
                    mask_block[3],
                ])
                .swizzle_relaxed(u8x16::new([
                    0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15,
                ]))
                .to_array(),
            )
        }
        (4, 4) => {
            let values = u8x16::new(source_block);
            let alpha = u8x16::new(mask_block)
                .swizzle_relaxed(u8x16::new([0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3]));
            let alpha_mask = u8x16::new([0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
            let preserved = values & (u8x16::splat(255) ^ alpha_mask);
            Some((preserved | (alpha & alpha_mask)).to_array())
        }
        _ => None,
    }
}

#[inline]
fn write_put_alpha_scalar(
    source: &[u8],
    source_start: usize,
    output: &mut [u8],
    output_start: usize,
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
    alpha: u8,
) -> Option<()> {
    if source_start.checked_add(source_channels)? > source.len()
        || output_start.checked_add(output_channels)? > output.len()
    {
        return None;
    }
    if cmyk_source {
        let c = u32::from(source[source_start]);
        let m = u32::from(source[source_start + 1]);
        let y = u32::from(source[source_start + 2]);
        let k = u32::from(source[source_start + 3]);
        let nk = 255u32.saturating_sub(k);
        let red = (nk as i32 - crate::color::muldiv255(c, nk) as i32).clamp(0, 255) as u8;
        let green = (nk as i32 - crate::color::muldiv255(m, nk) as i32).clamp(0, 255) as u8;
        let blue = (nk as i32 - crate::color::muldiv255(y, nk) as i32).clamp(0, 255) as u8;
        output[output_start..output_start + 4].copy_from_slice(&[red, green, blue, alpha]);
    } else {
        output[output_start..output_start + source_channels]
            .copy_from_slice(&source[source_start..source_start + source_channels]);
        output[output_start + output_channels - 1] = alpha;
    }
    Some(())
}

/// Replace alpha samples from an L/1 mask using the source's native byte
/// layout.  This is a data-plane operation, not an RGBA promotion: L/ P
/// inputs become two-byte LA/PA samples, RGB inputs become four-byte RGBA,
/// and existing LA/RGBA alpha bytes are overwritten in place in the new
/// output buffer. The shared block kernel packs each complete vector group;
/// only the final incomplete group is handled by the scalar tail.
pub(super) fn simd_put_alpha_data_bytes(
    source: &[u8],
    mask: &[u8],
    pixels: usize,
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
) -> Option<(Vec<u8>, u64, u64)> {
    let pixels_per_vector = match (source_channels, output_channels) {
        (1, 2) | (2, 2) => 8,
        (3, 4) | (4, 4) => 4,
        _ => return None,
    };
    let source_len = pixels.checked_mul(source_channels)?;
    let output_len = pixels.checked_mul(output_channels)?;
    if source.len() != source_len || mask.len() != pixels {
        return None;
    }

    let mut output = vec![0u8; output_len];
    let mut pixel = 0usize;
    while pixel + pixels_per_vector <= pixels {
        let source_start = pixel * source_channels;
        let source_bytes = pixels_per_vector * source_channels;
        let output_start = pixel * output_channels;
        let output_bytes = pixels_per_vector * output_channels;
        let mut source_block = [0u8; 16];
        source_block[..source_bytes]
            .copy_from_slice(&source[source_start..source_start + source_bytes]);
        let mut mask_block = [0u8; 16];
        mask_block[..pixels_per_vector].copy_from_slice(&mask[pixel..pixel + pixels_per_vector]);

        let result = simd_put_alpha_block(
            source_block,
            mask_block,
            source_channels,
            output_channels,
            cmyk_source,
        )?;
        output[output_start..output_start + output_bytes].copy_from_slice(&result[..output_bytes]);
        pixel += pixels_per_vector;
    }

    for index in pixel..pixels {
        let source_start = index * source_channels;
        let output_start = index * output_channels;
        write_put_alpha_scalar(
            source,
            source_start,
            &mut output,
            output_start,
            source_channels,
            output_channels,
            cmyk_source,
            mask[index],
        )?;
    }

    Some((
        output,
        (pixel / pixels_per_vector) as u64,
        (pixels - pixel) as u64,
    ))
}

/// Interleave native RGB bytes with one constant alpha while building the
/// required RGBA result. A four-pixel vector exactly fills one `u8x16`; the
/// output grows into reserved storage so it is not zero-filled first.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_put_alpha_rgb_row(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    alpha: u8,
) {
    debug_assert_eq!(source.len(), width * 3);
    debug_assert_eq!(output.len(), width * 4);
    let vector_pixels = width / 4 * 4;
    const SHUFFLE: [u8; 16] = [0, 1, 2, 12, 3, 4, 5, 13, 6, 7, 8, 14, 9, 10, 11, 15];
    for (source_block, output_block) in source[..vector_pixels * 3]
        .chunks_exact(12)
        .zip(output[..vector_pixels * 4].chunks_exact_mut(16))
    {
        let mut padded = [alpha; 16];
        padded[..12].copy_from_slice(source_block);
        let packed = u8x16::new(padded)
            .swizzle_relaxed(u8x16::new(SHUFFLE))
            .to_array();
        output_block.copy_from_slice(&packed);
    }
    for pixel in vector_pixels..width {
        let source_start = pixel * 3;
        let output_start = pixel * 4;
        output[output_start..output_start + 3]
            .copy_from_slice(&source[source_start..source_start + 3]);
        output[output_start + 3] = alpha;
    }
}

pub(super) fn simd_put_alpha_rgb_mask_row(
    source: &[u8],
    mask: &[u8],
    output: &mut [u8],
    width: usize,
) {
    debug_assert_eq!(source.len(), width * 3);
    debug_assert_eq!(mask.len(), width);
    debug_assert_eq!(output.len(), width * 4);
    let vector_pixels = width / 4 * 4;
    const SHUFFLE: [u8; 16] = [0, 1, 2, 12, 3, 4, 5, 13, 6, 7, 8, 14, 9, 10, 11, 15];
    for ((source_block, mask_block), output_block) in source[..vector_pixels * 3]
        .chunks_exact(12)
        .zip(mask[..vector_pixels].chunks_exact(4))
        .zip(output[..vector_pixels * 4].chunks_exact_mut(16))
    {
        let mut padded = [0u8; 16];
        padded[..12].copy_from_slice(source_block);
        padded[12..].copy_from_slice(mask_block);
        let packed = u8x16::new(padded)
            .swizzle_relaxed(u8x16::new(SHUFFLE))
            .to_array();
        output_block.copy_from_slice(&packed);
    }
    for pixel in vector_pixels..width {
        let source_start = pixel * 3;
        let output_start = pixel * 4;
        output[output_start..output_start + 3]
            .copy_from_slice(&source[source_start..source_start + 3]);
        output[output_start + 3] = mask[pixel];
    }
}

pub(super) fn simd_put_alpha_rgba_mask_row(output: &mut [u8], mask: &[u8]) {
    debug_assert_eq!(output.len(), mask.len() * 4);
    for (pixel, &alpha) in output.chunks_exact_mut(4).zip(mask) {
        pixel[3] = alpha;
    }
}

/// Replace the alpha lane of native RGBA storage with one constant using
/// 16-byte vectors, preserving the three color lanes without repacking pixels.
pub(super) fn simd_put_alpha_rgba_constant_bytes(
    source: &[u8],
    width: u32,
    height: u32,
    alpha: u8,
) -> Option<(Vec<u8>, u64)> {
    let dimensions = CheckedDims::new(width, height, 4).ok()?;
    if source.len() != dimensions.total_bytes() {
        return None;
    }
    let mut output = vec![0; source.len()];
    let rgba_mask = u8x16::new(NATIVE_BYTE_RGBA_MASK);
    let alpha_values = u8x16::splat(alpha) & (u8x16::splat(u8::MAX) ^ rgba_mask);
    let transform = |input| (input & rgba_mask) | alpha_values;
    native_byte_transform_copy_bytes(source, &mut output, &transform);
    Some((
        output,
        u64::try_from(dimensions.total_pixels().div_ceil(4)).ok()?,
    ))
}

#[cfg(feature = "parallel")]
pub(super) const SIMD_PUT_ALPHA_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

pub(super) const SIMD_PUT_ALPHA_RGB_MASK_VECTOR_THRESHOLD: usize = 512 * 512;

#[inline]
pub(super) fn simd_put_alpha_rgb_constant_bytes(
    source: &[u8],
    width: u32,
    height: u32,
    alpha: u8,
) -> Option<(Vec<u8>, u64, u64)> {
    let dimensions = CheckedDims::new(width, height, 3).ok()?;
    let pixels = dimensions.total_pixels();
    if source.len() != dimensions.total_bytes() {
        return None;
    }
    let output_length = pixels.checked_mul(4)?;
    #[cfg(feature = "parallel")]
    if pixels >= SIMD_PUT_ALPHA_PARALLEL_PIXEL_THRESHOLD {
        let width = usize::try_from(width).ok()?;
        let height = usize::try_from(height).ok()?;
        let source_stride = width.checked_mul(3)?;
        let output_stride = width.checked_mul(4)?;
        let mut output = vec![alpha; output_length];
        simd_rows_mut_serial!(
            &mut output,
            output_stride,
            height,
            |_row_start, _row_end, y, row| {
                let source_start = y as usize * source_stride;
                simd_put_alpha_rgb_row(
                    &source[source_start..source_start + source_stride],
                    row,
                    width,
                    alpha,
                );
            }
        );
        return Some((
            output,
            (width / 4 * height) as u64,
            (width % 4 * height) as u64,
        ));
    }

    let mut output = Vec::with_capacity(output_length);
    let vector_pixels = pixels / 4 * 4;
    const SHUFFLE: [u8; 16] = [0, 1, 2, 12, 3, 4, 5, 13, 6, 7, 8, 14, 9, 10, 11, 15];
    for block in source[..vector_pixels * 3].chunks_exact(12) {
        let mut padded = [alpha; 16];
        padded[..12].copy_from_slice(block);
        let packed = u8x16::new(padded)
            .swizzle_relaxed(u8x16::new(SHUFFLE))
            .to_array();
        output.extend_from_slice(&packed);
    }
    for pixel in vector_pixels..pixels {
        let start = pixel * 3;
        output.extend_from_slice(&[source[start], source[start + 1], source[start + 2], alpha]);
    }
    Some((
        output,
        (vector_pixels / 4) as u64,
        (pixels - vector_pixels) as u64,
    ))
}

/// Replace alpha with one constant byte without first materializing a full
/// mask image. The output-changing promotion cases use the same native
/// interleave/conversion block as image-backed putalpha.
pub(super) fn simd_put_alpha_constant_bytes(
    source: &[u8],
    pixels: usize,
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
    alpha: u8,
) -> Option<(Vec<u8>, u64, u64)> {
    let pixels_per_vector = match (source_channels, output_channels) {
        (1, 2) | (2, 2) => 8,
        (3, 4) | (4, 4) => 4,
        _ => return None,
    };
    let source_len = pixels.checked_mul(source_channels)?;
    let output_len = pixels.checked_mul(output_channels)?;
    if source.len() != source_len {
        return None;
    }

    let mut output = vec![0u8; output_len];
    let mut pixel = 0usize;
    while pixel + pixels_per_vector <= pixels {
        let source_start = pixel * source_channels;
        let source_bytes = pixels_per_vector * source_channels;
        let output_start = pixel * output_channels;
        let output_bytes = pixels_per_vector * output_channels;
        let mut source_block = [0u8; 16];
        source_block[..source_bytes]
            .copy_from_slice(&source[source_start..source_start + source_bytes]);
        let result = simd_put_alpha_block(
            source_block,
            [alpha; 16],
            source_channels,
            output_channels,
            cmyk_source,
        )?;
        output[output_start..output_start + output_bytes].copy_from_slice(&result[..output_bytes]);
        pixel += pixels_per_vector;
    }

    for index in pixel..pixels {
        write_put_alpha_scalar(
            source,
            index * source_channels,
            &mut output,
            index * output_channels,
            source_channels,
            output_channels,
            cmyk_source,
            alpha,
        )?;
    }

    Some((
        output,
        (pixel / pixels_per_vector) as u64,
        (pixels - pixel) as u64,
    ))
}

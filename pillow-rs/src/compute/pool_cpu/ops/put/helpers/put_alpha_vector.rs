//! Original serial vector alpha kernels, with layout fixed by the mode owner.
//!
//! Const parameters specialize layouts before pixel execution. Architecture
//! availability is established by the linked mode declaration, never here.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel, Rgb, Rgba, RgbaImage};
use super::put_alpha::AlphaInput;
use wide::{u8x16, u16x16};

const NATIVE_BYTE_RGBA_MASK: [u8; 16] = [
    255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0,
];
fn cmyk_block(source_block: [u8; 16], mask_block: [u8; 16]) -> [u8; 16] {
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
    .to_array()
}

pub(crate) fn simd_put_alpha_block<const SOURCE: usize, const OUTPUT: usize, const CMYK: bool>(
    source_block: [u8; 16],
    mask_block: [u8; 16],
) -> Option<[u8; 16]> {
    match (SOURCE, OUTPUT) {
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
        (4, 4) if CMYK => Some(cmyk_block(source_block, mask_block)),
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

pub(crate) fn write_put_alpha_scalar<const SOURCE: usize, const OUTPUT: usize, const CMYK: bool>(
    source: &[u8],
    source_start: usize,
    output: &mut [u8],
    output_start: usize,
    alpha: u8,
) -> Option<()> {
    if source_start.checked_add(SOURCE)? > source.len()
        || output_start.checked_add(OUTPUT)? > output.len()
    {
        return None;
    }
    if CMYK {
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
        output[output_start..output_start + SOURCE]
            .copy_from_slice(&source[source_start..source_start + SOURCE]);
        output[output_start + OUTPUT - 1] = alpha;
    }
    Some(())
}

fn native_byte_transform_copy_bytes<F>(input: &[u8], output: &mut [u8], transform: &F)
where
    F: Fn(u8x16) -> u8x16,
{
    let mut input_chunks = input.chunks_exact(16);
    let mut output_chunks = output.chunks_exact_mut(16);
    for (source, destination) in input_chunks.by_ref().zip(output_chunks.by_ref()) {
        let vector =
            u8x16::new(<[u8; 16]>::try_from(source).expect("chunks_exact yields 16-byte chunks"));
        destination.copy_from_slice(&transform(vector).to_array());
    }
    let input_tail = input_chunks.remainder();
    let output_tail = output_chunks.into_remainder();
    if !input_tail.is_empty() {
        let mut padded = [0u8; 16];
        padded[..input_tail.len()].copy_from_slice(input_tail);
        let transformed = transform(u8x16::new(padded)).to_array();
        output_tail.copy_from_slice(&transformed[..input_tail.len()]);
    }
}

pub(crate) fn simd_put_alpha_rgba_constant_bytes(
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

pub(crate) fn simd_put_alpha_rgb_constant_bytes(
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

fn simd_put_alpha_constant_bytes<const SOURCE: usize, const OUTPUT: usize, const CMYK: bool>(
    source: &[u8],
    pixels: usize,
    alpha: u8,
) -> Option<(Vec<u8>, u64, u64)> {
    let pixels_per_vector = match (SOURCE, OUTPUT) {
        (1, 2) | (2, 2) => 8,
        (3, 4) | (4, 4) => 4,
        _ => return None,
    };
    let source_len = pixels.checked_mul(SOURCE)?;
    let output_len = pixels.checked_mul(OUTPUT)?;
    if source.len() != source_len {
        return None;
    }

    let mut output = vec![0u8; output_len];
    let mut pixel = 0usize;
    while pixel + pixels_per_vector <= pixels {
        let source_start = pixel * SOURCE;
        let source_bytes = pixels_per_vector * SOURCE;
        let output_start = pixel * OUTPUT;
        let output_bytes = pixels_per_vector * OUTPUT;
        let mut source_block = [0u8; 16];
        source_block[..source_bytes]
            .copy_from_slice(&source[source_start..source_start + source_bytes]);
        let result = simd_put_alpha_block::<SOURCE, OUTPUT, CMYK>(
            source_block,
            [alpha; 16],
        )?;
        output[output_start..output_start + output_bytes].copy_from_slice(&result[..output_bytes]);
        pixel += pixels_per_vector;
    }

    for index in pixel..pixels {
        write_put_alpha_scalar::<SOURCE, OUTPUT, CMYK>(
            source,
            index * SOURCE,
            &mut output,
            index * OUTPUT,
            alpha,
        )?;
    }

    Some((
        output,
        (pixel / pixels_per_vector) as u64,
        (pixels - pixel) as u64,
    ))
}

fn unsupported() -> PilError {
    PilError::NotImplementedError(
        "SIMD PutAlpha is unsupported for the validated image layout/parameters".into(),
    )
}

fn finish<P: Pixel<Subpixel = u8>>(
    width: u32, height: u32, bytes: Vec<u8>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    let dimensions = CheckedDims::new(width, height, P::CHANNEL_COUNT)?;
    crate::image_utils::typed_bytes_to_buffer(dimensions, bytes)
}

pub(crate) fn rgb_constant(input: AlphaInput<'_, Rgb<u8>>) -> Result<RgbaImage, PilError> {
    let source = input.image;
    let (output, vectors, tail) = simd_put_alpha_rgb_constant_bytes(
        source.as_raw(), source.width(), source.height(), input.alpha,
    ).ok_or_else(unsupported)?;
    crate::compute::record_pipeline_operation_vector_blocks(vectors);
    crate::compute::record_pipeline_operation_scalar_tail(tail);
    finish(source.width(), source.height(), output)
}

pub(crate) fn rgba_constant(input: AlphaInput<'_, Rgba<u8>>) -> Result<RgbaImage, PilError> {
    let source = input.image;
    let (output, vectors) = simd_put_alpha_rgba_constant_bytes(
        source.as_raw(), source.width(), source.height(), input.alpha,
    ).ok_or_else(unsupported)?;
    crate::compute::record_pipeline_operation_vector_blocks(vectors);
    finish(source.width(), source.height(), output)
}

pub(crate) fn native_constant<
    const SOURCE: usize, const OUTPUT: usize, const CMYK: bool,
    P: Pixel<Subpixel = u8>, O: Pixel<Subpixel = u8>,
>(input: AlphaInput<'_, P>) -> Result<ImageBuffer<O, Vec<u8>>, PilError> {
    let source = input.image;
    let pixels = (source.width() as usize).checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutAlpha pixel count overflow".into()))?;
    let (output, vectors, tail) = simd_put_alpha_constant_bytes::<SOURCE, OUTPUT, CMYK>(
        source.as_raw(), pixels, input.alpha,
    ).ok_or_else(unsupported)?;
    crate::compute::record_pipeline_operation_vector_blocks(vectors);
    crate::compute::record_pipeline_operation_scalar_tail(tail);
    finish(source.width(), source.height(), output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_rgb_putalpha_mask_interleave_covers_vector_blocks_and_tails() {
        for width in [1, 3, 4, 5, 17] {
            let source: Vec<u8> = (0..width * 3)
                .map(|index| (index * 37 + 11) as u8)
                .collect();
            let mask: Vec<u8> = (0..width).map(|index| (index * 53 + 7) as u8).collect();
            let mut actual = vec![0u8; width * 4];
            super::super::put_alpha_data_vector::rgb_mask_row(&source, &mask, &mut actual);
            let expected: Vec<u8> = source.chunks_exact(3).zip(&mask)
                .flat_map(|(pixel, alpha)| [pixel[0], pixel[1], pixel[2], *alpha]).collect();
            assert_eq!(actual, expected, "width {width}");
        }
    }

    #[test]
    fn native_rgba_putalpha_mask_replaces_only_alpha_samples() {
        for pixels in [0, 1, 3, 4, 5, 17] {
            let original: Vec<u8> = (0..pixels * 4)
                .map(|index| (index * 29 + 3) as u8).collect();
            let mask: Vec<u8> = (0..pixels).map(|index| (index * 47 + 5) as u8).collect();
            let mut actual = crate::raster::RgbaImage::from_raw(
                pixels as u32, 1, original.clone(),
            ).expect("RGBA test storage has one pixel per four bytes");
            super::super::put_alpha_data::replace_rgba_mask(&mut actual, &mask);
            let mut expected = original;
            for (pixel, &alpha) in expected.chunks_exact_mut(4).zip(&mask) { pixel[3] = alpha; }
            assert_eq!(actual.as_raw(), &expected, "pixel count {pixels}");
        }
    }

    #[test]
    fn native_rgb_putalpha_interleave_covers_vector_blocks_and_tails() {
        for (width, height) in [(1, 1), (3, 2), (4, 1), (5, 3), (17, 4)] {
            let source: Vec<u8> = (0..width * height * 3)
                .map(|index| (index * 37 + 11) as u8).collect();
            let (actual, vector_blocks, scalar_tail) =
                super::simd_put_alpha_rgb_constant_bytes(&source, width, height, 173)
                    .expect("valid RGB storage should interleave");
            let expected: Vec<u8> = source.chunks_exact(3)
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 173]).collect();
            let pixels = u64::from(width) * u64::from(height);
            assert_eq!(actual, expected, "{width}x{height}");
            assert_eq!(vector_blocks, pixels / 4, "{width}x{height}");
            assert_eq!(scalar_tail, pixels % 4, "{width}x{height}");
        }
        assert!(super::simd_put_alpha_rgb_constant_bytes(&[1, 2], 1, 1, 173).is_none());
    }

    #[test]
    fn native_rgba_putalpha_constant_preserves_rgb_and_replaces_alpha() {
        for width in [1usize, 3, 4, 5, 17] {
            let height = 3;
            let original: Vec<u8> = (0..width * height * 4)
                .map(|index| (index * 31 + 7) as u8).collect();
            let source = original.clone();
            let mut expected = original;
            for pixel in expected.chunks_exact_mut(4) { pixel[3] = 173; }
            let (actual, vector_count) = super::simd_put_alpha_rgba_constant_bytes(
                &source, width as u32, height as u32, 173,
            ).expect("valid RGBA storage should replace alpha");
            assert_eq!(vector_count, (width * height).div_ceil(4) as u64);
            assert_eq!(actual, expected, "width {width}");
        }
        assert!(super::simd_put_alpha_rgba_constant_bytes(&[0, 1], 1, 1, 173).is_none());
    }
}

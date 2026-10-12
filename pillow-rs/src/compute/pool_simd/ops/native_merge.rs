//! Native SIMD merge kernels for interleaving independent image bands.
//!
//! Keep byte-layout shuffles and architecture-specific interleavers here;
//! the operation adapter owns admission, backend receipts, and allocation telemetry.

use crate::error::PilError;
use crate::raster::DynamicImage;
use wide::u8x16;

/// Assemble one complete native output block from independent L bands.
///
/// `Image.merge` is an interleave operation: its scalar control plane chooses
/// the source band for each output channel, while these fixed shuffle masks
/// perform the hot byte rearrangement in `u8x16`. RGB uses four pixels per
/// block because twelve output bytes fit in one vector; the unused lanes are
/// never stored. There is no packed-RGBA conversion here, so CMYK's fourth
/// sample remains K throughout the copy.
#[cfg(any(target_arch = "x86_64", test))]
const fn native_merge_rgb_ssse3_mask(output_chunk: usize, source_channel: usize) -> [u8; 16] {
    let mut mask = [0x80; 16];
    let mut lane = 0;
    while lane < 16 {
        let output_byte = output_chunk * 16 + lane;
        if output_byte % 3 == source_channel {
            mask[lane] = (output_byte / 3) as u8;
        }
        lane += 1;
    }
    mask
}

#[cfg(any(target_arch = "x86_64", test))]
const NATIVE_MERGE_RGB_SSSE3_MASKS: [[[u8; 16]; 3]; 3] = [
    [
        native_merge_rgb_ssse3_mask(0, 0),
        native_merge_rgb_ssse3_mask(0, 1),
        native_merge_rgb_ssse3_mask(0, 2),
    ],
    [
        native_merge_rgb_ssse3_mask(1, 0),
        native_merge_rgb_ssse3_mask(1, 1),
        native_merge_rgb_ssse3_mask(1, 2),
    ],
    [
        native_merge_rgb_ssse3_mask(2, 0),
        native_merge_rgb_ssse3_mask(2, 1),
        native_merge_rgb_ssse3_mask(2, 2),
    ],
];

pub(super) fn vector_block(
    bands: &[&[u8]],
    pixel: usize,
    channels: usize,
) -> Option<(usize, [u8; 16])> {
    let pixels_per_vector = match channels {
        1 => 16,
        2 => 8,
        3 | 4 => 4,
        _ => return None,
    };
    let mut packed = [0u8; 16];
    for channel in 0..channels {
        let source = bands.get(channel)?.get(pixel..pixel + pixels_per_vector)?;
        let start = channel * pixels_per_vector;
        packed[start..start + pixels_per_vector].copy_from_slice(source);
    }
    let indices = match channels {
        1 => [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        2 => [0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15],
        3 => [0, 4, 8, 1, 5, 9, 2, 6, 10, 3, 7, 11, 12, 13, 14, 15],
        4 => [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15],
        _ => return None,
    };
    Some((
        pixels_per_vector * channels,
        u8x16::new(packed)
            .swizzle_relaxed(u8x16::new(indices))
            .to_array(),
    ))
}

#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
#[target_feature(enable = "ssse3")]
pub(super) unsafe fn rgb_interleave_ssse3(
    width: u32,
    height: u32,
    pixels: usize,
    output_len: usize,
    bands: &[&[u8]],
) -> Result<(DynamicImage, u64, u64), PilError> {
    use core::arch::x86_64::{
        __m128i, _mm_loadu_si128, _mm_or_si128, _mm_shuffle_epi8, _mm_storeu_si128,
    };

    let [red, green, blue] = bands else {
        return Err(PilError::InternalError(
            "SSSE3 RGB merge requires exactly three bands".into(),
        ));
    };
    if red.len() != pixels || green.len() != pixels || blue.len() != pixels {
        return Err(PilError::InternalError(
            "SSSE3 RGB merge band lengths do not match".into(),
        ));
    }

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::<u8>::with_capacity(output_len);

    // SAFETY: the caller checks SSSE3 support before entering this function.
    // Each input has `pixels` initialized bytes and each load starts at a
    // sixteen-pixel boundary below `vector_pixels`, so all three 16-byte
    // loads are in bounds. The three stores write consecutive 16-byte pieces
    // of the 48-byte output for those same pixels. Their end is at most
    // `vector_pixels * 3 <= output_len`. The output allocation is distinct
    // from the immutable source bands and has capacity `output_len`; its
    // length is set only after every vector store and scalar-tail write.
    unsafe {
        let masks: [[__m128i; 3]; 3] = std::array::from_fn(|chunk| {
            std::array::from_fn(|channel| {
                _mm_loadu_si128(NATIVE_MERGE_RGB_SSSE3_MASKS[chunk][channel].as_ptr().cast())
            })
        });
        let red_ptr = red.as_ptr();
        let green_ptr = green.as_ptr();
        let blue_ptr = blue.as_ptr();
        let output_ptr = output.as_mut_ptr();
        let mut pixel = 0usize;
        while pixel < vector_pixels {
            let red = _mm_loadu_si128(red_ptr.add(pixel).cast());
            let green = _mm_loadu_si128(green_ptr.add(pixel).cast());
            let blue = _mm_loadu_si128(blue_ptr.add(pixel).cast());

            let output_0 = _mm_or_si128(
                _mm_or_si128(
                    _mm_shuffle_epi8(red, masks[0][0]),
                    _mm_shuffle_epi8(green, masks[0][1]),
                ),
                _mm_shuffle_epi8(blue, masks[0][2]),
            );
            let output_1 = _mm_or_si128(
                _mm_or_si128(
                    _mm_shuffle_epi8(red, masks[1][0]),
                    _mm_shuffle_epi8(green, masks[1][1]),
                ),
                _mm_shuffle_epi8(blue, masks[1][2]),
            );
            let output_2 = _mm_or_si128(
                _mm_or_si128(
                    _mm_shuffle_epi8(red, masks[2][0]),
                    _mm_shuffle_epi8(green, masks[2][1]),
                ),
                _mm_shuffle_epi8(blue, masks[2][2]),
            );
            _mm_storeu_si128(output_ptr.add(pixel * 3).cast(), output_0);
            _mm_storeu_si128(output_ptr.add(pixel * 3 + 16).cast(), output_1);
            _mm_storeu_si128(output_ptr.add(pixel * 3 + 32).cast(), output_2);
            pixel += 16;
        }
        for pixel in vector_pixels..pixels {
            let output_offset = pixel * 3;
            output_ptr
                .add(output_offset)
                .write(*red.get_unchecked(pixel));
            output_ptr
                .add(output_offset + 1)
                .write(*green.get_unchecked(pixel));
            output_ptr
                .add(output_offset + 2)
                .write(*blue.get_unchecked(pixel));
        }
        output.set_len(output_len);
    }

    let image = crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, 3)?;
    Ok((
        image,
        (vector_pixels / 16) as u64,
        ((pixels - vector_pixels) * 3) as u64,
    ))
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
pub(super) fn rgb_interleave_neon(
    width: u32,
    height: u32,
    pixels: usize,
    output_len: usize,
    bands: &[&[u8]],
) -> Result<(DynamicImage, u64, u64), PilError> {
    use std::arch::aarch64::{uint8x16x3_t, vld1q_u8, vst3q_u8};

    let [red, green, blue] = bands else {
        return Err(PilError::InternalError(
            "SIMD RGB merge requires exactly three bands".into(),
        ));
    };
    if red.len() != pixels || green.len() != pixels || blue.len() != pixels {
        return Err(PilError::InternalError(
            "SIMD RGB merge band lengths do not match".into(),
        ));
    }

    let vector_pixels = pixels / 16 * 16;
    let mut output: Vec<u8> = Vec::with_capacity(output_len);

    // SAFETY: each source contains exactly `pixels` bytes, and every NEON
    // load starts at a 16-pixel boundary below `vector_pixels`, so its 16-byte
    // read is in bounds. Each `vst3q_u8` writes 48 bytes for those same 16
    // pixels; its destination offset is below `output_len - 47`. The scalar
    // tail writes three bytes per remaining pixel, also within `output_len`.
    // The fresh output allocation cannot overlap the immutable sources, and
    // its length stays zero until all vector and tail bytes are initialized.
    unsafe {
        let red_ptr = red.as_ptr();
        let green_ptr = green.as_ptr();
        let blue_ptr = blue.as_ptr();
        let output_ptr = output.as_mut_ptr();
        let mut pixel = 0usize;
        while pixel + 64 <= vector_pixels {
            let channels_0 = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel)),
                vld1q_u8(green_ptr.add(pixel)),
                vld1q_u8(blue_ptr.add(pixel)),
            );
            let channels_1 = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel + 16)),
                vld1q_u8(green_ptr.add(pixel + 16)),
                vld1q_u8(blue_ptr.add(pixel + 16)),
            );
            let channels_2 = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel + 32)),
                vld1q_u8(green_ptr.add(pixel + 32)),
                vld1q_u8(blue_ptr.add(pixel + 32)),
            );
            let channels_3 = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel + 48)),
                vld1q_u8(green_ptr.add(pixel + 48)),
                vld1q_u8(blue_ptr.add(pixel + 48)),
            );
            vst3q_u8(output_ptr.add(pixel * 3), channels_0);
            vst3q_u8(output_ptr.add((pixel + 16) * 3), channels_1);
            vst3q_u8(output_ptr.add((pixel + 32) * 3), channels_2);
            vst3q_u8(output_ptr.add((pixel + 48) * 3), channels_3);
            pixel += 64;
        }
        while pixel < vector_pixels {
            let channels = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel)),
                vld1q_u8(green_ptr.add(pixel)),
                vld1q_u8(blue_ptr.add(pixel)),
            );
            vst3q_u8(output_ptr.add(pixel * 3), channels);
            pixel += 16;
        }
        for pixel in vector_pixels..pixels {
            let offset = pixel * 3;
            output_ptr.add(offset).write(*red.get_unchecked(pixel));
            output_ptr
                .add(offset + 1)
                .write(*green.get_unchecked(pixel));
            output_ptr.add(offset + 2).write(*blue.get_unchecked(pixel));
        }
        output.set_len(output_len);
    }

    let image = crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, 3)?;
    Ok((
        image,
        (vector_pixels / 16) as u64,
        ((pixels - vector_pixels) * 3) as u64,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_merge_rgb_ssse3_masks_match_planar_to_interleaved_mapping() {
        let red = (0..16)
            .map(|index| ((index * 7 + 1) & 255) as u8)
            .collect::<Vec<_>>();
        let green = (0..16)
            .map(|index| ((index * 11 + 3) & 255) as u8)
            .collect::<Vec<_>>();
        let blue = (0..16)
            .map(|index| ((index * 13 + 5) & 255) as u8)
            .collect::<Vec<_>>();
        let bands = [&red, &green, &blue];
        let expected = (0..16)
            .flat_map(|index| [red[index], green[index], blue[index]])
            .collect::<Vec<_>>();
        let mut actual = [0u8; 48];

        for output_chunk in 0..3 {
            for channel in 0..3 {
                let mask = super::NATIVE_MERGE_RGB_SSSE3_MASKS[output_chunk][channel];
                for lane in 0..16 {
                    let source_pixel = mask[lane];
                    if source_pixel < 0x80 {
                        actual[output_chunk * 16 + lane] |=
                            bands[channel][usize::from(source_pixel)];
                    }
                }
            }
        }

        assert_eq!(actual.as_slice(), expected.as_slice());
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[test]
    fn neon_native_rgb_merge_writes_complete_structure_blocks_and_tails() {
        for length in [0usize, 1, 15, 16, 17, 31, 32, 33] {
            let red = (0..length).map(|i| (i * 7 + 1) as u8).collect::<Vec<_>>();
            let green = (0..length).map(|i| (i * 11 + 3) as u8).collect::<Vec<_>>();
            let blue = (0..length).map(|i| (i * 13 + 5) as u8).collect::<Vec<_>>();
            let expected = (0..length)
                .flat_map(|i| [red[i], green[i], blue[i]])
                .collect::<Vec<_>>();
            let bands = [&red[..], &green[..], &blue[..]];

            let (actual, vector_blocks, scalar_tail) =
                super::rgb_interleave_neon(length as u32, 1, length, length * 3, &bands)
                    .expect("native RGB merge should succeed");

            assert_eq!(actual.as_bytes(), expected, "pixel count {length}");
            assert_eq!(vector_blocks, (length / 16) as u64);
            assert_eq!(scalar_tail, ((length % 16) * 3) as u64);
        }
    }
}

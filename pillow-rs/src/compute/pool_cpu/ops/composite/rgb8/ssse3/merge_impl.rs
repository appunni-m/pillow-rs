//! SSSE3 planar-to-interleaved kernel for native RGB Merge.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::composite::helpers::{merge, native_bytes};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

const fn rgb_ssse3_mask(output_chunk: usize, source_channel: usize) -> [u8; 16] {
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

const RGB_SSSE3_MASKS: [[[u8; 16]; 3]; 3] = [
    [rgb_ssse3_mask(0, 0), rgb_ssse3_mask(0, 1), rgb_ssse3_mask(0, 2)],
    [rgb_ssse3_mask(1, 0), rgb_ssse3_mask(1, 1), rgb_ssse3_mask(1, 2)],
    [rgb_ssse3_mask(2, 0), rgb_ssse3_mask(2, 1), rgb_ssse3_mask(2, 2)],
];

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    let (width, height) = (source.width(), source.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, 3)?;
    let pixels = dimensions.total_bytes() / 3;
    if pixels < 16 {
        return merge::interleave_wide(
            source,
            remaining,
            Mode::ImageRgb8,
            crate::compute::pool_cpu::ops::composite::ops::MergeLayout::Three {
                lab_encoding: false,
            },
        );
    }
    let bands = merge::collect_bands(source, remaining, pixels)?;
    let [red, green, blue] = bands.as_slice() else {
        return Err(PilError::InternalError(
            "SSSE3 RGB Merge requires exactly three bands".into(),
        ));
    };
    let output_len = dimensions.total_bytes();
    // SAFETY: the prepared Merge declaration requires SSSE3 capability and
    // its request guard validates the exact RGB band layout and vector size.
    let (output, vector_blocks, scalar_tail) = unsafe {
        interleave_rgb_ssse3(red, green, blue, pixels, output_len)
    };
    let image = native_bytes::construct(width, height, Mode::ImageRgb8, output)?;
    crate::compute::record_pipeline_allocation(output_len);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("ssse3-interleave");
    Ok(image)
}

#[allow(unsafe_code)]
#[inline]
#[target_feature(enable = "ssse3")]
unsafe fn interleave_rgb_ssse3(
    red: &[u8],
    green: &[u8],
    blue: &[u8],
    pixels: usize,
    output_len: usize,
) -> (Vec<u8>, u64, u64) {
    use core::arch::x86_64::{
        __m128i, _mm_loadu_si128, _mm_or_si128, _mm_shuffle_epi8, _mm_storeu_si128,
    };

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::<u8>::with_capacity(output_len);
    // SAFETY: strategy admission checks the CPU's SSSE3 capability and the
    // Merge guard checks three equally sized one-byte bands with at least
    // sixteen pixels. Each load below is within those bands. The stores cover
    // disjoint 16-byte regions through vector_pixels * 3, and the scalar tail
    // stays within output_len. The fresh allocation cannot alias the inputs;
    // its length is set only after all bytes have been initialized.
    unsafe {
        let masks: [[__m128i; 3]; 3] = std::array::from_fn(|chunk| {
            std::array::from_fn(|channel| {
                _mm_loadu_si128(RGB_SSSE3_MASKS[chunk][channel].as_ptr().cast())
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
            let offset = pixel * 3;
            output_ptr.add(offset).write(*red.get_unchecked(pixel));
            output_ptr.add(offset + 1).write(*green.get_unchecked(pixel));
            output_ptr.add(offset + 2).write(*blue.get_unchecked(pixel));
        }
        output.set_len(output_len);
    }

    (
        output,
        (vector_pixels / 16) as u64,
        ((pixels - vector_pixels) * 3) as u64,
    )
}

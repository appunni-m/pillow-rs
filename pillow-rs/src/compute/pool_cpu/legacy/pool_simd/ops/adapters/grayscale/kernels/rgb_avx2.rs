//! x86 AVX2 implementation of the RGB-to-L grayscale loop.

/// Convert interleaved RGB bytes with AVX2 when the running CPU supports it.
///
/// Returns `None` when the operation cannot use the kernel. The output keeps
/// the scalar path's short-buffer behavior: missing pixels are zero-filled,
/// trailing incomplete RGB samples are ignored, and excess samples are unused.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)] // Isolated target-feature entry point for the checked SIMD kernel.
pub(super) fn grayscale_rgb_avx2(source: &[u8], expected_pixels: usize) -> Option<(Vec<u8>, u64)> {
    if !std::is_x86_feature_detected!("avx2") {
        return None;
    }

    // SAFETY: runtime feature detection established AVX2 support. The inner
    // function reads complete 48-byte blocks from `source` or a fully
    // initialized 48-byte tail buffer and stores into a 16-byte local array.
    Some(unsafe { grayscale_rgb_avx2_inner(source, expected_pixels) })
}

#[cfg(target_arch = "x86_64")]
struct GrayscaleRgbAvx2Masks {
    red_0: std::arch::x86_64::__m128i,
    red_1: std::arch::x86_64::__m128i,
    red_2: std::arch::x86_64::__m128i,
    green_0: std::arch::x86_64::__m128i,
    green_1: std::arch::x86_64::__m128i,
    green_2: std::arch::x86_64::__m128i,
    blue_0: std::arch::x86_64::__m128i,
    blue_1: std::arch::x86_64::__m128i,
    blue_2: std::arch::x86_64::__m128i,
    bias: std::arch::x86_64::__m256i,
    coeff_77: std::arch::x86_64::__m256i,
    coeff_150: std::arch::x86_64::__m256i,
    coeff_29: std::arch::x86_64::__m256i,
    coeff_70: std::arch::x86_64::__m256i,
    coeff_47: std::arch::x86_64::__m256i,
    coeff_117: std::arch::x86_64::__m256i,
}

#[cfg(target_arch = "x86_64")]
macro_rules! append_rgb_luma_avx2_block {
    ($output:ident, $input:expr, $count:expr, $masks:ident) => {{
        use std::arch::x86_64::{
            _mm_loadu_si128, _mm_or_si128, _mm_packus_epi16, _mm_shuffle_epi8, _mm_storeu_si128,
            _mm256_add_epi16, _mm256_castsi256_si128, _mm256_cvtepu8_epi16,
            _mm256_extracti128_si256, _mm256_mullo_epi16, _mm256_srli_epi16, _mm256_sub_epi16,
        };

        let input: *const u8 = $input;
        let mut packed = [0u8; 16];

        // SAFETY: the wrapper checked AVX2, and each call supplies one
        // complete block from the source or the initialized tail buffer.
        unsafe {
            let source_0 = _mm_loadu_si128(input.cast());
            let source_1 = _mm_loadu_si128(input.add(16).cast());
            let source_2 = _mm_loadu_si128(input.add(32).cast());

            let red = _mm_or_si128(
                _mm_or_si128(
                    _mm_shuffle_epi8(source_0, $masks.red_0),
                    _mm_shuffle_epi8(source_1, $masks.red_1),
                ),
                _mm_shuffle_epi8(source_2, $masks.red_2),
            );
            let green = _mm_or_si128(
                _mm_or_si128(
                    _mm_shuffle_epi8(source_0, $masks.green_0),
                    _mm_shuffle_epi8(source_1, $masks.green_1),
                ),
                _mm_shuffle_epi8(source_2, $masks.green_2),
            );
            let blue = _mm_or_si128(
                _mm_or_si128(
                    _mm_shuffle_epi8(source_0, $masks.blue_0),
                    _mm_shuffle_epi8(source_1, $masks.blue_1),
                ),
                _mm_shuffle_epi8(source_2, $masks.blue_2),
            );

            let red = _mm256_cvtepu8_epi16(red);
            let green = _mm256_cvtepu8_epi16(green);
            let blue = _mm256_cvtepu8_epi16(blue);

            // Exact Pillow coefficients decompose as 19595=77*256-117,
            // 38470=150*256+70, and 7471=29*256+47. The biased residual
            // stays in 2933..62603; base plus its carry stays below 65536.
            let base = _mm256_add_epi16(
                _mm256_add_epi16(
                    _mm256_mullo_epi16(red, $masks.coeff_77),
                    _mm256_mullo_epi16(green, $masks.coeff_150),
                ),
                _mm256_mullo_epi16(blue, $masks.coeff_29),
            );
            let residual = _mm256_add_epi16(
                _mm256_sub_epi16(
                    _mm256_add_epi16(
                        _mm256_mullo_epi16(green, $masks.coeff_70),
                        _mm256_mullo_epi16(blue, $masks.coeff_47),
                    ),
                    _mm256_mullo_epi16(red, $masks.coeff_117),
                ),
                $masks.bias,
            );
            let luma =
                _mm256_srli_epi16::<8>(_mm256_add_epi16(base, _mm256_srli_epi16::<8>(residual)));
            let luma = _mm_packus_epi16(
                _mm256_castsi256_si128(luma),
                _mm256_extracti128_si256::<1>(luma),
            );
            _mm_storeu_si128(packed.as_mut_ptr().cast(), luma);
        }

        $output.extend_from_slice(&packed[..$count]);
    }};
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(unsafe_code)] // The safe wrapper checks AVX2 and the inner loop bounds every load.
unsafe fn grayscale_rgb_avx2_inner(source: &[u8], expected_pixels: usize) -> (Vec<u8>, u64) {
    use std::arch::x86_64::{_mm_setr_epi8, _mm256_set1_epi16};

    let masks = GrayscaleRgbAvx2Masks {
        red_0: _mm_setr_epi8(0, 3, 6, 9, 12, 15, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1),
        red_1: _mm_setr_epi8(-1, -1, -1, -1, -1, -1, 2, 5, 8, 11, 14, -1, -1, -1, -1, -1),
        red_2: _mm_setr_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 1, 4, 7, 10, 13),
        green_0: _mm_setr_epi8(1, 4, 7, 10, 13, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1),
        green_1: _mm_setr_epi8(-1, -1, -1, -1, -1, 0, 3, 6, 9, 12, 15, -1, -1, -1, -1, -1),
        green_2: _mm_setr_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 2, 5, 8, 11, 14),
        blue_0: _mm_setr_epi8(2, 5, 8, 11, 14, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1),
        blue_1: _mm_setr_epi8(-1, -1, -1, -1, -1, 1, 4, 7, 10, 13, -1, -1, -1, -1, -1, -1),
        blue_2: _mm_setr_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 3, 6, 9, 12, 15),
        bias: _mm256_set1_epi16(i16::MIN),
        coeff_77: _mm256_set1_epi16(77),
        coeff_150: _mm256_set1_epi16(150),
        coeff_29: _mm256_set1_epi16(29),
        coeff_70: _mm256_set1_epi16(70),
        coeff_47: _mm256_set1_epi16(47),
        coeff_117: _mm256_set1_epi16(117),
    };

    let valid_pixels = expected_pixels.min(source.len() / 3);
    let full_blocks = valid_pixels / 16;
    let tail_pixels = valid_pixels % 16;
    let mut output = Vec::with_capacity(expected_pixels);

    for block in 0..full_blocks {
        let offset = block * 48;
        // SAFETY: `full_blocks` is derived from `source.len() / 3`, so each
        // offset has a complete 48-byte RGB block available.
        let input = unsafe { source.as_ptr().add(offset) };
        append_rgb_luma_avx2_block!(output, input, 16, masks);
    }

    if tail_pixels != 0 {
        let tail_bytes = tail_pixels * 3;
        let tail_start = full_blocks * 48;
        let mut padded = [0u8; 48];
        padded[..tail_bytes].copy_from_slice(&source[tail_start..tail_start + tail_bytes]);
        append_rgb_luma_avx2_block!(output, padded.as_ptr(), tail_pixels, masks);
    }

    let vector_blocks = (full_blocks + usize::from(tail_pixels != 0)) as u64;
    output.resize(expected_pixels, 0);
    (output, vector_blocks)
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::grayscale_rgb_avx2;
    use crate::color::rgb_to_luma_u8;

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn grayscale_rgb_avx2_matches_every_color_and_handles_tails() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }

        const PIXELS: usize = 1 << 24;
        let mut source = Vec::with_capacity(PIXELS * 3);
        for value in 0..PIXELS {
            source.push((value >> 16) as u8);
            source.push((value >> 8) as u8);
            source.push(value as u8);
        }
        let (actual, vector_blocks) =
            grayscale_rgb_avx2(&source, PIXELS).expect("AVX2 grayscale kernel");
        assert_eq!(vector_blocks, (PIXELS / 16) as u64);
        assert_eq!(actual.len(), PIXELS);
        for (index, pixel) in source.chunks_exact(3).enumerate() {
            assert_eq!(
                actual[index],
                rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]),
                "RGB pixel {index}: {pixel:?}"
            );
        }

        let short_source: Vec<u8> = (0..59).map(|index| (index * 37 + 11) as u8).collect();
        let (tail, vector_blocks) =
            grayscale_rgb_avx2(&short_source, 21).expect("AVX2 tail kernel");
        let mut expected: Vec<u8> = short_source
            .chunks_exact(3)
            .take(21)
            .map(|pixel| rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]))
            .collect();
        expected.resize(21, 0);
        assert_eq!(tail, expected);
        assert_eq!(vector_blocks, 2);

        let (prefix, vector_blocks) =
            grayscale_rgb_avx2(&short_source, 5).expect("AVX2 bounded kernel");
        assert_eq!(prefix, expected[..5]);
        assert_eq!(vector_blocks, 1);
    }
}

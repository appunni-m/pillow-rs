//! x86 SSSE3 implementation of native YCbCr Y-channel extraction.

/// Gather the first byte from native YCbCr triples using SSSE3 when present.
///
/// The returned counters describe sixteen-pixel vector blocks and scalar-tail
/// pixels. Callers must still validate that the source length matches the
/// image dimensions before constructing an image from the output.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
pub(super) fn ycbcr_luma_ssse3_bytes(source: &[u8]) -> Option<(Vec<u8>, u64, u64)> {
    if source.len() % 3 != 0 || !std::is_x86_feature_detected!("ssse3") {
        return None;
    }
    // SAFETY: the runtime feature check above proves SSSE3 is available.
    Some(unsafe { gather_ycbcr_luma_ssse3(source) })
}

#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
#[target_feature(enable = "ssse3")]
unsafe fn gather_ycbcr_luma_ssse3(source: &[u8]) -> (Vec<u8>, u64, u64) {
    use core::arch::x86_64::{
        _mm_loadu_si128, _mm_or_si128, _mm_setr_epi8, _mm_shuffle_epi8, _mm_storeu_si128,
    };

    const LANES: usize = 16;
    let pixel_count = source.len() / 3;
    let vector_pixels = pixel_count / LANES * LANES;
    let mut output = Vec::<u8>::with_capacity(pixel_count);

    // SAFETY: the caller checked SSSE3. Each loop reads three consecutive
    // sixteen-byte ranges for sixteen complete three-byte pixels. The final
    // load ends at or before source.len(), and the store initializes exactly
    // sixteen bytes within the output capacity.
    unsafe {
        let from_first = _mm_setr_epi8(0, 3, 6, 9, 12, 15, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1);
        let from_second =
            _mm_setr_epi8(-1, -1, -1, -1, -1, -1, 2, 5, 8, 11, 14, -1, -1, -1, -1, -1);
        let from_third = _mm_setr_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 1, 4, 7, 10, 13);
        for pixel in (0..vector_pixels).step_by(LANES) {
            let offset = pixel * 3;
            let first = _mm_loadu_si128(source.as_ptr().add(offset).cast());
            let second = _mm_loadu_si128(source.as_ptr().add(offset + 16).cast());
            let third = _mm_loadu_si128(source.as_ptr().add(offset + 32).cast());
            let y0 = _mm_shuffle_epi8(first, from_first);
            let y1 = _mm_shuffle_epi8(second, from_second);
            let y2 = _mm_shuffle_epi8(third, from_third);
            let y = _mm_or_si128(_mm_or_si128(y0, y1), y2);
            _mm_storeu_si128(output.as_mut_ptr().add(pixel).cast(), y);
        }
        output.set_len(vector_pixels);
    }

    for pixel in vector_pixels..pixel_count {
        output.push(source[pixel * 3]);
    }
    debug_assert_eq!(output.len(), pixel_count);
    (
        output,
        (vector_pixels / LANES) as u64,
        (pixel_count - vector_pixels) as u64,
    )
}

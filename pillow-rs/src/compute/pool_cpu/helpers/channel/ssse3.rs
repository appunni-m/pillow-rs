//! Original bounded SSSE3 gathers; prepared owners establish CPU capability.

/// # Safety
///
/// The caller must establish SSSE3 support, channels in 2..=4, a channel
/// inside that layout, and a source length divisible by the channel count.
#[target_feature(enable = "ssse3")]
#[allow(unsafe_code)]
pub(crate) unsafe fn gather_interleaved_channel_ssse3(
    source: &[u8],
    channels: usize,
    channel: usize,
) -> (Vec<u8>, u64, u64) {
    let pixel_count = source.len() / channels;
    let mut output = Vec::<u8>::with_capacity(pixel_count);
    // SAFETY: the caller established the channel layout and SSSE3 support;
    // the vector has capacity for every output pixel.
    let (vector_blocks, scalar_tail) = unsafe {
        gather_interleaved_channel_ssse3_into_raw(source, output.as_mut_ptr(), channels, channel)
    };
    // SAFETY: the raw helper initializes all `pixel_count` output bytes before
    // returning successfully.
    unsafe { output.set_len(pixel_count) };
    debug_assert_eq!(output.len(), pixel_count);
    (output, vector_blocks, scalar_tail)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
#[allow(unsafe_code)]
/// # Safety
///
/// The caller must establish SSSE3 support, validate `channels` as 2 through
/// 4, validate `channel < channels`, ensure the source length is divisible by
/// `channels`, provide writable output for every source pixel, and ensure the
/// output region does not overlap the source region.
pub(crate) unsafe fn gather_interleaved_channel_ssse3_into_raw(
    source: &[u8],
    output: *mut u8,
    channels: usize,
    channel: usize,
) -> (u64, u64) {
    use core::arch::x86_64::{
        _mm_loadu_si128, _mm_or_si128, _mm_setzero_si128, _mm_shuffle_epi8, _mm_storeu_si128,
    };

    const LANES: usize = 16;
    let pixel_count = source.len() / channels;
    let vector_pixels = pixel_count / LANES * LANES;
    let selectors: [[i8; LANES]; 4] = std::array::from_fn(|block| {
        std::array::from_fn(|lane| {
            let source_index = lane * channels + channel;
            if source_index / LANES == block {
                (source_index % LANES) as i8
            } else {
                i8::MIN
            }
        })
    });
    // SAFETY: the caller proved runtime SSSE3 support and non-overlapping
    // source/output regions. For each complete group of sixteen output pixels,
    // `channels * 16` source bytes are available; each unaligned load stays in
    // that range, each selector is a readable 16-byte stack array, and the
    // 16-byte store fits the writable output region.
    unsafe {
        for pixel in (0..vector_pixels).step_by(LANES) {
            let source_ptr = source.as_ptr().add(pixel * channels);
            let mut selected = _mm_setzero_si128();
            for block in 0..channels {
                let input = _mm_loadu_si128(source_ptr.add(block * LANES).cast());
                let selector = _mm_loadu_si128(selectors[block].as_ptr().cast());
                selected = _mm_or_si128(selected, _mm_shuffle_epi8(input, selector));
            }
            _mm_storeu_si128(output.add(pixel).cast(), selected);
        }
        for pixel in vector_pixels..pixel_count {
            *output.add(pixel) = source[pixel * channels + channel];
        }
    }
    (
        (vector_pixels / LANES) as u64,
        (pixel_count - vector_pixels) as u64,
    )
}


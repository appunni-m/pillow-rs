//! SSSE3 deinterleave kernel for one complete native CMYK block.

const fn build_selectors() -> [[[i8; 16]; 4]; 4] {
    let mut selectors = [[[i8::MIN; 16]; 4]; 4];
    let mut channel = 0;
    while channel < 4 {
        let mut source_block = 0;
        while source_block < 4 {
            let mut pixel = 0;
            while pixel < 16 {
                let byte = pixel * 4 + channel;
                if byte / 16 == source_block {
                    selectors[channel][source_block][pixel] = (byte % 16) as i8;
                }
                pixel += 1;
            }
            source_block += 1;
        }
        channel += 1;
    }
    selectors
}

const SELECTORS: [[[i8; 16]; 4]; 4] = build_selectors();

/// Deinterleave one complete 16-pixel CMYK block with SSSE3 byte shuffles.
///
/// # Safety
/// The caller must establish SSSE3 support before calling this function.
#[target_feature(enable = "ssse3")]
#[allow(unsafe_code)]
pub(crate) unsafe fn deinterleave_cmyk_16_ssse3(source: &[u8; 64]) -> [[u8; 16]; 4] {
    use core::arch::x86_64::{
        _mm_loadu_si128, _mm_or_si128, _mm_setzero_si128, _mm_shuffle_epi8, _mm_storeu_si128,
    };

    // SAFETY: source is a fixed 64-byte block, so each unaligned load covers
    // exactly one initialized vector and stays within the source allocation.
    let inputs = unsafe {
        [
            _mm_loadu_si128(source.as_ptr().cast()),
            _mm_loadu_si128(source.as_ptr().add(16).cast()),
            _mm_loadu_si128(source.as_ptr().add(32).cast()),
            _mm_loadu_si128(source.as_ptr().add(48).cast()),
        ]
    };
    let mut channels = [[0u8; 16]; 4];
    for channel in 0..4 {
        let mut selected = _mm_setzero_si128();
        for (source_block, input) in inputs.iter().enumerate() {
            let selector =
                unsafe { _mm_loadu_si128(SELECTORS[channel][source_block].as_ptr().cast()) };
            selected = _mm_or_si128(selected, _mm_shuffle_epi8(*input, selector));
        }
        // SAFETY: each output channel owns sixteen writable bytes, and the
        // loop writes one complete vector to that initialized stack array.
        unsafe { _mm_storeu_si128(channels[channel].as_mut_ptr().cast(), selected) };
    }
    channels
}

#[cfg(test)]
mod tests {
    use super::deinterleave_cmyk_16_ssse3;

    #[test]
    #[allow(unsafe_code)]
    fn ssse3_deinterleave_preserves_channel_order() {
        if !std::is_x86_feature_detected!("ssse3") {
            return;
        }

        let source = std::array::from_fn(|index| (index * 37 + 11) as u8);
        // SAFETY: runtime detection above confirms SSSE3 and source contains
        // exactly sixteen complete CMYK pixels.
        let channels = unsafe { deinterleave_cmyk_16_ssse3(&source) };
        for pixel in 0..16 {
            for channel in 0..4 {
                assert_eq!(channels[channel][pixel], source[pixel * 4 + channel]);
            }
        }
    }
}

//! Existing channel-gather boundary regression, moved with its kernel.

#[cfg(target_arch = "x86_64")]
#[test]
#[allow(unsafe_code)]
fn ssse3_direct_channel_gather_covers_vector_boundaries() {
    if !std::is_x86_feature_detected!("ssse3") {
        return;
    }

    for channels in 2..=4 {
        for pixel_count in [0, 1, 15, 16, 17, 31, 32, 33] {
            let source: Vec<u8> = (0..pixel_count * channels)
                .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
                .collect();
            for channel in 0..channels {
                let mut output = vec![0xA5; pixel_count];
                // SAFETY: the CPU check above established SSSE3; these loops
                // supply channels in 2..=4, a valid index, exact source bytes,
                // and a separate writable output byte for every pixel.
                let (vector_blocks, scalar_tail) = unsafe {
                    super::ssse3::gather_interleaved_channel_ssse3_into_raw(
                        &source, output.as_mut_ptr(), channels, channel,
                    )
                };
                let expected: Vec<u8> = source
                    .chunks_exact(channels)
                    .map(|pixel| pixel[channel])
                    .collect();

                assert_eq!(
                    output, expected,
                    "channels={channels} channel={channel} pixels={pixel_count}"
                );
                assert_eq!(vector_blocks, (pixel_count / 16) as u64);
                assert_eq!(scalar_tail, (pixel_count % 16) as u64);
            }
        }
    }
}

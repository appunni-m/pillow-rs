//! Original portable vector gather, with layout selected before execution.

use wide::{i16x8, u8x16, u16x8, u16x16};

#[cfg(any(
    all(target_arch = "x86", any(target_feature = "sse2", target_feature = "ssse3")),
    all(target_arch = "wasm32", target_feature = "simd128"),
))]
pub(crate) fn gather<const CHANNELS: usize, const CHANNEL: usize>(
    source: &[u8],
) -> (Vec<u8>, u64, u64) {
    const LANES: usize = 16;
    let pixel_count = source.len() / CHANNELS;
    let selectors: [u8x16; CHANNELS] = std::array::from_fn(|_block| {
        u8x16::new(std::array::from_fn(|lane| {
            ((lane * CHANNELS + CHANNEL) % LANES) as u8
        }))
    });
    let masks: [u8x16; CHANNELS] = std::array::from_fn(|block| {
        u8x16::new(std::array::from_fn(|lane| {
            if (lane * CHANNELS + CHANNEL) / LANES == block {
                u8::MAX
            } else {
                0
            }
        }))
    });

    let mut output = Vec::with_capacity(pixel_count);
    let vector_pixels = pixel_count / LANES * LANES;
    for pixels in source[..vector_pixels * CHANNELS].chunks_exact(CHANNELS * LANES) {
        let input_vectors: [u8x16; CHANNELS] = std::array::from_fn(|block| {
            let mut lanes = [0u8; LANES];
            let start = block * LANES;
            lanes.copy_from_slice(&pixels[start..start + LANES]);
            u8x16::new(lanes)
        });
        let mut selected = u8x16::splat(0);
        for block in 0..CHANNELS {
            selected |= input_vectors[block].swizzle_relaxed(selectors[block]) & masks[block];
        }
        output.extend_from_slice(&selected.to_array());
    }

    for pixel in source[vector_pixels * CHANNELS..].chunks_exact(CHANNELS) {
        output.push(pixel[CHANNEL]);
    }
    debug_assert_eq!(output.len(), pixel_count);
    (
        output,
        (vector_pixels / LANES) as u64,
        (pixel_count - vector_pixels) as u64,
    )
}

#[inline]
pub(crate) fn simd_pack_u16x16(value: u16x16) -> u8x16 {
    let [low, high]: [u16x8; 2] = bytemuck::cast(value);
    let low: i16x8 = bytemuck::cast(low);
    let high: i16x8 = bytemuck::cast(high);
    u8x16::narrow_i16x8(low, high)
}


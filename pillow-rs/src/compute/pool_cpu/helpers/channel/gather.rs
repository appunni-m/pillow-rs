//! Shared lane selection with compile-time native channel layout.

use wide::u8x16;

#[inline(always)]
pub(crate) fn gather_interleaved_channel<const CHANNELS: usize, const CHANNEL: usize>(
    blocks: &[u8x16; 4],
) -> u8x16 {
    let mut samples = u8x16::splat(0);
    for (block, bytes) in blocks.iter().enumerate().take(CHANNELS) {
        let select = u8x16::new(std::array::from_fn(|lane| {
            let index = lane * CHANNELS + CHANNEL;
            if index / 16 == block {
                (index % 16) as u8
            } else {
                0x80
            }
        }));
        samples |= bytes.swizzle_relaxed(select);
    }
    samples
}


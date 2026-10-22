//! Shared native-byte SIMD transformations and row traversal.

#[cfg(feature = "parallel")]
use super::simd_rows_mut_serial;
use wide::u8x16;

// Native byte transforms always operate on L/LA/RGB/RGBA layouts. Keeping
// their active-channel masks in one table avoids duplicating an input-driven
// branch in every closure monomorphization of `native_byte_transform`.
pub(super) const NATIVE_BYTE_ZERO_MASK: [u8; 16] = [0; 16];
pub(super) const NATIVE_BYTE_ALL_MASK: [u8; 16] = [u8::MAX; 16];
pub(super) const NATIVE_BYTE_LA_MASK: [u8; 16] = [
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
    u8::MAX,
    0,
];
pub(super) const NATIVE_BYTE_RGBA_MASK: [u8; 16] = [
    u8::MAX,
    u8::MAX,
    u8::MAX,
    0,
    u8::MAX,
    u8::MAX,
    u8::MAX,
    0,
    u8::MAX,
    u8::MAX,
    u8::MAX,
    0,
    u8::MAX,
    u8::MAX,
    u8::MAX,
    0,
];

const NATIVE_BYTE_ACTIVE_MASKS: [[u8; 16]; 5] = [
    NATIVE_BYTE_ZERO_MASK,
    NATIVE_BYTE_ALL_MASK,
    NATIVE_BYTE_LA_MASK,
    NATIVE_BYTE_ALL_MASK,
    NATIVE_BYTE_RGBA_MASK,
];

#[inline]
pub(super) fn native_byte_transform_bytes<F>(bytes: &mut [u8], channels: usize, transform: &F)
where
    F: Fn(u8x16) -> u8x16,
{
    // Native-byte callers pass only channel counts 1..=4, so this lookup is
    // total for every supported native byte image.
    let active = NATIVE_BYTE_ACTIVE_MASKS[channels];
    let active_vector = u8x16::new(active);
    let inactive = u8x16::splat(u8::MAX) - active_vector;
    let mut chunks = bytes.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let input = u8x16::new(
            <[u8; 16]>::try_from(&*chunk).expect("chunks_exact_mut yields 16-byte chunks"),
        );
        let transformed = transform(input);
        let output = (transformed & active_vector) | (input & inactive);
        chunk.copy_from_slice(&output.to_array());
    }
    let remainder = chunks.into_remainder();
    if !remainder.is_empty() {
        // A short row/image still has a genuine vector data path: copy only
        // the valid bytes into a stack-padded block, run the same SIMD kernel,
        // then commit only those lanes. This avoids pretending that a scalar
        // per-byte tail is SIMD while also avoiding an out-of-bounds load.
        let mut padded = [0u8; 16];
        padded[..remainder.len()].copy_from_slice(remainder);
        let input = u8x16::new(padded);
        let transformed = transform(input);
        let output = (transformed & active_vector) | (input & inactive);
        remainder.copy_from_slice(&output.to_array()[..remainder.len()]);
    }
}

#[inline]
pub(super) fn native_all_channel_transform_bytes<F>(bytes: &mut [u8], transform: &F)
where
    F: Fn(u8x16) -> u8x16,
{
    let mut chunks = bytes.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let input = u8x16::new(
            <[u8; 16]>::try_from(&*chunk).expect("chunks_exact_mut yields 16-byte chunks"),
        );
        chunk.copy_from_slice(&transform(input).to_array());
    }
    let remainder = chunks.into_remainder();
    if !remainder.is_empty() {
        let mut padded = [0u8; 16];
        padded[..remainder.len()].copy_from_slice(remainder);
        remainder.copy_from_slice(&transform(u8x16::new(padded)).to_array()[..remainder.len()]);
    }
}

/// Transform native posterize samples directly from the immutable source into
/// the one output allocation. Cloning and then mutating the clone makes two
/// full-frame passes for a point operation whose output depends only on each
/// input byte.
#[inline]
pub(super) fn native_byte_transform_copy_bytes<F>(input: &[u8], output: &mut [u8], transform: &F)
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

#[inline]
pub(super) fn apply_native_rows<F>(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    transform: F,
) where
    F: Fn(&mut [u8]) + Send + Sync,
{
    #[cfg(feature = "parallel")]
    let row_stride = width.saturating_mul(channels);
    #[cfg(feature = "parallel")]
    if bytes.len() >= 256 * 1024 && row_stride != 0 {
        simd_rows_mut_serial!(
            bytes,
            row_stride,
            height,
            |_row_start, _row_end, _y, row| {
                transform(row);
            }
        );
    } else if row_stride != 0 {
        for row in bytes.chunks_mut(row_stride).take(height) {
            transform(row);
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let row_stride = width.saturating_mul(channels);
        if row_stride != 0 {
            for row in bytes.chunks_mut(row_stride).take(height) {
                transform(row);
            }
        }
    }
}

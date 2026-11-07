use std::borrow::Cow;

/// Borrow little-endian F-mode sample bytes as native `f32` values when their
/// length and alignment permit a checked cast.
#[must_use]
pub(crate) fn f32_samples_borrowed_from_le_bytes(
    bytes: &[u8],
    sample_count: usize,
) -> Option<&[f32]> {
    #[cfg(target_endian = "little")]
    {
        let sample_bytes = bytes.get(..sample_count.saturating_mul(4)).unwrap_or(bytes);
        if let Ok(samples) = bytemuck::try_cast_slice::<u8, f32>(sample_bytes)
            && samples.len() == sample_count
        {
            return Some(samples);
        }
    }
    None
}

/// View little-endian F-mode sample bytes as native `f32` values when that is
/// safe, otherwise decode into owned samples. Select at most `sample_count`
/// words and ignore incomplete trailing bytes, matching the old
/// `chunks_exact(4).take(sample_count)` decoders.
#[must_use]
pub(crate) fn f32_samples_from_le_bytes(bytes: &[u8], sample_count: usize) -> Cow<'_, [f32]> {
    if let Some(samples) = f32_samples_borrowed_from_le_bytes(bytes, sample_count) {
        return Cow::Borrowed(samples);
    }
    let sample_bytes = bytes.get(..sample_count.saturating_mul(4)).unwrap_or(bytes);
    Cow::Owned(
        sample_bytes
            .chunks_exact(4)
            .take(sample_count)
            .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
            .collect(),
    )
}

/// Borrow little-endian I-mode sample bytes as native `i32` values when their
/// length and alignment permit a checked cast.
#[must_use]
pub(crate) fn i32_samples_borrowed_from_le_bytes(
    bytes: &[u8],
    sample_count: usize,
) -> Option<&[i32]> {
    #[cfg(target_endian = "little")]
    {
        let sample_bytes = bytes.get(..sample_count.saturating_mul(4)).unwrap_or(bytes);
        if let Ok(samples) = bytemuck::try_cast_slice::<u8, i32>(sample_bytes)
            && samples.len() == sample_count
        {
            return Some(samples);
        }
    }
    None
}

/// View little-endian I-mode sample bytes as native `i32` values when the
/// storage is aligned; otherwise decode them without changing sample bits.
/// Select at most `sample_count` words and ignore incomplete trailing bytes,
/// matching the existing four-byte scalar-carrier readers.
#[must_use]
pub(crate) fn i32_samples_from_le_bytes(bytes: &[u8], sample_count: usize) -> Cow<'_, [i32]> {
    if let Some(samples) = i32_samples_borrowed_from_le_bytes(bytes, sample_count) {
        return Cow::Borrowed(samples);
    }
    let sample_bytes = bytes.get(..sample_count.saturating_mul(4)).unwrap_or(bytes);
    Cow::Owned(
        sample_bytes
            .chunks_exact(4)
            .take(sample_count)
            .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
            .collect(),
    )
}

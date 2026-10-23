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

#[cfg(test)]
mod f32_sample_view_tests {
    use super::f32_samples_from_le_bytes;
    use crate::raster::RgbaImage;
    use std::borrow::Cow;

    #[test]
    fn aligned_little_endian_scalar_carrier_is_borrowed() {
        let words = [1.25f32.to_bits(), (-0.0f32).to_bits()];
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        let image = RgbaImage::from_raw(2, 1, bytes).expect("two scalar samples");
        let raw = image.as_raw();
        let samples = f32_samples_from_le_bytes(raw, words.len());

        if cfg!(target_endian = "little")
            && (raw.as_ptr() as usize).is_multiple_of(std::mem::align_of::<f32>())
        {
            assert!(matches!(samples, Cow::Borrowed(_)));
        } else {
            assert!(matches!(samples, Cow::Owned(_)));
        }
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>(),
            words
        );
    }

    #[test]
    fn unaligned_little_endian_samples_decode_without_changing_word_bits() {
        let expected = [1.25f32.to_bits(), (-0.0f32).to_bits(), 0x7fc1_2345];
        let bytes = expected
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        let mut storage = vec![0; bytes.len() + std::mem::align_of::<f32>()];
        let base = storage.as_ptr() as usize;
        let offset = (0..std::mem::align_of::<f32>())
            .find(|offset| (base + offset) % std::mem::align_of::<f32>() != 0)
            .expect("an unaligned byte offset must exist");
        storage[offset..offset + bytes.len()].copy_from_slice(&bytes);

        let samples = f32_samples_from_le_bytes(&storage[offset..], expected.len());
        assert!(matches!(samples, Cow::Owned(_)));
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[cfg(test)]
mod i32_sample_view_tests {
    use super::{i32_samples_borrowed_from_le_bytes, i32_samples_from_le_bytes};
    use std::borrow::Cow;

    #[test]
    fn aligned_little_endian_integer_carrier_is_borrowed() {
        let expected = [i32::MIN, -123_456_789, -1, 0, 1, i32::MAX];
        let bytes = expected
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        let samples = i32_samples_from_le_bytes(&bytes, expected.len());

        if cfg!(target_endian = "little")
            && (bytes.as_ptr() as usize).is_multiple_of(std::mem::align_of::<i32>())
        {
            assert!(matches!(samples, Cow::Borrowed(_)));
            assert_eq!(
                i32_samples_borrowed_from_le_bytes(&bytes, expected.len()),
                Some(&expected[..])
            );
        } else {
            assert!(matches!(samples, Cow::Owned(_)));
            assert_eq!(
                i32_samples_borrowed_from_le_bytes(&bytes, expected.len()),
                None
            );
        }
        assert_eq!(samples.as_ref(), expected);
    }

    #[test]
    fn unaligned_integer_carrier_decodes_signed_words_and_ignores_trailing_bytes() {
        let expected = [i32::MIN, -1, 0, i32::MAX];
        let bytes = expected
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        let mut storage = vec![0; bytes.len() + std::mem::align_of::<i32>() + 1];
        let base = storage.as_ptr() as usize;
        let offset = (0..std::mem::align_of::<i32>())
            .find(|offset| (base + offset) % std::mem::align_of::<i32>() != 0)
            .expect("an unaligned byte offset must exist");
        storage[offset..offset + bytes.len()].copy_from_slice(&bytes);
        storage[offset + bytes.len()] = 0xa5;

        let samples = i32_samples_from_le_bytes(&storage[offset..], expected.len());
        assert!(matches!(samples, Cow::Owned(_)));
        assert_eq!(
            i32_samples_borrowed_from_le_bytes(&storage[offset..], expected.len()),
            None
        );
        assert_eq!(samples.as_ref(), expected);
    }
}

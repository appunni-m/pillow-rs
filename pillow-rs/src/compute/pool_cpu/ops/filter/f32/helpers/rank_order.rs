/// Detect F32 samples whose comparison semantics require the serial rank path.
pub(super) fn requires_default_comparator(raw: &[u8]) -> bool {
    let mut saw_negative_zero = false;
    let mut saw_positive_zero = false;
    for bytes in raw.chunks_exact(4) {
        let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if value.is_nan() {
            // NaN comparisons are false in Pillow's partition selector; the vector order is not equivalent.
            return true;
        }
        if value == 0.0 {
            saw_negative_zero |= value.is_sign_negative();
            saw_positive_zero |= !value.is_sign_negative();
            if saw_negative_zero && saw_positive_zero {
                // Pillow compares zero signs equal although their output bytes differ.
                return true;
            }
        }
    }
    false
}

/// Retains the old name for the existing migration-only unit-test module.
#[cfg(test)]
pub(super) fn requires_default_comparator_sort(raw: &[u8]) -> bool {
    requires_default_comparator(raw)
}

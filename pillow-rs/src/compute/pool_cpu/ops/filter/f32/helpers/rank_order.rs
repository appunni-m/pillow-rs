/// Detect samples for which the vector ordering may select different F bytes.
pub(super) fn requires_default_comparator_sort(raw: &[u8]) -> bool {
    let mut saw_negative_zero = false;
    let mut saw_positive_zero = false;
    for bytes in raw.chunks_exact(4) {
        let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if value.is_nan() {
            // The serial partial_cmp fallback treats every NaN as equal to every value.
            return true;
        }
        if value == 0.0 {
            saw_negative_zero |= value.is_sign_negative();
            saw_positive_zero |= !value.is_sign_negative();
            if saw_negative_zero && saw_positive_zero {
                // partial_cmp considers zero signs equal although their output bytes differ.
                return true;
            }
        }
    }
    false
}

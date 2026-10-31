//! Exact bit-pattern uniformity shared by F-mode rank strategies.

pub(super) fn native_uniform_f_mode_image(raw: &[u8], width: usize, height: usize) -> bool {
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    let Some(byte_count) = pixel_count.checked_mul(4) else {
        return false;
    };
    if pixel_count == 0 || raw.len() != byte_count {
        return false;
    }
    let Some(first) = raw.get(..4) else {
        return false;
    };
    raw.chunks_exact(4).skip(1).all(|pixel| pixel == first)
}

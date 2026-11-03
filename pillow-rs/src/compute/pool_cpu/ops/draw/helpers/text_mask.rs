//! Native FreeType mask reads shared by direct Draw Text owners.

/// Read coverage from a mask whose layout was selected before entering the loop.
#[inline]
pub(crate) fn coverage<const RGBA_MASK: bool>(mask: &[u8], pixel_index: usize) -> u8 {
    if RGBA_MASK {
        mask[pixel_index * 4 + 3]
    } else {
        mask[pixel_index]
    }
}

/// Read the mask's RGB ink when present, otherwise use the mode's resolved fill.
#[inline]
pub(crate) fn color<const RGBA_MASK: bool>(
    mask: &[u8],
    pixel_index: usize,
    fill: (u8, u8, u8),
) -> (u8, u8, u8) {
    if RGBA_MASK {
        let offset = pixel_index * 4;
        (mask[offset], mask[offset + 1], mask[offset + 2])
    } else {
        fill
    }
}

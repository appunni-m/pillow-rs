//! Native glyph-mask layouts shared by direct Draw Text owners.

/// Expand one coverage byte per glyph pixel to Pillow's RGBA mask layout.
pub(crate) fn expand_coverage_to_rgba(
    mask: &[u8],
    fill: (u8, u8, u8),
) -> Vec<u8> {
    let mut pixels = vec![0u8; mask.len() * 4];
    for (index, coverage) in mask.iter().copied().enumerate() {
        if coverage == 0 {
            continue;
        }
        let offset = index * 4;
        pixels[offset] = fill.0;
        pixels[offset + 1] = fill.1;
        pixels[offset + 2] = fill.2;
        pixels[offset + 3] = coverage;
    }
    pixels
}

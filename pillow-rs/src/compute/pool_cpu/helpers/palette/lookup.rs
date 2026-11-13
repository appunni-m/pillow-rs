//! Native indexed-palette sample lookup shared by CPU operation families.

use crate::compute::registry::support::PaletteInput;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
pub(crate) fn color(index: u8, palette: PaletteInput<'_>) -> [u8; 3] {
    let start = usize::from(index) * 3;
    let Some(rgb) = palette.rgb else {
        return [0; 3];
    };
    [
        rgb.get(start).copied().unwrap_or(0),
        rgb.get(start + 1).copied().unwrap_or(0),
        rgb.get(start + 2).copied().unwrap_or(0),
    ]
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
pub(crate) fn alpha(index: u8, palette: PaletteInput<'_>) -> u8 {
    palette
        .alpha
        .get(usize::from(index))
        .copied()
        .unwrap_or(u8::MAX)
}

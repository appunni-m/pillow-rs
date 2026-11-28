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

/// Returns one palette entry in native CMYK channels when present.
///
/// RGB palettes use Pillow's existing channel complement conversion. Native
/// CMYK entries bypass the RGB projection so P-to-CMYK keeps all four source
/// samples without a round trip through another mode.
// VERIFY: source=pending; parity=pending; benchmark=pending
#[inline]
pub(crate) fn cmyk(index: u8, palette: PaletteInput<'_>) -> [u8; 4] {
    if let Some(entries) = palette
        .data
        .and_then(crate::image::PaletteData::cmyk_entries)
    {
        let start = usize::from(index) * 4;
        return [
            entries.get(start).copied().unwrap_or(0),
            entries.get(start + 1).copied().unwrap_or(0),
            entries.get(start + 2).copied().unwrap_or(0),
            entries.get(start + 3).copied().unwrap_or(0),
        ];
    }
    let [red, green, blue] = color(index, palette);
    [u8::MAX - red, u8::MAX - green, u8::MAX - blue, 0]
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

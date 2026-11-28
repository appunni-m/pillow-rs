//! Native P quantization strategy for the CPU Default architecture.

use crate::compute::pool_cpu::helpers::quantization::RgbPixelSource;
use crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source;
use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::GrayImage;

/// Quantizes P indices through the attached palette's RGB entries.
// Source contract: P quantization discards palette alpha and returns an RGB P
// palette. Keep an alpha-bearing P source in the frozen parity matrix.
// TODO(pillow-rs#3ptb): add the P palette-alpha source case to parity.
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &GrayImage,
    palette: PaletteInput<'_>,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let colors_by_index = palette_color_table(palette.rgb);
    quantize_rgb_source::execute(
        &PaletteSamples {
            indices: image.as_raw(),
            colors: &colors_by_index,
        },
        width,
        height,
        colors,
        kmeans,
        method,
    )
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn palette_color_table(palette: Option<&[u8]>) -> [[u8; 3]; 256] {
    let palette = palette.unwrap_or_default();
    let mut colors = [[0u8; 3]; 256];
    for (index, color) in colors.iter_mut().enumerate() {
        let offset = index * 3;
        *color = [
            palette.get(offset).copied().unwrap_or(0),
            palette.get(offset + 1).copied().unwrap_or(0),
            palette.get(offset + 2).copied().unwrap_or(0),
        ];
    }
    colors
}

struct PaletteSamples<'a> {
    indices: &'a [u8],
    colors: &'a [[u8; 3]; 256],
}

impl RgbPixelSource for PaletteSamples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.indices.len()
    }

    #[inline(always)]
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        self.colors[usize::from(self.indices[index])]
    }
}

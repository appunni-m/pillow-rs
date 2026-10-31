//! Pillow-compatible LittleCMS CLUT conversion for exact ImageRgb8 input.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, RgbImage};

const GRID: usize = crate::lab::GRID;
const CHANNELS: usize = 3;

#[derive(Clone, Copy)]
struct Axis {
    cell: usize,
    fraction: i64,
    step: usize,
}

const fn make_axes() -> [Axis; 256] {
    let mut axes = [Axis { cell: 0, fraction: 0, step: 1 }; 256];
    let mut value = 0usize;
    while value < axes.len() {
        let scaled = value as u32 * 257 * 32;
        let fixed = scaled + (scaled + 0x7fff) / 0xffff;
        axes[value] = Axis {
            cell: (fixed >> 16) as usize,
            fraction: (fixed & 0xffff) as i64,
            step: if value == 255 { 0 } else { 1 },
        };
        value += 1;
    }
    axes
}

const AXES: [Axis; 256] = make_axes();

/// Converts tightly packed native RGB8 bytes to Pillow's packed LAB samples.
pub(crate) fn convert_rgb_bytes(source: &[u8], pixel_count: usize) -> Result<Vec<u8>, PilError> {
    let expected = pixel_count
        .checked_mul(CHANNELS)
        .ok_or_else(|| PilError::InternalError("LAB input size overflow".into()))?;
    if source.len() != expected {
        return Err(PilError::InternalError(
            "LAB conversion requires tightly packed RGB8 input".into(),
        ));
    }
    let mut output = vec![0; expected];
    for (source, target) in source.chunks_exact(3).zip(output.chunks_exact_mut(3)) {
        let r = AXES[source[0] as usize];
        let g = AXES[source[1] as usize];
        let b = AXES[source[2] as usize];
        let base = r.cell * GRID * GRID + g.cell * GRID + b.cell;

        // Keep LittleCMS's six-tetrahedra tie-breaking order and vertex order.
        let (p, q, t) = if r.fraction >= g.fraction {
            if g.fraction >= b.fraction {
                (0usize, 1usize, 2usize)
            } else if b.fraction >= r.fraction {
                (2, 0, 1)
            } else {
                (0, 2, 1)
            }
        } else if r.fraction >= b.fraction {
            (1, 0, 2)
        } else if g.fraction >= b.fraction {
            (1, 2, 0)
        } else {
            (2, 1, 0)
        };
        let fractions = [r.fraction, g.fraction, b.fraction];
        let steps = [r.step * GRID * GRID, g.step * GRID, b.step];
        let v0 = base * CHANNELS;
        let v1 = (base + steps[p]) * CHANNELS;
        let v2 = (base + steps[p] + steps[q]) * CHANNELS;
        let v3 = (base + steps[p] + steps[q] + steps[t]) * CHANNELS;
        for channel in 0..CHANNELS {
            let a0 = crate::lab::lut_value(v0 + channel);
            let a1 = crate::lab::lut_value(v1 + channel);
            let a2 = crate::lab::lut_value(v2 + channel);
            let a3 = crate::lab::lut_value(v3 + channel);
            let rest = (a1 - a0) * fractions[p]
                + (a2 - a1) * fractions[q]
                + (a3 - a2) * fractions[t]
                + 0x8001;
            let lab_v4 = a0 + ((rest + (rest >> 16)) >> 16);

            // Pillow quantizes the LabV4 sample, then removes A/B's +128 bias.
            let packed = ((lab_v4 as u32 * 65281 + 8_388_608) >> 24) as u8;
            target[channel] = if channel == 0 { packed } else { packed.wrapping_sub(128) };
        }
    }
    Ok(output)
}

/// Executes the source-mode kernel and constructs the exact LAB variant.
pub(crate) fn execute(input: &RgbImage) -> Result<DynamicImage, PilError> {
    let (width, height) = input.dimensions();
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|width| usize::try_from(height).ok().and_then(|height| width.checked_mul(height)))
        .ok_or_else(|| PilError::InternalError("LAB image size overflow".into()))?;
    let bytes = convert_rgb_bytes(input.as_raw(), pixels)?;
    let output = ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("LAB output buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageLab8(output))
}

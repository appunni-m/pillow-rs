//! Shared LittleCMS-compatible RGB-to-LAB lookup-table interpolation.

use crate::error::PilError;
use crate::raster::RgbImage;

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

/// Converts tightly packed RGB8 samples to Pillow's packed LAB color samples.
pub(crate) fn convert_rgb_bytes(source: &[u8], pixel_count: usize) -> Result<Vec<u8>, PilError> {
    convert_color_bytes::<3>(source, pixel_count, "LAB conversion requires tightly packed RGB8 input")
}

/// Converts four-byte RGB-family pixels to LAB, ignoring only their auxiliary lane.
pub(crate) fn convert_four_byte_rgb_bytes(
    source: &[u8],
    pixel_count: usize,
) -> Result<Vec<u8>, PilError> {
    convert_color_bytes::<4>(
        source,
        pixel_count,
        "LAB conversion requires tightly packed four-byte color input",
    )
}

pub(crate) fn pixel_count(width: u32, height: u32) -> Result<usize, PilError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| usize::try_from(height).ok().and_then(|height| width.checked_mul(height)))
        .ok_or_else(|| PilError::InternalError("LAB image size overflow".into()))
}

/// Converts mode-native RGB samples directly to packed LAB output.
///
/// Convert mode strategies map their own source pixels to RGB components and
/// feed them here without materializing an intermediate image mode.
#[inline]
pub(crate) fn convert_rgb_samples<I>(
    width: u32,
    height: u32,
    samples: I,
) -> Result<RgbImage, PilError>
where
    I: ExactSizeIterator<Item = [u8; 3]>,
{
    let count = pixel_count(width, height)?;
    if samples.len() != count {
        return Err(PilError::InternalError(
            "LAB conversion sample count did not match image dimensions".into(),
        ));
    }
    let mut output = Vec::with_capacity(
        count
            .checked_mul(CHANNELS)
            .ok_or_else(|| PilError::InternalError("LAB output size overflow".into()))?,
    );
    for sample in samples {
        output.extend_from_slice(&convert_rgb_pixel(sample));
    }
    RgbImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("LAB conversion output shape mismatch".into()))
}

fn convert_color_bytes<const STRIDE: usize>(
    source: &[u8],
    pixel_count: usize,
    mismatch_reason: &'static str,
) -> Result<Vec<u8>, PilError> {
    let expected_source = pixel_count
        .checked_mul(STRIDE)
        .ok_or_else(|| PilError::InternalError("LAB input size overflow".into()))?;
    if source.len() != expected_source {
        return Err(PilError::InternalError(mismatch_reason.into()));
    }
    let output_len = pixel_count
        .checked_mul(CHANNELS)
        .ok_or_else(|| PilError::InternalError("LAB input size overflow".into()))?;
    let mut output = vec![0; output_len];
    for (source, target) in source.chunks_exact(STRIDE).zip(output.chunks_exact_mut(CHANNELS)) {
        target.copy_from_slice(&convert_rgb_pixel([source[0], source[1], source[2]]));
    }
    Ok(output)
}

#[inline(always)]
fn convert_rgb_pixel(rgb: [u8; 3]) -> [u8; 3] {
    let r = AXES[rgb[0] as usize];
    let g = AXES[rgb[1] as usize];
    let b = AXES[rgb[2] as usize];
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
    let mut output = [0; CHANNELS];
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
        output[channel] = if channel == 0 { packed } else { packed.wrapping_sub(128) };
    }
    output
}

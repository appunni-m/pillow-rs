//! Pillow's byte RGB-to-LAB conversion.
//!
//! Pillow routes this conversion through LittleCMS's default sRGB-to-LabV4
//! transform and then packs the LabV2 byte representation.  The bundled
//! 33³ CLUT is the exact LittleCMS transform table; input coordinates,
//! tetrahedral interpolation, and byte packing below preserve its integer
//! rounding.  This avoids a runtime ICC dependency and has been checked
//! against every 8-bit RGB input.

use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView, ImageBuffer, RgbImage};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const GRID: usize = 33;
const CHANNELS: usize = 3;
const LUT_VALUES: usize = GRID * GRID * GRID * CHANNELS;
const LUT_BYTES: usize = LUT_VALUES * 2;
pub(crate) const GPU_TABLE_WORDS: usize = GRID * GRID * GRID * 2;
const LAB_PROFILE_TEMPLATE: &[u8; 572] = include_bytes!("data/lab-identity-profile.icc");

#[repr(align(2))]
struct AlignedLut([u8; LUT_BYTES]);

static LUT_BYTES_LE: AlignedLut = AlignedLut(*include_bytes!("data/lcms-lab-lut.u16le"));

#[derive(Clone, Copy)]
struct Axis {
    cell: usize,
    fraction: i64,
    step: usize,
}

const fn make_axes() -> [Axis; 256] {
    let mut axes = [Axis {
        cell: 0,
        fraction: 0,
        step: 1,
    }; 256];
    let mut value = 0usize;
    while value < axes.len() {
        // LittleCMS represents the byte sample on its 15.16 CLUT coordinate
        // grid. The correction and all three fractions use integer math.
        let scaled = value as u32 * 257 * 32;
        let fixed = scaled + (scaled + 0x7fff) / 0xffff;
        let cell = (fixed >> 16) as usize;
        axes[value] = Axis {
            cell,
            fraction: (fixed & 0xffff) as i64,
            step: if value == 255 { 0 } else { 1 },
        };
        value += 1;
    }
    axes
}

const AXES: [Axis; 256] = make_axes();

/// Build the profile Pillow's `ImageCms.createProfile("LAB")` attaches.
/// LittleCMS stamps profile creation time at whole-second precision; preserve
/// that observable field while keeping the profile bytes otherwise static.
pub(crate) fn pillow_lab_icc_profile() -> Vec<u8> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let within_day = seconds % 86_400;
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };

    let date = [
        (year as u16).to_be_bytes(),
        (month as u16).to_be_bytes(),
        (day as u16).to_be_bytes(),
        ((within_day / 3_600) as u16).to_be_bytes(),
        (((within_day / 60) % 60) as u16).to_be_bytes(),
        ((within_day % 60) as u16).to_be_bytes(),
    ];
    let mut profile = LAB_PROFILE_TEMPLATE.to_vec();
    for (index, part) in date.into_iter().enumerate() {
        profile[24 + index * 2..26 + index * 2].copy_from_slice(&part);
    }
    profile
}

/// Return the RGB-to-LAB CLUT packed as two u32 words per grid vertex.
///
/// The first word stores L in its low half and A in its high half; the second
/// stores B in its low half. This reduces the device table from three storage
/// reads to two per vertex while preserving the exact 16-bit samples.
pub(crate) fn gpu_table_words() -> &'static [u32] {
    static WORDS: OnceLock<Box<[u32]>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let mut words = Vec::with_capacity(GPU_TABLE_WORDS);
        for vertex in 0..GRID * GRID * GRID {
            let value = vertex * CHANNELS;
            let l = lut_value(value) as u16;
            let a = lut_value(value + 1) as u16;
            let b = lut_value(value + 2) as u16;
            words.push(u32::from(l) | (u32::from(a) << 16));
            words.push(u32::from(b));
        }
        words.into_boxed_slice()
    })
}

/// Synthetic RGBA storage view used only to satisfy GPU auxiliary layout and
/// shape preflight. The GPU planner uploads the cached packed words directly.
pub(crate) fn gpu_table_image() -> Arc<DynamicImage> {
    static IMAGE: OnceLock<Arc<DynamicImage>> = OnceLock::new();
    Arc::clone(IMAGE.get_or_init(|| {
        let bytes = gpu_table_words()
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        let image = ImageBuffer::from_raw(GPU_TABLE_WORDS as u32, 1, bytes)
            .unwrap_or_else(|| unreachable!("fixed LAB table dimensions match its bytes"));
        Arc::new(DynamicImage::ImageRgba8(image))
    }))
}

#[cfg(target_endian = "little")]
#[inline]
fn lut() -> &'static [u16] {
    bytemuck::cast_slice(&LUT_BYTES_LE.0)
}

#[cfg(target_endian = "big")]
#[inline]
fn lut_value(index: usize) -> i64 {
    let start = index * 2;
    i64::from(u16::from_le_bytes([
        LUT_BYTES_LE.0[start],
        LUT_BYTES_LE.0[start + 1],
    ]))
}

#[cfg(target_endian = "little")]
#[inline]
fn lut_value(index: usize) -> i64 {
    i64::from(lut()[index])
}

/// Convert a tightly packed RGB8 buffer to Pillow's packed LAB bytes.
///
/// LAB shares RGB storage in `DynamicImage`; the A and B channels are stored
/// with Pillow's +128 bias and decoded at the public pixel/data boundary.
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

        // LittleCMS uses six tetrahedra with this tie-breaking order. Its
        // vertex order and 0x8001 bias are observable at byte boundaries.
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
            let a0 = lut_value(v0 + channel);
            let a1 = lut_value(v1 + channel);
            let a2 = lut_value(v2 + channel);
            let a3 = lut_value(v3 + channel);
            let rest = (a1 - a0) * fractions[p]
                + (a2 - a1) * fractions[q]
                + (a3 - a2) * fractions[t]
                + 0x8001;
            let lab_v4 = a0 + ((rest + (rest >> 16)) >> 16);

            // Pillow's generic three-channel packer quantizes the 16-bit
            // LabV4 values to bytes, then removes the 128 bias from A and B.
            let packed = ((lab_v4 as u32 * 65281 + 8_388_608) >> 24) as u8;
            target[channel] = if channel == 0 {
                packed
            } else {
                packed.wrapping_sub(128)
            };
        }
    }
    Ok(output)
}

/// Convert an RGB8 raster to the byte representation used by Pillow's LAB.
pub(crate) fn convert_rgb_image(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    let (width, height) = img.dimensions();
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::InternalError("LAB image size overflow".into()))?;
    let output = match img {
        DynamicImage::ImageRgb8(rgb) => convert_rgb_bytes(rgb.as_raw(), pixel_count)?,
        _ => {
            let rgb = img.to_rgb8();
            convert_rgb_bytes(rgb.as_raw(), pixel_count)?
        }
    };
    let image: RgbImage = ImageBuffer::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("LAB output buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageRgb8(image))
}

#[cfg(test)]
mod tests {
    use super::convert_rgb_bytes;

    #[test]
    fn matches_pillow_lab_byte_examples() {
        let colors = [
            ([0, 0, 0], [0, 0, 0]),
            ([255, 255, 255], [255, 0, 0]),
            ([128, 128, 128], [137, 0, 0]),
            ([255, 0, 0], [138, 81, 70]),
            ([0, 255, 0], [224, 177, 81]),
            ([0, 0, 255], [75, 68, 144]),
            ([12, 34, 56], [32, 254, 239]),
            ([255, 0, 255], [153, 94, 195]),
            ([0, 255, 255], [231, 205, 241]),
            ([255, 255, 0], [249, 240, 93]),
        ];
        for (source, expected) in colors {
            assert_eq!(convert_rgb_bytes(&source, 1).unwrap(), expected);
        }
    }
}

//! Shared LittleCMS RGB-to-LAB lookup-table data and profile metadata.
//!
//! Pillow routes this conversion through LittleCMS's default sRGB-to-LabV4
//! transform and then packs the LabV2 byte representation.  The bundled
//! 33³ CLUT is the exact LittleCMS transform table. CPU interpolation lives
//! with the mode-specific Color strategy; GPU table packing stays here because
//! it reads the same immutable CLUT vertices.

use crate::raster::{DynamicImage, ImageBuffer};
#[cfg(feature = "gpu")]
use std::sync::{Arc, OnceLock};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const GRID: usize = 33;
const CHANNELS: usize = 3;
const LUT_VALUES: usize = GRID * GRID * GRID * CHANNELS;
const LUT_BYTES: usize = LUT_VALUES * 2;
#[cfg(feature = "gpu")]
pub(crate) const GPU_TABLE_WORDS: usize = GRID * GRID * GRID * 2;
const LAB_PROFILE_TEMPLATE: &[u8; 572] = include_bytes!("data/lab-identity-profile.icc");

#[cfg(target_arch = "wasm32")]
fn unix_timestamp_seconds() -> u64 {
    // `std::time::SystemTime::now()` is unsupported on `wasm32-unknown-unknown`.
    // Date.now() is available in both browser and Node hosts and preserves the
    // whole-second timestamp that LittleCMS writes into the profile header.
    let milliseconds = js_sys::Date::now();
    if milliseconds.is_finite() && milliseconds >= 0.0 {
        (milliseconds / 1_000.0) as u64
    } else {
        0
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[repr(align(2))]
struct AlignedLut([u8; LUT_BYTES]);

static LUT_BYTES_LE: AlignedLut = AlignedLut(*include_bytes!("data/lcms-lab-lut.u16le"));

/// Build the profile Pillow's `ImageCms.createProfile("LAB")` attaches.
/// LittleCMS stamps profile creation time at whole-second precision; preserve
/// that observable field while keeping the profile bytes otherwise static.
pub(crate) fn pillow_lab_icc_profile() -> Vec<u8> {
    let seconds = unix_timestamp_seconds();
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
#[cfg(feature = "gpu")]
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
#[cfg(feature = "gpu")]
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
pub(crate) fn lut_value(index: usize) -> i64 {
    let start = index * 2;
    i64::from(u16::from_le_bytes([
        LUT_BYTES_LE.0[start],
        LUT_BYTES_LE.0[start + 1],
    ]))
}

#[cfg(target_endian = "little")]
#[inline]
pub(crate) fn lut_value(index: usize) -> i64 {
    i64::from(lut()[index])
}


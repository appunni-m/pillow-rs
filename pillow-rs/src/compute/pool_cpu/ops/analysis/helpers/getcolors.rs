//! Native-sample color counting shared by exact-mode GetColors owners.

use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::image::color_count::{
    PillowColorCounts, RgbGrayscaleColorCounts, SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS,
    ScalarColorDomainCounts, count_pillow_color_scalar_samples, getcolors_rgb8_grayscale,
    getcolors_scalar_byte_domain, getcolors_scalar_i_repeated_runs, pillow_color_components,
    pillow_color_direct_table_slots, pillow_color_pixel, pillow_color_table_parameters,
    scalar_byte_domain_is_plausible, scalar_i_prefix_has_long_run,
    scalar_prefix_has_repeated_samples,
};
use crate::raster::{GrayImage, ImageBuffer, Mode, Pixel, RgbImage, RgbaImage};
use std::collections::HashMap;

pub(crate) type GetColorsOutput = Option<Vec<(u32, FormattedPixelValue)>>;

/// Counts exact L, One, and P samples in ascending byte order.
pub(crate) fn luma8(maxcolors: u32, image: &GrayImage) -> Result<GetColorsOutput, PilError> {
    let mut histogram = [0u32; 256];
    let mut distinct = 0usize;
    let limit = maxcolors as usize;
    for pixel in image.pixels() {
        let count = &mut histogram[pixel[0] as usize];
        if *count == 0 {
            distinct += 1;
            if distinct > limit {
                return Ok(None);
            }
        }
        *count += 1;
    }
    Ok(Some(
        histogram
            .into_iter()
            .enumerate()
            .filter_map(|(value, count)| {
                (count > 0).then_some((count, FormattedPixelValue::Scalar(value as u8)))
            })
            .collect(),
    ))
}

/// Counts native byte channels using Pillow's hash slot and probe ordering.
pub(crate) fn byte_bands<P>(
    maxcolors: u32,
    image: &ImageBuffer<P, Vec<u8>>,
) -> Result<GetColorsOutput, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let (code_mask, code_polynomial) = color_table_parameters(maxcolors)?;
    let pixel_count = image_pixel_count(image);
    byte_bands_with_table(maxcolors, image, code_mask, code_polynomial, pixel_count)
}

/// Keeps Pillow's RGB grayscale shortcut on its exact RGB8 owner.
pub(crate) fn rgb8(maxcolors: u32, image: &RgbImage) -> Result<GetColorsOutput, PilError> {
    let (code_mask, code_polynomial) = color_table_parameters(maxcolors)?;
    let pixel_count = image_pixel_count(image);
    if maxcolors > 1023 && pixel_count >= 64 * 1024 {
        match getcolors_rgb8_grayscale(image, maxcolors as usize, code_mask, code_polynomial) {
            RgbGrayscaleColorCounts::NotGrayscale => {}
            RgbGrayscaleColorCounts::TooManyColors => return Ok(None),
            RgbGrayscaleColorCounts::Colors(colors) => return Ok(Some(colors)),
        }
    }
    byte_bands_with_table(maxcolors, image, code_mask, code_polynomial, pixel_count)
}

fn color_table_parameters(maxcolors: u32) -> Result<(u32, u32), PilError> {
    pillow_color_table_parameters(maxcolors).ok_or_else(|| PilError::MemoryError(String::new()))
}

fn image_pixel_count<P, Container>(image: &ImageBuffer<P, Container>) -> usize
where
    P: Pixel,
    Container: std::ops::Deref<Target = [P::Subpixel]>,
{
    usize::try_from(image.width())
        .ok()
        .and_then(|width| {
            usize::try_from(image.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .unwrap_or(0)
}

fn byte_bands_with_table<P>(
    maxcolors: u32,
    image: &ImageBuffer<P, Vec<u8>>,
    code_mask: u32,
    code_polynomial: u32,
    pixel_count: usize,
) -> Result<GetColorsOutput, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let bands = usize::from(P::CHANNEL_COUNT);
    let max_distinct = maxcolors as usize;
    let initial_capacity = if maxcolors <= 256 && image.as_raw().len() >= 64 * 1024 {
        max_distinct + 1
    } else {
        0
    };
    let mut counts = PillowColorCounts::new(code_mask, initial_capacity, pixel_count);
    let mut distinct = 0usize;
    for pixel in image.pixels() {
        if !counts.count(
            &mut distinct,
            pillow_color_pixel(pixel.channels()),
            max_distinct,
            code_mask,
            code_polynomial,
        ) {
            return Ok(None);
        }
    }
    Ok(Some(
        counts
            .into_slot_order(distinct)
            .into_iter()
            .map(|(_, (pixel, count))| (count, pillow_color_components(pixel, bands)))
            .collect(),
    ))
}

/// Counts native I samples and formats them as signed integers.
pub(crate) fn scalar_i32(
    maxcolors: u32,
    image: &RgbaImage,
) -> Result<GetColorsOutput, PilError> {
    scalar(maxcolors, Mode::ImageI32, image.as_raw())
}

/// Counts native F samples and formats their original f32 values.
pub(crate) fn scalar_f32(
    maxcolors: u32,
    image: &RgbaImage,
) -> Result<GetColorsOutput, PilError> {
    scalar(maxcolors, Mode::ImageF32, image.as_raw())
}

fn scalar(maxcolors: u32, mode: Mode, bytes: &[u8]) -> Result<GetColorsOutput, PilError> {
    let (code_mask, code_polynomial) = color_table_parameters(maxcolors)?;
    let max_distinct = maxcolors as usize;
    let pixel_count = bytes.len() / 4;
    let has_repeated_prefix = if mode == Mode::ImageI32 {
        scalar_i_prefix_has_long_run(bytes)
    } else {
        scalar_prefix_has_repeated_samples(bytes)
    };
    match scalar_color_domain_counts(
        mode,
        bytes,
        max_distinct,
        code_mask,
        code_polynomial,
        pixel_count,
        has_repeated_prefix,
    ) {
        ScalarColorDomainCounts::NotApplicable => {}
        ScalarColorDomainCounts::TooManyColors => return Ok(None),
        ScalarColorDomainCounts::Colors(colors) => return Ok(Some(colors)),
    }
    let initial_capacity = if maxcolors <= 256 && bytes.len() >= 64 * 1024 {
        max_distinct + 1
    } else {
        0
    };
    let mut counts = PillowColorCounts::new(code_mask, initial_capacity, pixel_count);
    let mut distinct = 0usize;
    let counted = if has_repeated_prefix {
        count_repeated_scalar_samples(
            &mut counts,
            &mut distinct,
            bytes,
            max_distinct,
            code_mask,
            code_polynomial,
        )
    } else {
        count_pillow_color_scalar_samples(
            &mut counts,
            &mut distinct,
            bytes,
            max_distinct,
            code_mask,
            code_polynomial,
        )
    };
    if !counted {
        return Ok(None);
    }
    Ok(Some(format_scalar_color_counts(counts, distinct, mode)))
}

fn scalar_color_domain_counts(
    mode: Mode,
    bytes: &[u8],
    max_distinct: usize,
    code_mask: u32,
    code_polynomial: u32,
    pixel_count: usize,
    has_repeated_prefix: bool,
) -> ScalarColorDomainCounts {
    let direct_table_slots = pillow_color_direct_table_slots(code_mask, pixel_count);
    if mode == Mode::ImageI32
        && pixel_count >= 4096
        && has_repeated_prefix
        && direct_table_slots.is_some_and(|slots| slots > SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS)
        && scalar_byte_domain_is_plausible(Mode::ImageI32, bytes)
    {
        match getcolors_scalar_i_repeated_runs(bytes, max_distinct, code_mask, code_polynomial) {
            ScalarColorDomainCounts::NotApplicable => {}
            result => return result,
        }
    }
    if pixel_count >= 4096
        && !has_repeated_prefix
        && scalar_byte_domain_is_plausible(mode, bytes)
    {
        match getcolors_scalar_byte_domain(mode, bytes, max_distinct, code_mask, code_polynomial) {
            ScalarColorDomainCounts::NotApplicable => {}
            result => return result,
        }
    }
    ScalarColorDomainCounts::NotApplicable
}

#[inline(always)]
fn count_repeated_scalar_samples(
    counts: &mut PillowColorCounts,
    distinct: &mut usize,
    bytes: &[u8],
    max_distinct: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> bool {
    let mut samples = bytes.chunks_exact(4).peekable();
    while let Some(sample) = samples.next() {
        let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
        let mut repetitions = 1usize;
        while let Some(next) = samples.peek() {
            if *next != sample {
                break;
            }
            samples.next();
            repetitions += 1;
        }
        if repetitions == 1 {
            if !counts.count(distinct, pixel, max_distinct, code_mask, code_polynomial) {
                return false;
            }
            continue;
        }
        match counts.count_repeated(
            distinct,
            pixel,
            repetitions,
            max_distinct,
            code_mask,
            code_polynomial,
        ) {
            Some(true) => {}
            Some(false) => return false,
            None => {
                for _ in 0..repetitions {
                    if !counts.count(distinct, pixel, max_distinct, code_mask, code_polynomial) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn format_scalar_color_counts(
    counts: PillowColorCounts,
    distinct: usize,
    mode: Mode,
) -> Vec<(u32, FormattedPixelValue)> {
    counts
        .into_slot_order(distinct)
        .into_iter()
        .map(|(_, (pixel, count))| {
            let value = if mode == Mode::ImageI32 {
                FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes()))
            } else {
                FormattedPixelValue::Float(f32::from_bits(pixel) as f64)
            };
            (count, value)
        })
        .collect()
}

// NO_PILLOW_SUPPORT: native high-depth colors retain their sample width.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct NativeColorKey {
    bands: u8,
    samples: [u32; 4],
}

pub(crate) fn u16_bands<P>(
    maxcolors: u32,
    image: &ImageBuffer<P, Vec<u16>>,
) -> Result<GetColorsOutput, PilError>
where
    P: Pixel<Subpixel = u16>,
{
    color_table_parameters(maxcolors)?;
    native_bands(maxcolors, image, u32::from, |samples| {
        FormattedPixelValue::U16Components(samples.to_vec())
    })
}

pub(crate) fn f32_bands<P>(
    maxcolors: u32,
    image: &ImageBuffer<P, Vec<f32>>,
) -> Result<GetColorsOutput, PilError>
where
    P: Pixel<Subpixel = f32>,
{
    color_table_parameters(maxcolors)?;
    native_bands(maxcolors, image, f32::to_bits, |samples| {
        FormattedPixelValue::FloatComponents(samples.to_vec())
    })
}

fn native_bands<S, P, Key, Format>(
    maxcolors: u32,
    image: &ImageBuffer<P, Vec<S>>,
    key_sample: Key,
    format: Format,
) -> Result<GetColorsOutput, PilError>
where
    S: Copy,
    P: Pixel<Subpixel = S>,
    Key: Fn(S) -> u32,
    Format: Fn(&[S]) -> FormattedPixelValue,
{
    let limit = maxcolors as usize;
    let initial_capacity = limit.min(image_pixel_count(image)).min(4096);
    let mut indexes = HashMap::<NativeColorKey, usize>::new();
    indexes
        .try_reserve(initial_capacity)
        .map_err(|_| PilError::MemoryError(String::new()))?;
    let mut colors = Vec::<(FormattedPixelValue, u32)>::new();
    colors
        .try_reserve_exact(initial_capacity)
        .map_err(|_| PilError::MemoryError(String::new()))?;
    let bands = P::CHANNEL_COUNT;
    for pixel in image.pixels() {
        let samples = pixel.channels();
        let mut key = NativeColorKey { bands, samples: [0; 4] };
        for (index, &sample) in samples.iter().enumerate() {
            key.samples[index] = key_sample(sample);
        }
        if let Some(&index) = indexes.get(&key) {
            colors[index].1 += 1;
            continue;
        }
        if colors.len() >= limit {
            return Ok(None);
        }
        indexes
            .try_reserve(1)
            .map_err(|_| PilError::MemoryError(String::new()))?;
        colors
            .try_reserve(1)
            .map_err(|_| PilError::MemoryError(String::new()))?;
        let index = colors.len();
        indexes.insert(key, index);
        colors.push((format(samples), 1));
    }
    Ok(Some(
        colors
            .into_iter()
            .map(|(color, count)| (count, color))
            .collect(),
    ))
}

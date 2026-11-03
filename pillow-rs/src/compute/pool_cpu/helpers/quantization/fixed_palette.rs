//! Mapping to caller-supplied palettes.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

use super::RgbPixelSource;

/// Return the first palette entry with the smallest squared RGB distance.
///
/// Keeping the scan order for ties matches Pillow's palette-index preference
/// and avoids making palette order an accidental implementation detail.
fn nearest_palette_index(pixel: [u8; 3], palette: &[[u8; 3]]) -> u8 {
    let mut best_index = 0usize;
    let mut best_distance = u32::MAX;
    for (index, entry) in palette.iter().enumerate() {
        let dr = i32::from(pixel[0]) - i32::from(entry[0]);
        let dg = i32::from(pixel[1]) - i32::from(entry[1]);
        let db = i32::from(pixel[2]) - i32::from(entry[2]);
        let distance = (dr * dr + dg * dg + db * db) as u32;
        if distance < best_distance {
            best_distance = distance;
            best_index = index;
        }
    }
    u8::try_from(best_index).unwrap_or(0)
}

/// Map source RGB samples to a fixed palette with Pillow's error diffusion.
pub(crate) fn map_pixels_to_fixed_palette(
    pixels: &[u8],
    width: u32,
    height: u32,
    palette: &[[u8; 3]],
    dither: bool,
) -> Result<Vec<u8>, PilError> {
    map_fixed_palette_source(pixels, width, height, palette, dither)
}

/// Maps native L samples to an RGB palette without constructing an RGB raster.
pub(crate) fn map_luma_to_fixed_palette(
    pixels: &[u8],
    width: u32,
    height: u32,
    palette: &[[u8; 3]],
    dither: bool,
) -> Result<Vec<u8>, PilError> {
    let dimensions = CheckedDims::new(width, height, 1)?;
    if pixels.len() != dimensions.total_pixels() {
        return Err(PilError::InternalError(
            "fixed palette L input buffer shape mismatch".to_owned(),
        ));
    }
    map_fixed_palette_source(&LumaSamples(pixels), width, height, palette, dither)
}

fn map_fixed_palette_source<S>(
    pixels: &S,
    width: u32,
    height: u32,
    palette: &[[u8; 3]],
    dither: bool,
) -> Result<Vec<u8>, PilError>
where
    S: RgbPixelSource + ?Sized,
{
    let dims = CheckedDims::new(width, height, 1)?;
    let mut output = dims.alloc_buffer();
    if palette.is_empty() {
        return Ok(output);
    }
    let width = width as usize;

    if !dither {
        for index in 0..pixels.pixel_count() {
            output[index] = nearest_palette_index(pixels.rgb_at(index), palette);
        }
        return Ok(output);
    }

    // Match Pillow's topalette accumulator layout: one extra column and a
    // single division by 16 when the next pixel reads its propagated error.
    let mut errors = vec![0i32; (width + 1) * 3];
    for y in 0..height as usize {
        let mut carry = [0i32; 3];
        let mut delayed_five = [0i32; 3];
        let mut delayed_one = [0i32; 3];
        for x in 0..width {
            let error = x * 3;
            let source_pixel = pixels.rgb_at(y * width + x);
            let mut corrected = [0u8; 3];
            for channel in 0..3 {
                corrected[channel] = clip_palette_component(
                    i32::from(source_pixel[channel])
                        + (carry[channel] + errors[error + 3 + channel]) / 16,
                );
                carry[channel] = i32::from(corrected[channel]);
            }

            let palette_index = nearest_palette_index(corrected, palette);
            output[y * width + x] = palette_index;
            let selected = palette[usize::from(palette_index)];
            for channel in 0..3 {
                carry[channel] -= i32::from(selected[channel]);
                let original_error = carry[channel];
                let doubled = original_error + original_error;
                carry[channel] = original_error + doubled;
                errors[error + channel] = carry[channel] + delayed_five[channel];
                carry[channel] += doubled;
                delayed_five[channel] = carry[channel] + delayed_one[channel];
                delayed_one[channel] = original_error;
                carry[channel] += doubled;
            }
        }

        // Pillow stores only the blue-channel edge accumulators at the extra
        // column; preserve that observable edge behavior for fixed palettes.
        let edge = width * 3;
        errors[edge] = delayed_five[2];
        errors[edge + 1] = delayed_one[2];
        errors[edge + 2] = delayed_one[2];
    }
    Ok(output)
}

struct LumaSamples<'a>(&'a [u8]);

impl RgbPixelSource for LumaSamples<'_> {
    #[inline(always)]
    fn pixel_count(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    fn rgb_at(&self, index: usize) -> [u8; 3] {
        let sample = self.0[index];
        [sample, sample, sample]
    }
}

#[inline(always)]
fn clip_palette_component(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

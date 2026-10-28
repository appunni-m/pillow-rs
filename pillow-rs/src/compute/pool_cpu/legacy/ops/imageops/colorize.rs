use super::*;

pub(crate) fn gather_native_channel<const CHANNELS: usize, const CHANNEL: usize>(
    source: &[u8],
) -> (Vec<u8>, u64, u64) {
    #[cfg(target_arch = "x86_64")]
    {
        let pixel_count = source.len() / CHANNELS;
        if let Some(gathered) =
            crate::color::gather_interleaved_channel_ssse3_bytes(source, CHANNELS, CHANNEL)
        {
            return gathered;
        }

        // The portable wide shuffle lowers to scalar code on baseline x86_64
        // builds without SSSE3. Keep that path honestly scalar in telemetry.
        let output = source
            .chunks_exact(CHANNELS)
            .map(|pixel| pixel[CHANNEL])
            .collect();
        return (output, 0, pixel_count as u64);
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        const LANES: usize = 16;
        let pixel_count = source.len() / CHANNELS;
        let selectors: [u8x16; CHANNELS] = std::array::from_fn(|_block| {
            u8x16::new(std::array::from_fn(|lane| {
                ((lane * CHANNELS + CHANNEL) % LANES) as u8
            }))
        });
        let masks: [u8x16; CHANNELS] = std::array::from_fn(|block| {
            u8x16::new(std::array::from_fn(|lane| {
                if (lane * CHANNELS + CHANNEL) / LANES == block {
                    u8::MAX
                } else {
                    0
                }
            }))
        });

        let mut output = Vec::with_capacity(pixel_count);
        let vector_pixels = pixel_count / LANES * LANES;
        for pixels in source[..vector_pixels * CHANNELS].chunks_exact(CHANNELS * LANES) {
            let input_vectors: [u8x16; CHANNELS] = std::array::from_fn(|block| {
                let mut lanes = [0u8; LANES];
                let start = block * LANES;
                lanes.copy_from_slice(&pixels[start..start + LANES]);
                u8x16::new(lanes)
            });
            let mut selected = u8x16::splat(0);
            for block in 0..CHANNELS {
                selected |= input_vectors[block].swizzle_relaxed(selectors[block]) & masks[block];
            }
            output.extend_from_slice(&selected.to_array());
        }

        for pixel in source[vector_pixels * CHANNELS..].chunks_exact(CHANNELS) {
            output.push(pixel[CHANNEL]);
        }
        debug_assert_eq!(output.len(), pixel_count);
        (
            output,
            (vector_pixels / LANES) as u64,
            (pixel_count - vector_pixels) as u64,
        )
    }
}

/// Colorize: map grayscale values to a two-color gradient.
/// Always outputs RGB (PIL behavior).
///
/// PIL builds a 256-entry LUT per channel in ``ImageOps.colorize`` using
/// floor integer division (``//``), then applies it via ``ImageOps._lut``.
/// The mapping supports optional three-color ``mid`` plus blackpoint /
/// midpoint / whitepoint positions; this is replicated exactly so negative
/// color deltas round the same way.
pub fn colorize_lut(
    black: &(u8, u8, u8),
    white: &(u8, u8, u8),
    mid: Option<(u8, u8, u8)>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
) -> [[u8; 256]; 3] {
    let mut lut = [[0u8; 256]; 3];
    for channel in 0..3 {
        let black_c = [black.0, black.1, black.2][channel] as i32;
        let white_c = [white.0, white.1, white.2][channel] as i32;
        let mid_c = mid.map(|m| [m.0, m.1, m.2][channel] as i32);
        let bp = blackpoint as i32;
        let mp = midpoint as i32;
        let wp = whitepoint as i32;
        for (index, slot) in lut[channel].iter_mut().enumerate() {
            let index = index as i32;
            let value = if index < bp {
                black_c
            } else if let Some(mid_c) = mid_c {
                if index < mp {
                    let span = mp - bp;
                    let step = if span == 0 {
                        0
                    } else {
                        ((index - bp) * (mid_c - black_c)).div_euclid(span)
                    };
                    black_c + step
                } else if index < wp {
                    let span = wp - mp;
                    let step = if span == 0 {
                        0
                    } else {
                        ((index - mp) * (white_c - mid_c)).div_euclid(span)
                    };
                    mid_c + step
                } else {
                    white_c
                }
            } else if index < wp {
                let span = wp - bp;
                let step = if span == 0 {
                    0
                } else {
                    ((index - bp) * (white_c - black_c)).div_euclid(span)
                };
                black_c + step
            } else {
                white_c
            };
            *slot = value.clamp(0, 255) as u8;
        }
    }
    lut
}

pub fn op_colorize(
    img: &DynamicImage,
    black: &(u8, u8, u8),
    white: &(u8, u8, u8),
    mid: Option<(u8, u8, u8)>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
) -> Result<DynamicImage, PilError> {
    // L is already the exact input domain for ImageOps.colorize. Borrow its
    // samples instead of cloning the whole grayscale image; all other modes
    // retain the established Pillow-compatible conversion path.
    let gray: std::borrow::Cow<'_, [u8]> = match img {
        DynamicImage::ImageLuma8(gray) => std::borrow::Cow::Borrowed(gray.as_raw()),
        _ => std::borrow::Cow::Owned(img.to_luma8().into_raw()),
    };
    let (w, h) = (img.width(), img.height());
    let lut = colorize_lut(black, white, mid, blackpoint, midpoint, whitepoint);
    let output_len = gray
        .len()
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("colorize output is too large".into()))?;
    let mut output = Vec::with_capacity(output_len);
    for &sample in gray.iter() {
        let index = usize::from(sample);
        output.extend_from_slice(&[lut[0][index], lut[1][index], lut[2][index]]);
    }
    // Colorize always outputs RGB (PIL behavior)
    crate::raster::RgbImage::from_raw(w, h, output)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("colorize buffer mismatch".into()))
}

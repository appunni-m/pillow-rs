//! Native byte-channel implementation for the direct Pillow ModeFilter.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn execute<P, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    size: u32,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), CHANNELS as u8)?;
    if image.as_raw().len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "ModeFilter source bytes do not match its typed channel layout".into(),
        ));
    }
    let output = mode_filter_native_bytes::<CHANNELS>(
        image.as_raw(),
        dimensions.width as usize,
        dimensions.height as usize,
        size,
    );
    ImageBuffer::from_raw(dimensions.width, dimensions.height, output)
        .ok_or_else(|| PilError::InternalError("ModeFilter output shape mismatch".into()))
}

pub(super) fn mode_filter_native_bytes<const CHANNELS: usize>(
    raw: &[u8],
    width: usize,
    height: usize,
    size: u32,
) -> Vec<u8> {
    let mut output = vec![0u8; width * height * CHANNELS];
    let width = width as i32;
    let height = height as i32;
    let half = (size / 2) as i32;

    for y in 0..height {
        for x in 0..width {
            let mut histograms = [[0u32; 256]; CHANNELS];
            accumulate_neighborhood::<CHANNELS>(
                raw,
                &mut histograms,
                (width, height),
                (x, y),
                half,
            );
            write_mode_pixel::<CHANNELS>(raw, &mut output, &histograms, width, (x, y));
        }
    }
    output
}

fn accumulate_neighborhood<const CHANNELS: usize>(
    raw: &[u8],
    histograms: &mut [[u32; 256]; CHANNELS],
    shape: (i32, i32),
    center: (i32, i32),
    half: i32,
) {
    let (width, height) = shape;
    let (x, y) = center;
    for dy in -half..=half {
        let source_y = y + dy;
        if source_y < 0 || source_y >= height {
            continue;
        }
        for dx in -half..=half {
            let source_x = x + dx;
            if source_x < 0 || source_x >= width {
                continue;
            }
            let base = ((source_y * width + source_x) as usize) * CHANNELS;
            for (channel, histogram) in histograms.iter_mut().enumerate() {
                histogram[raw[base + channel] as usize] += 1;
            }
        }
    }
}

fn write_mode_pixel<const CHANNELS: usize>(
    raw: &[u8],
    output: &mut [u8],
    histograms: &[[u32; 256]; CHANNELS],
    width: i32,
    center: (i32, i32),
) {
    let (x, y) = center;
    let pixel = ((y * width + x) as usize) * CHANNELS;
    for (channel, histogram) in histograms.iter().enumerate() {
        let (mode, max_count) = find_mode_with_count(histogram);
        let original = raw[pixel + channel];
        output[pixel + channel] = if max_count > 2 { mode } else { original };
    }
}

fn find_mode_with_count(histogram: &[u32; 256]) -> (u8, u32) {
    let mut mode = 0u8;
    let mut max_count = histogram[0];
    for (value, &count) in histogram.iter().enumerate().skip(1) {
        if count > max_count {
            max_count = count;
            mode = value as u8;
        }
    }
    (mode, max_count)
}

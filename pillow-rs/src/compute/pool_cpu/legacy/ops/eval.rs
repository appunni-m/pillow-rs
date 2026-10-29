use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

#[path = "eval/tests.rs"]
#[cfg(test)]
mod tests;

/// Apply a byte LUT directly in the image's native storage layout.
///
/// The generic Eval implementation widens multi-band images to RGBA before
/// applying the table. This path maps each native byte sample directly into
/// one output allocation, avoiding both that conversion and a clone-then-map
/// pass. Keep it limited to the four native byte layouts and exact table
/// lengths; typed, palette, and malformed descriptors retain the established
/// implementation.
fn map_native_byte_lut<const CHANNELS: usize>(source: &[u8], lut: &[u8]) -> Option<Vec<u8>> {
    if CHANNELS == 0
        || lut.len() != CHANNELS.checked_mul(256)?
        || !source.len().is_multiple_of(CHANNELS)
    {
        return None;
    }
    let mut affine = [(0u8, 0u8); CHANNELS];
    let mut all_affine = true;
    for channel in 0..CHANNELS {
        let start = channel * 256;
        let Some(coefficients) =
            crate::compute::byte_lut_affine_coefficients(&lut[start..start + 256])
        else {
            all_affine = false;
            break;
        };
        affine[channel] = coefficients;
    }
    let mut output = Vec::with_capacity(source.len());
    if all_affine {
        for pixel in source.chunks_exact(CHANNELS) {
            for (channel, &sample) in pixel.iter().enumerate() {
                let (multiplier, offset) = affine[channel];
                output.push(sample.wrapping_mul(multiplier).wrapping_add(offset));
            }
        }
        return Some(output);
    }
    match CHANNELS {
        1 => {
            for &sample in source {
                output.push(lut[usize::from(sample)]);
            }
        }
        2 => {
            for pixel in source.chunks_exact(2) {
                output.push(lut[usize::from(pixel[0])]);
                output.push(lut[256 + usize::from(pixel[1])]);
            }
        }
        3 => {
            for pixel in source.chunks_exact(3) {
                output.push(lut[usize::from(pixel[0])]);
                output.push(lut[256 + usize::from(pixel[1])]);
                output.push(lut[512 + usize::from(pixel[2])]);
            }
        }
        4 => {
            for pixel in source.chunks_exact(4) {
                output.push(lut[usize::from(pixel[0])]);
                output.push(lut[256 + usize::from(pixel[1])]);
                output.push(lut[512 + usize::from(pixel[2])]);
                output.push(lut[768 + usize::from(pixel[3])]);
            }
        }
        _ => return None,
    }
    Some(output)
}

fn eval_native_byte_lut(img: &DynamicImage, lut: &[u8]) -> Option<DynamicImage> {
    match img {
        DynamicImage::ImageLuma8(gray) if lut.len() == 256 => {
            let output = map_native_byte_lut::<1>(gray.as_raw(), lut)?;
            Some(DynamicImage::ImageLuma8(GrayImage::from_raw(
                gray.width(),
                gray.height(),
                output,
            )?))
        }
        DynamicImage::ImageLumaA8(la) if lut.len() == 512 => {
            let output = map_native_byte_lut::<2>(la.as_raw(), lut)?;
            Some(DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(
                la.width(),
                la.height(),
                output,
            )?))
        }
        DynamicImage::ImageRgb8(rgb) if lut.len() == 768 => {
            let output = map_native_byte_lut::<3>(rgb.as_raw(), lut)?;
            Some(DynamicImage::ImageRgb8(RgbImage::from_raw(
                rgb.width(),
                rgb.height(),
                output,
            )?))
        }
        DynamicImage::ImageRgba8(rgba) if lut.len() == 1024 => {
            let output = map_native_byte_lut::<4>(rgba.as_raw(), lut)?;
            Some(DynamicImage::ImageRgba8(RgbaImage::from_raw(
                rgba.width(),
                rgba.height(),
                output,
            )?))
        }
        _ => None,
    }
}

pub fn op_eval(img: &DynamicImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    let n_bands = match img.color() {
        crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
        crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
        crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
        _ => 4,
    };
    // `image_eval_validated` checks the exact band length before queuing the
    // operation, and internal PointOp fusion constructs complete tables. A
    // malformed Eval descriptor is outside the supported public input
    // boundary, so the executor does not duplicate that validation.
    if let Some(output) = eval_native_byte_lut(img, lut) {
        return Ok(output);
    }
    // For single-channel images (mode "1", "L", "P"), operate on Luma8 directly
    // to avoid precision loss through RGBA round-trip.
    if n_bands == 1 {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            let idx = ip[0] as usize;
            op[0] = lut[idx];
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbaImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        for b in 0..4 {
            let idx = ip[b] as usize;
            let band = b.min(n_bands - 1);
            op[b] = lut[band * 256 + idx];
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgba8(out)))
}

// ── EffectNoise ──

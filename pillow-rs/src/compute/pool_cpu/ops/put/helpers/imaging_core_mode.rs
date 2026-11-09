//! Native storage paths for Pillow's callback-visible ImagingCore setmode.

use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, GrayImage, Mode, RgbImage, RgbaImage};

const WRONG_MODE: &str = "image has wrong mode";

pub(crate) fn set_mode(image: &mut DynamicImage, target: Mode) -> Result<(), PilError> {
    let source = image.mode();
    if source == target {
        return Ok(());
    }
    match (source, target) {
        (Mode::One, Mode::ImageLuma8) => one_to_luma(image),
        (Mode::ImageLuma8, Mode::One) => luma_to_one(image),
        (Mode::ImageRgb8, Mode::ImageRgba8 | Mode::ImageRgbx8) => {
            rgb_to_four_byte_mode(image, target)
        }
        (Mode::ImageRgba8 | Mode::ImageRgbx8, Mode::ImageRgb8) => four_byte_to_rgb(image),
        (Mode::ImageRgba8, Mode::ImageRgbx8) => rgba_to_rgbx(image),
        (Mode::ImageRgbx8, Mode::ImageRgba8) => rgbx_to_rgba(image),
        _ => Err(PilError::ValueError(WRONG_MODE.into())),
    }
}

fn one_to_luma(image: &mut DynamicImage) -> Result<(), PilError> {
    let mut pixels = match take_image(image) {
        DynamicImage::One(pixels) => pixels,
        source => {
            *image = source;
            return Err(PilError::InternalError(
                "ImagingCore 1-to-L received another raster identity".into(),
            ));
        }
    };
    for sample in pixels.as_mut() {
        *sample = if *sample == 0 { 0 } else { u8::MAX };
    }
    *image = DynamicImage::ImageLuma8(pixels);
    Ok(())
}

fn luma_to_one(image: &mut DynamicImage) -> Result<(), PilError> {
    let mut pixels = match take_image(image) {
        DynamicImage::ImageLuma8(pixels) => pixels,
        source => {
            *image = source;
            return Err(PilError::InternalError(
                "ImagingCore L-to-1 received another raster identity".into(),
            ));
        }
    };
    for sample in pixels.as_mut() {
        *sample = if *sample >= 128 { u8::MAX } else { 0 };
    }
    *image = DynamicImage::One(pixels);
    Ok(())
}

fn rgb_to_four_byte_mode(image: &mut DynamicImage, target: Mode) -> Result<(), PilError> {
    let DynamicImage::ImageRgb8(source) = image else {
        return Err(PilError::InternalError(
            "ImagingCore RGB reinterpretation received another raster identity".into(),
        ));
    };
    let width = source.width();
    let height = source.height();
    let pixel_count = checked_pixel_count(width, height)?;
    let mut output = reserved_bytes(pixel_count, 4)?;
    let hidden = source.hidden_pixel_lanes();
    for (index, sample) in source.as_raw().chunks_exact(3).enumerate() {
        output.extend_from_slice(sample);
        let lane = if target == Mode::ImageRgba8 {
            u8::MAX
        } else {
            // If the compact carrier never captured RGB's physical padding
            // lane, make the newly visible RGBX sample deterministic.
            hidden
                .and_then(|lanes| lanes.for_pixel(index))
                .and_then(|lanes| lanes.first())
                .copied()
                .unwrap_or(0)
        };
        output.push(lane);
    }
    let buffer = RgbaImage::from_raw(width, height, output).ok_or_else(|| {
        PilError::InternalError("ImagingCore RGB reinterpretation produced invalid storage".into())
    })?;
    *image = match target {
        Mode::ImageRgba8 => DynamicImage::ImageRgba8(buffer),
        Mode::ImageRgbx8 => DynamicImage::ImageRgbx8(buffer),
        _ => {
            return Err(PilError::InternalError(
                "ImagingCore RGB reinterpretation received an invalid target".into(),
            ));
        }
    };
    Ok(())
}

fn four_byte_to_rgb(image: &mut DynamicImage) -> Result<(), PilError> {
    let source = match image {
        DynamicImage::ImageRgba8(source) | DynamicImage::ImageRgbx8(source) => source,
        _ => {
            return Err(PilError::InternalError(
                "ImagingCore four-byte reinterpretation received another raster identity".into(),
            ));
        }
    };
    let width = source.width();
    let height = source.height();
    let pixel_count = checked_pixel_count(width, height)?;
    let mut rgb = reserved_bytes(pixel_count, 3)?;
    let mut hidden = Vec::new();
    hidden.try_reserve_exact(pixel_count).map_err(|_| {
        PilError::MemoryError("not enough memory for ImagingCore mode change".into())
    })?;
    for sample in source.as_raw().chunks_exact(4) {
        rgb.extend_from_slice(&sample[..3]);
        hidden.push(sample[3]);
    }
    let pixels = RgbImage::from_raw(width, height, rgb).ok_or_else(|| {
        PilError::InternalError("ImagingCore RGB reinterpretation produced invalid storage".into())
    })?;
    let hidden = HiddenPixelLanes::per_pixel(1, pixel_count, hidden).ok_or_else(|| {
        PilError::InternalError("ImagingCore RGB reinterpretation lost its fourth lane".into())
    })?;
    let mut output = DynamicImage::ImageRgb8(pixels);
    if !output.replace_hidden_pixel_lanes(Some(hidden)) {
        return Err(PilError::InternalError(
            "ImagingCore RGB reinterpretation could not retain its fourth lane".into(),
        ));
    }
    *image = output;
    Ok(())
}

fn rgba_to_rgbx(image: &mut DynamicImage) -> Result<(), PilError> {
    let pixels = match take_image(image) {
        DynamicImage::ImageRgba8(pixels) => pixels,
        source => {
            *image = source;
            return Err(PilError::InternalError(
                "ImagingCore RGBA-to-RGBX received another raster identity".into(),
            ));
        }
    };
    *image = DynamicImage::ImageRgbx8(pixels);
    Ok(())
}

fn rgbx_to_rgba(image: &mut DynamicImage) -> Result<(), PilError> {
    let mut pixels = match take_image(image) {
        DynamicImage::ImageRgbx8(pixels) => pixels,
        source => {
            *image = source;
            return Err(PilError::InternalError(
                "ImagingCore RGBX-to-RGBA received another raster identity".into(),
            ));
        }
    };
    for pixel in pixels.as_mut().chunks_exact_mut(4) {
        pixel[3] = u8::MAX;
    }
    *image = DynamicImage::ImageRgba8(pixels);
    Ok(())
}

fn take_image(image: &mut DynamicImage) -> DynamicImage {
    std::mem::replace(image, DynamicImage::One(GrayImage::new(0, 0)))
}

fn checked_pixel_count(width: u32, height: u32) -> Result<usize, PilError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::DimensionError("ImagingCore mode-change size overflow".into()))
}

fn reserved_bytes(pixel_count: usize, channels: usize) -> Result<Vec<u8>, PilError> {
    let byte_count = pixel_count.checked_mul(channels).ok_or_else(|| {
        PilError::DimensionError("ImagingCore mode-change byte count overflow".into())
    })?;
    let mut output = Vec::new();
    output.try_reserve_exact(byte_count).map_err(|_| {
        PilError::MemoryError("not enough memory for ImagingCore mode change".into())
    })?;
    Ok(output)
}

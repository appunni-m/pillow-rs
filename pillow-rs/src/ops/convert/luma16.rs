use crate::error::PilError;
use crate::image::Image;
use crate::raster::{DynamicImage, GenericImageView};

use super::mode::is_luma16_mode;

pub(super) fn try_convert_from_luma16(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    if !is_luma16_mode(source_mode)
        || !matches!(target_mode, "I" | "F" | "P" | "PA" | "HSV" | "YCbCr")
    {
        return Ok(None);
    }
    if source_mode != "I;16N" && matches!(target_mode, "I" | "F") {
        let source = image.materialized_shared()?;
        let DynamicImage::ImageLuma16(samples) = source.as_ref() else {
            return Err(PilError::InternalError(
                "invalid unsigned-16 storage".into(),
            ));
        };
        let mut result = crate::raster::RgbaImage::new(samples.width(), samples.height());
        for (output, sample) in result.pixels_mut().zip(samples.pixels()) {
            output.0 = if target_mode == "I" {
                i32::from(sample[0]).to_le_bytes()
            } else {
                f32::from(sample[0]).to_le_bytes()
            };
        }
        return Ok(Some(Image::from_dynamic(
            DynamicImage::ImageRgba8(result),
            Some(target_mode.to_owned()),
        )));
    }
    // Other destinations retry through clipped L; P uses identity gray indices.
    let luma = image.convert("L", None, None, None, None)?;
    Ok(Some(luma.convert(
        target_mode,
        None,
        dither,
        palette,
        colors,
    )?))
}

pub(super) fn try_convert_to_luma16(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
) -> Result<Option<Image>, PilError> {
    if !is_luma16_mode(target_mode) {
        return Ok(None);
    }
    if source_mode == "P" {
        return Err(PilError::ValueError("conversion not supported".into()));
    }
    let samples = if source_mode == "I" && target_mode != "I;16N" {
        convert_i32_to_luma16(image)?
    } else {
        convert_luma_to_luma16(image, source_mode)?
    };
    Ok(Some(Image::from_dynamic(
        DynamicImage::ImageLuma16(samples),
        Some(target_mode.to_owned()),
    )))
}

fn convert_i32_to_luma16(
    image: &Image,
) -> Result<crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>, PilError> {
    let image = image.materialize()?;
    let (width, height) = image.dimensions();
    let samples = image.as_bytes().chunks_exact(4).map(|bytes| {
        let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        value.clamp(0, i32::from(u16::MAX)) as u16
    });
    crate::raster::ImageBuffer::from_raw(width, height, samples.collect())
        .ok_or_else(|| PilError::InternalError("I;16 conversion buffer shape mismatch".into()))
}

fn convert_luma_to_luma16(
    image: &Image,
    source_mode: &str,
) -> Result<crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>, PilError> {
    let source = if source_mode == "YCbCr" {
        image.convert("RGB", None, None, None, None)?
    } else {
        image.clone()
    };
    let luma = source
        .convert("L", None, None, None, None)?
        .materialize()?
        .to_luma8();
    let (width, height) = luma.dimensions();
    Ok(crate::raster::ImageBuffer::from_fn(
        width,
        height,
        |x, y| crate::raster::Luma([u16::from(luma.get_pixel(x, y)[0])]),
    ))
}

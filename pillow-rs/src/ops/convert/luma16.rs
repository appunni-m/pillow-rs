use crate::error::PilError;
use crate::image::{Image, Mode};
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
    if matches!(target_mode, "I" | "F") {
        // Numeric I;16 → I/F conversion is a typed sample promotion, not a
        // host-side image conversion. Keep the decoded u16 buffer intact so
        // SIMD and GPU can widen each sample in their own conversion kernel.
        // I;16N retains Pillow's existing clipped-L conversion semantics.
        let mut result = if source_mode == "I;16N" {
            let luma = Image::push_op(
                image,
                crate::pipeline::PipelineOp::Convert {
                    mode: crate::pipeline::ColorMode::L,
                    matrix: None,
                    dither: None,
                },
            );
            Image::push_op(
                &luma,
                crate::pipeline::PipelineOp::Convert {
                    mode: if target_mode == "I" {
                        crate::pipeline::ColorMode::I
                    } else {
                        crate::pipeline::ColorMode::F
                    },
                    matrix: None,
                    dither: None,
                },
            )
        } else {
            Image::push_op(
                image,
                crate::pipeline::PipelineOp::Convert {
                    mode: if target_mode == "I" {
                        crate::pipeline::ColorMode::I
                    } else {
                        crate::pipeline::ColorMode::F
                    },
                    matrix: None,
                    dither: None,
                },
            )
        };
        if let Image::Pipeline {
            explicit_mode,
            mode_selector,
            ..
        } = &mut result
        {
            *explicit_mode = Some(target_mode.to_owned());
            *mode_selector = Mode::from_pillow_name(target_mode);
        }
        return Ok(Some(result));
    }
    super::nonstandard::with_eager_cpu_convert_receipt(
        || {
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
        },
        |result| {
            result
                .as_ref()
                .map_or(0, super::nonstandard::eager_convert_image_bytes)
        },
    )
}

pub(super) fn try_convert_to_luma16(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
) -> Result<Option<Image>, PilError> {
    if !is_luma16_mode(target_mode) {
        return Ok(None);
    }
    if matches!(source_mode, "1" | "L") {
        let mut result = Image::push_op(
            image,
            crate::pipeline::PipelineOp::Convert {
                mode: crate::pipeline::ColorMode::I16,
                matrix: None,
                dither: None,
            },
        );
        if let Image::Pipeline {
            explicit_mode,
            mode_selector,
            ..
        } = &mut result
        {
            *explicit_mode = Some(target_mode.to_owned());
            *mode_selector = Mode::from_pillow_name(target_mode);
        }
        return Ok(Some(result));
    }
    super::nonstandard::with_eager_cpu_convert_receipt(
        || {
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
        },
        |result| {
            result
                .as_ref()
                .map_or(0, super::nonstandard::eager_convert_image_bytes)
        },
    )
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

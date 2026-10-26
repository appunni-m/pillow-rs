use crate::color;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{ColorMode, PipelineOp};
use crate::raster::DynamicImage;

use super::mode::is_luma16_mode;

pub(super) fn try_convert_early_modes(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    if source_mode == "RGB" && matches!(target_mode, "La" | "RGBX" | "RGBa") {
        // An opaque RGB source needs no alpha arithmetic for these three
        // destinations. Reuse the native LA/RGBA converters and carry the
        // Pillow mode tag on the lazy result so SIMD and GPU can execute the
        // pixel work instead of materializing through the eager mode adapter.
        let (pipeline_mode, explicit_mode) = match target_mode {
            "La" => (ColorMode::LA, "La"),
            "RGBX" => (ColorMode::RGBA, "RGBX"),
            "RGBa" => (ColorMode::RGBA, "RGBa"),
            _ => unreachable!(),
        };
        let mut result = Image::push_op(
            image,
            PipelineOp::Convert {
                mode: pipeline_mode,
                matrix: None,
                dither: None,
            },
        );
        if let Image::Pipeline {
            explicit_mode: mode,
            ..
        } = &mut result
        {
            *mode = Some(explicit_mode.to_owned());
        }
        return Ok(Some(result));
    }
    if source_mode == "CMYK" && target_mode == "RGB" {
        return Ok(Some(Image::push_op(
            image,
            PipelineOp::Convert {
                mode: ColorMode::RGB,
                matrix: None,
                dither: None,
            },
        )));
    }
    if source_mode == target_mode {
        return Ok(Some(image.copy()));
    }
    let native_alpha_target = match (source_mode, target_mode) {
        ("LA", "La") | ("RGBa", "La") => Some(ColorMode::LaPremultiplied),
        ("La", "LA") => Some(ColorMode::LA),
        ("RGBA", "RGBa") => Some(ColorMode::RgbPremultiplied),
        ("RGBa", "RGBA") => Some(ColorMode::RGBA),
        ("RGBa", "RGB") => Some(ColorMode::RGB),
        ("RGBa", "RGBX") | ("RGBA", "RGBX") | ("LA", "RGBX") => Some(ColorMode::Rgbx),
        _ => None,
    };
    if let Some(mode) = native_alpha_target {
        let mut result = Image::push_op(
            image,
            PipelineOp::Convert {
                mode,
                matrix: None,
                dither: None,
            },
        );
        if let Image::Pipeline { explicit_mode, .. } = &mut result {
            *explicit_mode = Some(target_mode.to_owned());
        }
        return Ok(Some(result));
    }
    if target_mode == "LAB" {
        return Ok(Some(convert_to_lab(image, source_mode)?));
    }
    if let Some(converted) =
        convert_from_premultiplied(image, source_mode, target_mode, dither, palette, colors)?
    {
        return Ok(Some(converted));
    }
    if let Some(converted) =
        convert_to_premultiplied(image, source_mode, target_mode, dither, palette, colors)?
    {
        return Ok(Some(converted));
    }
    if target_mode == "RGBX" {
        return Ok(Some(convert_to_rgbx(
            image,
            source_mode,
            target_mode,
            dither,
            palette,
            colors,
        )?));
    }
    if source_mode == "PA" && target_mode == "P" {
        return Ok(Some(convert_pa_to_p(image)?));
    }
    Ok(None)
}

fn convert_to_lab(image: &Image, source_mode: &str) -> Result<Image, PilError> {
    // Pillow's LAB converter consumes RGB samples. Unsigned-16 luma has no
    // direct LAB converter, so follow the clipped L fallback before RGB.
    let rgb = if is_luma16_mode(source_mode) {
        image
            .convert("L", None, None, None, None)?
            .convert("RGB", None, None, None, None)?
    } else {
        image.convert("RGB", None, None, None, None)?
    };
    let mut result = Image::push_op(
        &rgb,
        PipelineOp::ConvertLab {
            icc_profile: crate::lab::pillow_lab_icc_profile().into(),
        },
    );
    if let Image::Pipeline { explicit_mode, .. } = &mut result {
        *explicit_mode = Some("LAB".to_owned());
    }
    Ok(result)
}

fn convert_from_premultiplied(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    let has_supported_conversion =
        source_mode == "La" || (source_mode == "RGBa" && !matches!(target_mode, "P" | "PA"));
    if !has_supported_conversion {
        return Ok(None);
    }
    super::nonstandard::with_eager_cpu_convert_receipt(
        || {
            let straight =
                crate::ops::pil_resize::unpremultiply_alpha(image.materialized_shared()?.as_ref());
            if matches!((source_mode, target_mode), ("La", "LA") | ("RGBa", "RGBA")) {
                return Ok(Some(Image::from_dynamic(straight, None)));
            }
            let rgb = Image::from_dynamic(DynamicImage::ImageRgb8(straight.to_rgb8()), None);
            Ok(Some(rgb.convert(
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

fn convert_to_premultiplied(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    if !matches!(target_mode, "RGBa" | "La") {
        return Ok(None);
    }
    if source_mode == "P" {
        return Err(PilError::ValueError("conversion not supported".into()));
    }
    super::nonstandard::with_eager_cpu_convert_receipt(
        || {
            if matches!((source_mode, target_mode), ("RGBA", "RGBa") | ("LA", "La")) {
                let premultiplied = crate::ops::pil_resize::premultiply_alpha(
                    image.materialized_shared()?.as_ref(),
                );
                return Ok(Some(Image::from_dynamic(
                    premultiplied,
                    Some(target_mode.to_owned()),
                )));
            }
            if matches!(source_mode, "1" | "L" | "LA" | "I" | "F") || is_luma16_mode(source_mode) {
                return Err(PilError::ValueError(format!(
                    "conversion from L to {target_mode} not supported"
                )));
            }
            // RGB has direct opaque RGBa/La converters; RGBA->La normalizes through RGB.
            let rgb = image.convert("RGB", None, dither, palette, colors)?;
            let rgb = rgb.materialized_shared()?;
            let converted = if target_mode == "RGBa" {
                DynamicImage::ImageRgba8(rgb.to_rgba8())
            } else {
                DynamicImage::ImageLumaA8(color::pil_grayscale_alpha(&rgb)?)
            };
            Ok(Some(Image::from_dynamic(
                converted,
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

fn convert_to_rgbx(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Image, PilError> {
    super::nonstandard::with_eager_cpu_convert_receipt(
        || {
            // LA/PA preserve alpha in X; RGBA's direct converter supplies 255.
            let base_mode = if matches!(source_mode, "LA" | "PA") {
                "RGBA"
            } else {
                "RGB"
            };
            let converted = image.convert(base_mode, None, dither, palette, colors)?;
            let mut rgba = converted.materialized_shared()?.to_rgba8();
            if source_mode == "P"
                && let Some(alpha) = image.palette_alpha()
            {
                let indices = image.materialized_shared()?;
                for (pixel, &index) in rgba.pixels_mut().zip(indices.as_bytes()) {
                    pixel[3] = alpha.get(usize::from(index)).copied().unwrap_or(255);
                }
            }
            Ok(Image::from_dynamic(
                DynamicImage::ImageRgba8(rgba),
                Some(target_mode.to_owned()),
            ))
        },
        super::nonstandard::eager_convert_image_bytes,
    )
}

fn convert_pa_to_p(image: &Image) -> Result<Image, PilError> {
    // Dropping PA alpha preserves indices and the existing palette.
    Ok(Image::Loaded(crate::image::LoadedData {
        image: std::sync::Arc::new(DynamicImage::ImageLuma8(
            image.materialized_shared()?.to_luma8(),
        )),
        known_zero_fill: false,
        explicit_mode: Some("P".to_owned()),
        decoded_mode: crate::raster::ColorType::L8.into(),
        palette: image.palette(),
        palette_alpha: image.palette_alpha(),
        source_format: image.source_format(),
        info: image.image_info(),
        exif: image.exif_metadata(),
    }))
}

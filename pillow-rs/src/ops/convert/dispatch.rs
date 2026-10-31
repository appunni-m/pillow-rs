use crate::color;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

use super::mode::is_luma16_mode;

pub(super) fn try_convert_scalar_to_rgbx(
    image: &Image,
    source_mode: Mode,
) -> Result<Option<Image>, PilError> {
    if !matches!(source_mode, Mode::ImageI32 | Mode::ImageF32) {
        return Ok(None);
    }
    if crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
        .for_mode(source_mode)
        .scope_for_target(Mode::ImageRgbx8)
        .is_none()
    {
        return Ok(None);
    }
    // TODO(pillow-rs#3ptb): parity-check I/F→RGBX for every CPU Architecture
    // request and verify the exact output identity plus fallback receipt.
    let requested_backend = crate::compute::preferred_active_backend()?
        .map(|(backend, _)| backend)
        .unwrap_or(crate::compute::Backend::Cpu);
    if requested_backend == crate::compute::Backend::Gpu {
        // TODO(pillow-rs#3ptb): route GPU-selected I/F→RGBX through a native
        // GPU strategy when the GPU Convert family is migrated.
        return Ok(None);
    }

    let source = image.materialized_shared_for_ops()?;
    let operation = PipelineOp::Convert {
        mode: Mode::ImageRgbx8,
        matrix: None,
        dither: None,
    };
    let input = crate::compute::registry::support::OperationInputs {
        image: source.as_ref(),
        palette: crate::compute::registry::support::PaletteInput {
            rgb: None,
            alpha: &[],
        },
    };
    let raster = crate::compute::execute_eager_cpu_arch_image_operation_with_route(
        "Convert",
        source.as_ref(),
        requested_backend,
        requested_backend,
        None,
        |context| {
            let prepared = crate::compute::registry::prepare_cpu(&operation, input, context)?;
            let selection = *prepared.selection();
            Ok((prepared.execute()?, selection))
        },
    )?;
    Image::from_dynamic_with_mode(raster, Some(Mode::ImageRgbx8)).map(Some)
}

pub(super) fn try_convert_early_modes(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    if source_mode == Mode::ImageRgb8
        && matches!(
            target_mode,
            Mode::ImageLaPremultiplied8 | Mode::ImageRgbx8 | Mode::ImageRgbPremultiplied8
        )
    {
        // An opaque RGB source needs no alpha arithmetic for these three
        // destinations. Reuse the native LA/RGBA converters and carry the
        // Pillow mode tag on the lazy result so SIMD and GPU can execute the
        // pixel work instead of materializing through the eager mode adapter.
        let pipeline_mode = target_mode;
        let mut result = Image::push_op(
            image,
            PipelineOp::Convert {
                mode: pipeline_mode,
                matrix: None,
                dither: None,
            },
        );
        if let Image::Pipeline { explicit_mode, .. } = &mut result {
            *explicit_mode = Some(pipeline_mode);
        }
        return Ok(Some(result));
    }
    if source_mode == Mode::ImageCmyk8 && target_mode == Mode::ImageRgb8 {
        return Ok(Some(Image::push_op(
            image,
            PipelineOp::Convert {
                mode: Mode::ImageRgb8,
                matrix: None,
                dither: None,
            },
        )));
    }
    if source_mode == target_mode {
        return Ok(Some(image.copy()));
    }
    let native_alpha_target = match (source_mode, target_mode) {
        (Mode::ImageLumaA8, Mode::ImageLaPremultiplied8)
        | (Mode::ImageRgbPremultiplied8, Mode::ImageLaPremultiplied8) => {
            Some(Mode::ImageLaPremultiplied8)
        }
        (Mode::ImageLaPremultiplied8, Mode::ImageLumaA8) => Some(Mode::ImageLumaA8),
        (Mode::ImageRgba8, Mode::ImageRgbPremultiplied8) => {
            Some(Mode::ImageRgbPremultiplied8)
        }
        (Mode::ImageRgbPremultiplied8, Mode::ImageRgba8) => Some(Mode::ImageRgba8),
        (Mode::ImageRgbPremultiplied8, Mode::ImageRgb8) => Some(Mode::ImageRgb8),
        (Mode::ImageRgbPremultiplied8, Mode::ImageRgbx8)
        | (Mode::ImageRgba8, Mode::ImageRgbx8)
        | (Mode::ImageLumaA8, Mode::ImageRgbx8) => Some(Mode::ImageRgbx8),
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
            *explicit_mode = Some(target_mode);
        }
        return Ok(Some(result));
    }
    if target_mode == Mode::ImageLab8 {
        return Ok(Some(convert_to_lab(image, source_mode)?));
    }
    if let Some(converted) =
        convert_to_premultiplied(image, source_mode, target_mode, dither, palette, colors)?
    {
        return Ok(Some(converted));
    }
    if target_mode == Mode::ImageRgbx8 {
        return Ok(Some(convert_to_rgbx(
            image,
            source_mode,
            target_mode,
            dither,
            palette,
            colors,
        )?));
    }
    Ok(None)
}

fn convert_to_lab(image: &Image, source_mode: Mode) -> Result<Image, PilError> {
    // The prepared ConvertLab operation consumes exact RGB8 samples.
    // Unsigned-16 luma retains the existing clipped-L-before-RGB route.
    let rgb = if is_luma16_mode(source_mode) {
        image
            .convert(Mode::ImageLuma8, None, None, None, None)?
            .convert(Mode::ImageRgb8, None, None, None, None)?
    } else {
        image.convert(Mode::ImageRgb8, None, None, None, None)?
    };
    let mut result = Image::push_op(
        &rgb,
        PipelineOp::ConvertLab {
            icc_profile: crate::lab::pillow_lab_icc_profile().into(),
        },
    );
    if let Image::Pipeline { explicit_mode, .. } = &mut result {
        *explicit_mode = Some(crate::raster::Mode::ImageLab8);
    }
    Ok(result)
}

fn convert_to_premultiplied(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    if !matches!(
        target_mode,
        Mode::ImageRgbPremultiplied8 | Mode::ImageLaPremultiplied8
    ) {
        return Ok(None);
    }
    if source_mode == Mode::ImagePalette8 {
        return Err(PilError::ValueError("conversion not supported".into()));
    }
    if crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
        .for_mode(source_mode)
        .scope_for_target(target_mode)
        .is_some()
    {
        // Prepared mode strategies own every declared source/target pair.
        return Ok(None);
    }
    super::nonstandard::with_eager_cpu_convert_receipt(
        || {
            if matches!(
                source_mode,
                Mode::One
                    | Mode::ImageLuma8
                    | Mode::ImageLumaA8
                    | Mode::ImageI32
                    | Mode::ImageF32
            ) || is_luma16_mode(source_mode)
            {
                return Err(PilError::ValueError(format!(
                    "conversion from L to {} not supported",
                    target_mode.pillow_name().unwrap_or("unknown")
                )));
            }
            // RGB has direct opaque RGBa/La converters; RGBA->La normalizes through RGB.
            let rgb = image.convert(Mode::ImageRgb8, None, dither, palette, colors)?;
            let rgb = rgb.materialized_shared()?;
            let converted = if target_mode == Mode::ImageRgbPremultiplied8 {
                DynamicImage::ImageRgba8(rgb.to_rgba8())
            } else {
                DynamicImage::ImageLumaA8(color::pil_grayscale_alpha(&rgb)?)
            };
            Ok(Some(Image::from_dynamic_with_mode(
                converted,
                Some(target_mode),
            )?))
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
    source_mode: Mode,
    target_mode: Mode,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Image, PilError> {
    super::nonstandard::with_eager_cpu_convert_receipt(
        || {
            // LA/PA preserve alpha in X; RGBA's direct converter supplies 255.
            let base_mode = if matches!(source_mode, Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8) {
                Mode::ImageRgba8
            } else {
                Mode::ImageRgb8
            };
            let converted = image.convert(base_mode, None, dither, palette, colors)?;
            let mut rgba = converted.materialized_shared()?.to_rgba8();
            if source_mode == Mode::ImagePalette8
                && let Some(alpha) = image.palette_alpha()
            {
                let indices = image.materialized_shared()?;
                for (pixel, &index) in rgba.pixels_mut().zip(indices.as_bytes()) {
                    pixel[3] = alpha.get(usize::from(index)).copied().unwrap_or(255);
                }
            }
            Image::from_dynamic_with_mode(
                DynamicImage::ImageRgba8(rgba),
                Some(target_mode),
            )
        },
        super::nonstandard::eager_convert_image_bytes,
    )
}


use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use crate::compute::registry::support::{
    OperationId, OperationModeTransition, OperationTargetComposition,
    OperationTargetCompositionKind, OperationTransitionSource, OutputModeRule,
};

pub(super) fn try_convert_scalar_to_rgbx(
    image: &Image,
    source_mode: Mode,
) -> Result<Option<Image>, PilError> {
    if !matches!(source_mode, Mode::ImageI32 | Mode::ImageF32) {
        return Ok(None);
    }
    if crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
        .scope_for_target(source_mode, Mode::ImageRgbx8)
        .is_none()
    {
        return Ok(None);
    }
    // TODO(pillow-rs#3ptb): parity-check I/F→RGBX for every CPU Architecture
    // request and verify the exact output identity plus fallback receipt.
    let (requested_backend, context) = image.cpu_arch_execution_route()?;
    if requested_backend == Some(crate::compute::Backend::Gpu) {
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
    let raster = crate::compute::execute_eager_cpu_arch_image_operation_with_context(
        "Convert",
        source.as_ref(),
        context,
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
        let result = Image::push_op(
            image,
            PipelineOp::Convert {
                mode: target_mode,
                matrix: None,
                dither: None,
            },
        );
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
        let result = Image::push_op(
            image,
            PipelineOp::Convert {
                mode,
                matrix: None,
                dither: None,
            },
        );
        return Ok(Some(result));
    }
    if let Some(composition) = crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
        .target_composition_for(source_mode, target_mode)
    {
        return execute_target_composition(image, dither, composition).map(Some);
    }
    Ok(None)
}

fn execute_target_composition(
    image: &Image,
    dither: Option<&str>,
    composition: OperationTargetComposition,
) -> Result<Image, PilError> {
    if let crate::compute::registry::support::OperationRouteOutcome::Rejects {
        reason, kind, ..
    } = composition.outcome
    {
        return Err(kind.into_error(reason));
    }
    if composition.output != OutputModeRule::ByTargetMode {
        return Err(PilError::InternalError(
            "Convert wrapper composition omitted its declared target output".into(),
        ));
    }
    let transition = composition.transitions.first().ok_or_else(|| {
        PilError::InternalError("Convert composition omitted its child transition".into())
    })?;
    let base_mode = transition.target;
    match composition.kind {
        OperationTargetCompositionKind::ConvertLab => {
            convert_to_lab(image, composition.transitions)
        }
        OperationTargetCompositionKind::ConvertPremultiplied => convert_to_premultiplied(
            image,
            composition.source_mode,
            composition.target_mode,
            base_mode,
            dither,
        ),
        OperationTargetCompositionKind::ConvertRgbx => convert_to_rgbx(
            image,
            composition.source_mode,
            composition.target_mode,
            base_mode,
            dither,
        ),
        OperationTargetCompositionKind::ConvertPalette => super::palette::convert_prepared_palette(
            image,
            composition.source_mode,
            composition.target_mode,
            dither,
        ),
        OperationTargetCompositionKind::ConvertGuard => Err(PilError::InternalError(
            "Convert guard route unexpectedly declared a successful composition".into(),
        )),
    }
}

fn convert_to_lab(
    image: &Image,
    transitions: &[OperationModeTransition],
) -> Result<Image, PilError> {
    let mut converted: Option<Image> = None;
    for transition in transitions {
        let source = match transition.operation {
            OperationId::Convert => match transition.source {
                OperationTransitionSource::Input => image,
                OperationTransitionSource::Previous => converted.as_ref().ok_or_else(|| {
                    PilError::InternalError(
                        "ConvertLab transition omitted its previous image".into(),
                    )
                })?,
            },
            OperationId::ConvertLab => break,
            _ => {
                return Err(PilError::InternalError(
                    "ConvertLab route declared a non-Convert child".into(),
                ));
            }
        };
        converted = Some(source.convert(transition.target, None, None, None, None)?);
    }
    let lab_input = converted.as_ref().unwrap_or(image);
    Ok(Image::push_op(
        lab_input,
        PipelineOp::ConvertLab {
            icc_profile: crate::lab::pillow_lab_icc_profile().into(),
        },
    ))
}

fn convert_to_premultiplied(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
    base_mode: Mode,
    dither: Option<&str>,
) -> Result<Image, PilError> {
    // RGB has direct opaque RGBa/La converters; RGBA->La normalizes through RGB.
    super::nonstandard::execute_eager_convert_composition(
        image,
        source_mode,
        base_mode,
        target_mode,
        dither,
        |converted, _source| match converted {
            DynamicImage::ImageRgb8(rgb) => {
                crate::compute::pool_cpu::ops::color::convert::helpers::premultiplied::from_rgb_samples(
                    rgb.width(),
                    rgb.height(),
                    rgb.pixels().map(|pixel| [pixel[0], pixel[1], pixel[2]]),
                    target_mode,
                )
            }
            _ => Err(PilError::InternalError(
                "Convert premultiplication child returned an unsupported mode".into(),
            )),
        },
    )
}

fn convert_to_rgbx(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
    base_mode: Mode,
    dither: Option<&str>,
) -> Result<Image, PilError> {
    // The declared route selects RGBA for LA/PA and RGB for other sources.
    super::nonstandard::execute_eager_convert_composition(
        image,
        source_mode,
        base_mode,
        target_mode,
        dither,
        |converted, source| {
            let mut rgba = converted.to_rgba8();
            if source_mode == Mode::ImagePalette8
                && let Some(alpha) = image.palette_alpha()
            {
                let DynamicImage::ImagePalette8(indices) = source else {
                    return Err(PilError::InternalError(
                        "Convert RGBX palette child did not retain P indices".into(),
                    ));
                };
                for (pixel, &index) in rgba.pixels_mut().zip(indices.as_raw()) {
                    pixel[3] = alpha.get(usize::from(index)).copied().unwrap_or(255);
                }
            }
            Ok(DynamicImage::ImageRgba8(rgba))
        },
    )
}


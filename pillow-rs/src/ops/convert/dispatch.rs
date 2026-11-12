use crate::compute::registry::support::{
    OperationTargetComposition, OperationTargetCompositionKind, OutputModeRule,
};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

pub(super) fn try_convert_scalar_to_rgbx(
    image: &Image,
    source_mode: Mode,
) -> Result<Option<Image>, PilError> {
    if !matches!(source_mode, Mode::ImageI32 | Mode::ImageF32) {
        return Ok(None);
    }
    if crate::compute::pool_cpu::ops::color::ops::CONVERT_SUPPORT
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

    let (source, mut operation_receipts) = image.materialized_shared_for_ops_with_receipts()?;
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
    let raster = crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
        "Convert",
        source.as_ref(),
        context,
        requested_backend,
        None,
        &mut operation_receipts,
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
        (Mode::ImageRgba8, Mode::ImageRgbPremultiplied8) => Some(Mode::ImageRgbPremultiplied8),
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
    if let Some(composition) = crate::compute::pool_cpu::ops::color::ops::CONVERT_SUPPORT
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
    match composition.kind {
        OperationTargetCompositionKind::ConvertPremultiplied => Err(PilError::InternalError(
            "Convert premultiplied rejection route unexpectedly admitted execution".into(),
        )),
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

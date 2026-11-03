use crate::error::PilError;
use crate::image::Image;
use crate::raster::Mode;

use super::mode::is_luma16_mode;

pub(super) fn try_convert_to_luma16(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
) -> Result<Option<Image>, PilError> {
    if !is_luma16_mode(target_mode) {
        return Ok(None);
    }
    if matches!(source_mode, Mode::One | Mode::ImageLuma8) {
        let result = Image::push_op(
            image,
            crate::pipeline::PipelineOp::Convert {
                mode: target_mode,
                matrix: None,
                dither: None,
            },
        );
        return Ok(Some(result));
    }
    if crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
        .scope_for_target(source_mode, target_mode)
        .is_some()
    {
        return execute_declared_source_strategy(image, target_mode).map(Some);
    }
    if source_mode == Mode::ImagePalette8 {
        return Err(PilError::ValueError("conversion not supported".into()));
    }
    Err(PilError::InternalError(
        "I;16 Convert target has no declared source-mode strategy".into(),
    ))
}

fn execute_declared_source_strategy(image: &Image, target_mode: Mode) -> Result<Image, PilError> {
    let source = image.materialized_shared_for_ops()?;
    let operation = crate::pipeline::PipelineOp::Convert {
        mode: target_mode,
        matrix: None,
        dither: None,
    };
    let palette_rgb = image.palette();
    let palette_alpha = image.palette_alpha().unwrap_or_default();
    let input = crate::compute::registry::support::OperationInputs {
        image: source.as_ref(),
        palette: crate::compute::registry::support::PaletteInput {
            rgb: palette_rgb.as_deref(),
            alpha: &palette_alpha,
        },
    };
    let (requested_backend, context) = image.cpu_arch_execution_route()?;
    let fallback_reason = (requested_backend == Some(crate::compute::Backend::Gpu)).then(|| {
        format!(
            "GPU Convert has no declared I;16 target strategy; used CPU {:?}",
            context.architecture()
        )
    });
    let output = crate::compute::execute_eager_cpu_arch_image_operation_with_context(
        "Convert",
        source.as_ref(),
        context,
        requested_backend,
        fallback_reason,
        |context| {
            let prepared = crate::compute::registry::prepare_cpu(&operation, input, context)?;
            let selection = *prepared.selection();
            prepared.execute().map(|output| (output, selection))
        },
    )?;
    Image::from_dynamic_with_mode(output, Some(target_mode))
}

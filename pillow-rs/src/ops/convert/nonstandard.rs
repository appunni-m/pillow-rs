use crate::compute::Backend;
use crate::compute::registry::support::{OperationInputs, PaletteInput};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

/// Runs a wrapper-owned Convert composition through one routed CPU child.
///
/// The exact source mode, palette inputs, child Convert target, and execution
/// context are captured before strategy preparation. The callback applies the
/// existing wrapper-only sample work to the prepared child's native output.
/// A lazy source is a separate image graph: `materialized_shared_for_ops`
/// evaluates it through that graph's stored Backend request, and
/// `prepare_execution` resolves the routed Backend/Architecture context before
/// this composition executes its child with the context captured here. Source
/// operation receipts are restored before the child strategy receipt.
pub(super) fn execute_eager_convert_composition(
    image: &Image,
    source_mode: Mode,
    base_mode: Mode,
    target_mode: Mode,
    dither: Option<&str>,
    finish: impl FnOnce(DynamicImage, &DynamicImage) -> Result<DynamicImage, PilError>,
) -> Result<Image, PilError> {
    let dither = if super::operation::prepared_convert_validates_dither(source_mode, base_mode) {
        super::input::parse_dither(dither)?
    } else {
        None
    };
    let (requested_backend, context) = image.cpu_arch_execution_route_advisory();
    let (source, mut operation_receipts) = image.materialized_shared_for_ops_with_receipts()?;
    let palette = if matches!(source_mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        image.palette()
    } else {
        None
    };
    let palette_alpha = match source_mode {
        Mode::ImagePalette8 => {
            super::palette::palette_alpha_for_convert(image).unwrap_or_default()
        }
        Mode::ImagePaletteAlpha8 => image.palette_alpha().unwrap_or_default(),
        _ => Vec::new(),
    };
    let inputs = OperationInputs {
        image: source.as_ref(),
        palette: PaletteInput {
            rgb: palette.as_deref(),
            alpha: &palette_alpha,
        },
    };
    let operation = PipelineOp::Convert {
        mode: base_mode,
        matrix: None,
        dither,
    };
    let fallback_reason = (requested_backend == Some(Backend::Gpu)).then(|| {
        format!(
            "GPU Convert has no {} composition strategy; used CPU {:?}",
            target_mode.pillow_name().unwrap_or("target"),
            context.architecture()
        )
    });
    let output = crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
        "Convert",
        source.as_ref(),
        context,
        requested_backend,
        fallback_reason,
        &mut operation_receipts,
        |context| {
            let prepared = crate::compute::registry::prepare_cpu(&operation, inputs, context)?;
            let selection = *prepared.selection();
            let child = prepared.execute()?;
            let output = finish(child, source.as_ref())?;
            Ok((output, selection))
        },
    )?;
    Image::from_dynamic_with_mode(output, Some(target_mode))
}

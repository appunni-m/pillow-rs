use crate::compute::Backend;
use crate::compute::registry::support::{OperationInputs, PaletteInput};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{DitherMethod, PipelineOp};
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn convert_to_binary(
    image: &Image,
    dither: Option<DitherMethod>,
) -> Result<Image, PilError> {
    let (source, mut operation_receipts) = image.materialized_shared_for_ops_with_receipts()?;
    let palette = image.palette();
    let input = OperationInputs {
        image: source.as_ref(),
        palette: PaletteInput {
            rgb: palette.as_deref(),
            alpha: &[],
        },
    };
    let operation = PipelineOp::Convert {
        mode: Mode::One,
        matrix: None,
        dither,
    };
    // Binary Convert is CPU-owned. Preserve the selected CPU Architecture
    // when GPU preference falls back and retain the original request receipt.
    let (requested_backend, context) = image.cpu_arch_execution_route()?;
    let fallback_reason = (requested_backend == Some(Backend::Gpu)).then(|| {
        format!(
            "GPU Convert has no binary target strategy; used CPU {:?}",
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
            let prepared = crate::compute::registry::prepare_cpu(&operation, input, context)?;
            let selection = *prepared.selection();
            prepared.execute().map(|output| (output, selection))
        },
    )?;
    Ok(Image::from_dynamic(output))
}

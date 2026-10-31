use crate::compute::Backend;
use crate::compute::registry::support::{OperationInputs, PaletteInput};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{DitherMethod, PipelineOp};
use crate::raster::Mode;

pub(super) fn convert_to_binary(
    image: &Image,
    dither: Option<DitherMethod>,
) -> Result<Image, PilError> {
    let source = image.materialized_shared_for_ops()?;
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
    let selected_backend = crate::compute::preferred_active_backend()?
        .map(|(backend, _)| backend)
        .unwrap_or(Backend::Cpu);
    // Binary Convert is CPU-owned. Preserve SIMD architecture selection while
    // routing an active GPU request through its established CPU fallback.
    let requested_backend = match selected_backend {
        Backend::Gpu => Backend::Cpu,
        backend => backend,
    };
    let output = crate::compute::execute_eager_cpu_arch_image_operation(
        "Convert",
        source.as_ref(),
        requested_backend,
        |context| {
            let prepared = crate::compute::registry::prepare_cpu(&operation, input, context)?;
            let selection = *prepared.selection();
            prepared.execute().map(|output| (output, selection))
        },
    )?;
    Ok(Image::from_dynamic(output))
}

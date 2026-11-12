//! Lazy palette-target Convert output and palette propagation.

use std::sync::Arc;

use crate::compute::PreparedExecution;
use crate::error::PilError;
use crate::image::{Image, PipelinePaletteOutput};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

// TODO(pillow-rs#3ptb): freeze direct lazy P/PA output, palette, wrapper
// metadata, and CPU architecture/fallback receipts in the parity matrix.
pub(super) fn evaluate_palette_target_convert(
    source: &Image,
    image: &DynamicImage,
    ops: &[PipelineOp],
    prepared: &PreparedExecution,
    palette: &Option<Vec<u8>>,
    palette_alpha: &Option<Vec<u8>>,
) -> Result<Option<(Arc<DynamicImage>, PipelinePaletteOutput)>, PilError> {
    let [
        PipelineOp::Convert {
            mode: target @ (Mode::ImagePalette8 | Mode::ImagePaletteAlpha8),
            matrix: None,
            ..
        },
    ] = ops
    else {
        return Ok(None);
    };

    let resolved_palette = palette.clone().or_else(|| source.extract_palette());
    let wrapper_palette_alpha = palette_alpha.clone().or_else(|| source.palette_alpha());
    let resolved_alpha = if image.mode() == Mode::ImagePalette8 {
        crate::ops::convert::palette_alpha_for_lazy_convert(source).unwrap_or_default()
    } else {
        wrapper_palette_alpha.clone().unwrap_or_default()
    };
    let inputs = crate::compute::registry::support::OperationInputs {
        image,
        palette: crate::compute::registry::support::PaletteInput {
            rgb: resolved_palette.as_deref(),
            alpha: &resolved_alpha,
        },
    };
    let output =
        crate::compute::execute_prepared_cpu_arch_convert_output(prepared, &ops[0], inputs)?;
    let wrapper_metadata = output.wrapper_metadata.ok_or_else(|| {
        PilError::InternalError("palette-target Convert omitted its wrapper metadata policy".into())
    })?;
    let (output_palette, output_palette_alpha) = if matches!(
        wrapper_metadata,
        crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::PreserveSourceLoaded
    ) {
        (resolved_palette, wrapper_palette_alpha)
    } else {
        (output.palette, output.palette_alpha)
    };
    Ok(Some((
        Arc::new(output.image),
        PipelinePaletteOutput {
            mode: *target,
            palette: output_palette,
            palette_alpha: output_palette_alpha,
            wrapper_metadata,
        },
    )))
}

pub(super) fn retain_preserved_palette_metadata(
    source: &Image,
    result: &DynamicImage,
    palette: &Option<Vec<u8>>,
    palette_alpha: &Option<Vec<u8>>,
    preserves_palette: bool,
) -> Option<PipelinePaletteOutput> {
    if !preserves_palette
        || !matches!(
            result.mode(),
            Mode::ImagePalette8 | Mode::ImagePaletteAlpha8
        )
    {
        return None;
    }
    let output_palette = palette.clone().or_else(|| source.palette());
    let output_palette_alpha = palette_alpha.clone().or_else(|| source.palette_alpha());
    let missing_required_palette = result.mode() == Mode::ImagePalette8 && output_palette.is_none();
    if missing_required_palette || (output_palette.is_none() && output_palette_alpha.is_none()) {
        return None;
    }
    Some(PipelinePaletteOutput {
        mode: result.mode(),
        palette: output_palette,
        palette_alpha: output_palette_alpha,
        wrapper_metadata:
            crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::PreserveSource,
    })
}

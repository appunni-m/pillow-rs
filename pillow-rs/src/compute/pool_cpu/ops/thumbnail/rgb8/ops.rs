//! Prepared RGB PutPixel/Thumbnail fusion over native RGB samples.

use crate::compute::registry::support::{ModeSupport, OperationId, StrategySelection};
use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::Thumbnail,
    Mode::ImageRgb8,
    super::super::ops::INPUTS,
);

/// Execute the supported native RGB PutPixel/Thumbnail fusion strategy.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_putpixel_thumbnail(
    image: &DynamicImage,
    writes: &[PipelineOp],
    thumbnail: &PipelineOp,
    context: &ExecutionContext,
    resize_selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    if context.backend() != Backend::Cpu || image.mode() != Mode::ImageRgb8 {
        return Err(PilError::InternalError(
            "RGB PutPixel thumbnail fusion reached an unsupported strategy".into(),
        ));
    }
    let reduced = super::helpers::putpixel_thumbnail::prepare(image, writes, thumbnail)?;
    let (source_width, source_height) = reduced.source_dimensions;
    let (output_width, output_height) = reduced.output_dimensions;
    let bounds =
        (resize_selection.implementation.operation == OperationId::ResizeBoxed).then_some((
            0.0,
            0.0,
            f64::from(source_width) / 2.0,
            f64::from(source_height) / 2.0,
        ));
    let reduced_image = DynamicImage::ImageRgb8(reduced.image);
    let inputs = crate::compute::registry::support::OperationInputs {
        image: &reduced_image,
        palette: crate::compute::registry::support::PaletteInput::default(),
    };
    let resize = crate::compute::pool_cpu::ops::resize::ops::prepare_direct_with_selection(
        inputs,
        output_width,
        output_height,
        reduced.filter,
        bounds,
        context,
        resize_selection,
    )?;
    resize.execute_rgb8_with_sparse_rows(reduced.sparse_nonzero_rows.as_deref())
}

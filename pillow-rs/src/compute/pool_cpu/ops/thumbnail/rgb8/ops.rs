//! Architecture selection for the RGB Thumbnail strategy.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::Thumbnail,
    Mode::ImageRgb8,
    super::super::ops::INPUTS,
);

/// Execute the supported native RGB PutPixel/Thumbnail fusion strategy.
pub(crate) fn execute_putpixel_thumbnail(
    image: &DynamicImage,
    writes: &[PipelineOp],
    thumbnail: &PipelineOp,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    match (context.backend(), context.architecture(), image.mode()) {
        (Backend::Cpu, Architecture::Default, Mode::ImageRgb8) => {
            super::default::rgb_putpixel_thumbnail_impl::execute_rgb_putpixel_thumbnail(
                image, writes, thumbnail,
            )
        }
        _ => Err(PilError::InternalError(
            "RGB PutPixel thumbnail fusion reached an unsupported strategy".into(),
        )),
    }
}

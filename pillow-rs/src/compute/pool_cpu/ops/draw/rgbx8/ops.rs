use crate::compute::Architecture;
use crate::compute::registry::support::ModeSupport;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use crate::compute::pool_cpu::ops::draw::TextComposeResult;

pub(crate) fn apply_text(
    input: super::super::TextComposeInput<'_>,
) -> Result<TextComposeResult, PilError> {
    default::text_impl::apply(
        input.image,
        input.x,
        input.y,
        input.width,
        input.height,
        input.pixels.as_ref(),
        input.fill,
        false,
    )
}

pub(crate) fn apply_bitmap(
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &super::super::BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    default::bitmap_impl::apply(image, x, y, mask_width, mask_height, mask, color)
}

pub(crate) const MODE: Mode = Mode::ImageRgbx8;
pub(crate) const CHANNELS: usize = 4;
pub(crate) const FLOAT_MODE: bool = false;

pub(crate) fn support() -> ModeSupport {
    super::super::ops::mode_support(MODE)
}

pub(crate) fn execute(
    image: &DynamicImage,
    ops: &[PipelineOp],
) -> Result<DynamicImage, PilError> {
    execute_with_architecture(image, ops, Architecture::Default)
}

pub(crate) fn execute_with_architecture(
    image: &DynamicImage,
    ops: &[PipelineOp],
    architecture: Architecture,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgbx8(pixels) = image else {
        return Err(PilError::InternalError(
            "Draw strategy received a different raster variant".into(),
        ));
    };
    match architecture {
        Architecture::Default => {
            let output = default::draw_impl::execute(pixels, ops)?;
            Ok(DynamicImage::ImageRgbx8(output))
        }
        Architecture::Sse2 => sse2::draw_impl::execute(image, ops),
        Architecture::Neon => neon::draw_impl::execute(image, ops),
        Architecture::WasmSimd128 => wasm_simd128::draw_impl::execute(image, ops),
        unexpected => Err(PilError::InternalError(format!(
            "unexpected {unexpected:?} architecture selected for Draw mode {MODE:?}"
        ))),
    }
}

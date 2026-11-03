use crate::compute::{Architecture, registry::support::StrategySelection};
use crate::compute::registry::support::ModeSupport;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use crate::compute::pool_cpu::ops::draw::TextComposeResult;

pub(crate) fn apply_text<const RGBA_MASK: bool>(
    input: super::super::TextComposeInput<'_>,
) -> Result<TextComposeResult, PilError> {
    super::default::text_impl::apply::<RGBA_MASK>(
        input.image,
        input.x,
        input.y,
        input.width,
        input.height,
        input.pixels.as_ref(),
        input.fill,
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
    super::default::bitmap_impl::apply(image, x, y, mask_width, mask_height, mask, color)
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
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgbx8(pixels) = image else {
        return Err(PilError::InternalError(
            "Draw strategy received a different raster variant".into(),
        ));
    };
    let architecture = super::super::helpers::strategy::selected_architecture(selection, MODE)?;
    match architecture {
        Architecture::Default => {
            let output = super::default::draw_impl::execute(pixels, ops)?;
            Ok(DynamicImage::ImageRgbx8(output))
        }
        Architecture::Sse2 => super::sse2::draw_impl::execute(image, ops),
        Architecture::Neon => super::neon::draw_impl::execute(image, ops),
        Architecture::WasmSimd128 => super::wasm_simd128::draw_impl::execute(image, ops),
        unexpected => Err(PilError::InternalError(format!(
            "unexpected {unexpected:?} architecture selected for Draw mode {MODE:?}"
        ))),
    }
}

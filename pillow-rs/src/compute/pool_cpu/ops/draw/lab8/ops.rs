use crate::compute::registry::support::ModeSupport;
use crate::compute::{Architecture, registry::support::StrategySelection};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) fn apply_bitmap(
    selection: &StrategySelection,
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &super::super::BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    super::super::helpers::strategy::require_default_selection(selection, MODE)?;
    super::default::bitmap_impl::apply(image, x, y, mask_width, mask_height, mask, color)
}

pub(crate) const MODE: Mode = Mode::ImageLab8;
const ARCHITECTURES: super::super::helpers::support::VectorArchitectures =
    super::super::helpers::support::vector_architectures(MODE);
pub(crate) const SUPPORT: ModeSupport =
    super::super::helpers::support::vector_support(MODE, &ARCHITECTURES);
pub(crate) const CHANNELS: usize = 3;
pub(crate) const FLOAT_MODE: bool = false;

pub(crate) fn apply_text<const RGBA_MASK: bool>(
    selection: &StrategySelection,
    input: super::super::TextComposeInput<'_>,
) -> Result<super::super::TextComposeResult, PilError> {
    super::super::helpers::strategy::require_default_selection(selection, MODE)?;
    super::default::text_impl::apply::<RGBA_MASK>(
        input.image,
        input.x,
        input.y,
        input.width,
        input.height,
        input.pixels.as_ref(),
        input.fill,
    )
    .map(super::super::TextComposeResult::Updated)
}

pub(crate) fn support() -> ModeSupport {
    SUPPORT
}

pub(crate) fn execute(
    image: &DynamicImage,
    ops: &[PipelineOp],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLab8(pixels) = image else {
        return Err(PilError::InternalError(
            "Draw strategy received a different raster variant".into(),
        ));
    };
    let architecture = super::super::helpers::strategy::selected_architecture(selection, MODE)?;
    match architecture {
        Architecture::Default => {
            let output = super::default::draw_impl::execute(pixels, ops)?;
            Ok(DynamicImage::ImageLab8(output))
        }
        Architecture::Sse2 => super::sse2::draw_impl::execute(image, ops),
        Architecture::Neon => super::neon::draw_impl::execute(image, ops),
        Architecture::WasmSimd128 => super::wasm_simd128::draw_impl::execute(image, ops),
        unexpected => Err(PilError::InternalError(format!(
            "unexpected {unexpected:?} architecture selected for Draw mode {MODE:?}"
        ))),
    }
}

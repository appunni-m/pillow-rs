use crate::compute::registry::support::ModeSupport;
use crate::compute::registry::support::StrategySelection;
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
}

pub(crate) const MODE: Mode = Mode::ImageI16B;
pub(crate) const SUPPORT: ModeSupport = super::super::helpers::support::serial_support(MODE);

pub(crate) fn support() -> ModeSupport {
    SUPPORT
}

pub(crate) fn execute(
    image: &DynamicImage,
    ops: &[PipelineOp],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::strategy::require_default_selection(selection, MODE)?;
    let DynamicImage::ImageI16B(pixels) = image else {
        return Err(PilError::InternalError(
            "Draw strategy received a different raster variant".into(),
        ));
    };
    let output = super::default::draw_impl::execute(pixels, ops)?;
    Ok(DynamicImage::ImageI16B(output))
}

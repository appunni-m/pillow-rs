use super::layout::{layout_for_mode, DrawLayout};

pub(crate) fn is_eligible(
    image: &crate::raster::DynamicImage,
    ops: &[crate::pipeline::PipelineOp],
    mode: crate::raster::Mode,
) -> bool {
    if ops.is_empty() {
        return false;
    }
    let Some(layout) = layout_for_mode(mode) else {
        return false;
    };
    image.mode() == mode
        && ops
            .iter()
            .all(|op| super::admission::supports_for_image(image, op, layout))
}

pub(crate) fn execute<const CHANNELS: usize, const FLOAT_MODE: bool>(
    image: &crate::raster::DynamicImage,
    ops: &[crate::pipeline::PipelineOp],
    mode: crate::raster::Mode,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    let layout = DrawLayout::new(mode, CHANNELS, FLOAT_MODE);
    if image.mode() != mode || layout_for_mode(mode) != Some(layout) {
        return Err(crate::error::PilError::InternalError(
            "Draw architecture strategy does not match the exact mode".into(),
        ));
    }
    super::execute::execute(image, ops, layout)
}

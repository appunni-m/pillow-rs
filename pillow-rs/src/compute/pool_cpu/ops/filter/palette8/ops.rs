use crate::raster::Mode;

pub(super) const MODE: Mode = Mode::ImagePalette8;

pub(in crate::compute::pool_cpu::ops::filter) fn mode_filter_support() -> crate::compute::registry::support::ModeSupport {
    crate::compute::registry::support::ModeSupport::serial(
        crate::compute::registry::support::OperationId::ModeFilter,
        MODE,
        super::super::ops::inputs_for(
            crate::compute::registry::support::OperationId::ModeFilter,
        ),
    )
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute_mode_filter(
    image: &crate::raster::DynamicImage,
    size: u32,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    super::default::execute_mode_filter(image, size)
}

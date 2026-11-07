use crate::raster::Mode;

pub(super) const MODE: Mode = Mode::ImagePalette8;

pub(in crate::compute::pool_cpu::ops::filter) fn mode_filter_support() -> crate::compute::registry::support::ModeSupport {
    super::super::ops::serial_mode_filter(MODE)
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute_mode_filter(
    image: &crate::raster::DynamicImage,
    size: u32,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    super::default::execute_mode_filter(image, size)
}

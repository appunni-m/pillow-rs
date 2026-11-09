pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>;
use crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_native_luma16_transpose_mode;

declare_native_luma16_transpose_mode!(
    ImageI16,
    Input,
    super::default::flip_impl::execute,
    super::default::mirror_impl::execute,
    super::default::transpose_impl::execute
);

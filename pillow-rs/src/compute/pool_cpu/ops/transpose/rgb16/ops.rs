pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Rgb<u16>, Vec<u16>>;
// NO_PILLOW_SUPPORT: native ImageRgb16 transpose preserves internal samples.
use crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_transpose_mode;

declare_transpose_mode!(ImageRgb16, Input, super::default::flip_impl::execute, super::default::mirror_impl::execute, super::default::transpose_impl::execute);

pub(crate) type Input = crate::raster::GrayAlphaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

declare_invert_mode!(ImageLaPremultiplied8, Input, super::default::invert_impl::execute, 2);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
declare_quantize_native_mode!(u8, "ImageLaPremultiplied8");

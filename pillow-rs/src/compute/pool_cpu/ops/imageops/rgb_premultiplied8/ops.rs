pub(crate) type Input = crate::raster::RgbaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

declare_invert_mode!(ImageRgbPremultiplied8, Input, super::default::invert_impl::execute, 4);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
declare_quantize_native_mode!(u8, "ImageRgbPremultiplied8");

pub(crate) type Input = crate::raster::RgbImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

declare_invert_mode!(ImageHsv8, Input, super::default::invert_impl::execute, 3);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
declare_quantize_native_mode!(u8, "ImageHsv8", ImageHsv8);

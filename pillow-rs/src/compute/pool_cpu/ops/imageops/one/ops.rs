pub(crate) type Input = crate::raster::GrayImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

declare_invert_native_mode!(One, Input, 1, 1);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this extends
// its operation to exact bilevel mode `1` using the native one-byte carrier.
declare_quantize_native_mode!(u8, "One", One);

pub(crate) type Input = crate::raster::RgbaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

// NO_PILLOW_SUPPORT: Pillow ImageOps.invert rejects `RGBa`; this Rust strategy
// inverts the native premultiplied RGBA carrier byte by byte.
declare_invert_mode!(
    ImageRgbPremultiplied8,
    Input,
    super::default::invert_impl::execute,
    4,
    Some("Invert extends Pillow with native RGBa inversion"),
);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
declare_quantize_native_mode!(u8, "ImageRgbPremultiplied8", ImageRgbPremultiplied8);

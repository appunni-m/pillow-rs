pub(crate) type Input = crate::raster::RgbaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

// NO_PILLOW_SUPPORT: Pillow ImageOps.invert rejects `RGBX`; this Rust strategy
// inverts native RGB and padding bytes independently.
declare_invert_mode!(
    ImageRgbx8,
    Input,
    super::default::invert_impl::execute,
    4,
    Some("Invert extends Pillow with native RGBX inversion"),
);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds an
// RGBX strategy that ignores the unused padding byte.
declare_quantize_native_mode!(u8, "ImageRgbx8", ImageRgbx8);

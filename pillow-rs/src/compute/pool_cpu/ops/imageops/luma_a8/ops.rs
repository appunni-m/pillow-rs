pub(crate) type Input = crate::raster::GrayAlphaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

// NO_PILLOW_SUPPORT: ImageOps.invert rejects LA; this route preserves the
// native carrier as a core-only extension.
declare_invert_native_mode!(
    ImageLumaA8,
    Input,
    2,
    2,
    Some("ImageOps.invert extends Pillow with native LA inversion"),
);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
declare_quantize_native_mode!(u8, "ImageLumaA8");

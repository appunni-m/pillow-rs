pub(crate) type Input = crate::raster::RgbaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

// NO_PILLOW_SUPPORT: ImageOps.invert rejects CMYK; this route preserves all
// four native color samples as a core-only extension.
declare_invert_native_mode!(
    ImageCmyk8,
    Input,
    4,
    4,
    Some("ImageOps.invert extends Pillow with native CMYK inversion"),
);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
declare_quantize_native_mode!(u8, "ImageCmyk8", ImageCmyk8);

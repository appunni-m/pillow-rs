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
// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this extends
// palette reduction to exact CMYK mode using its native carrier projection.
declare_quantize_native_mode!(u8, "ImageCmyk8", ImageCmyk8);

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageCmyk8,
        super::super::ops::CROP_BORDER_INPUTS,
    );

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
// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds a
// native LA strategy that reduces luma and alpha samples directly.
declare_quantize_native_mode!(u8, "ImageLumaA8", ImageLumaA8);

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageLumaA8,
        super::super::ops::CROP_BORDER_INPUTS,
    );

pub(crate) type Input = crate::raster::GrayAlphaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

// NO_PILLOW_SUPPORT: Pillow ImageOps.invert rejects `La`; this Rust strategy
// inverts both native luma and premultiplied-alpha bytes.
declare_invert_mode!(
    ImageLaPremultiplied8,
    Input,
    super::default::invert_impl::execute,
    2,
    Some("Invert extends Pillow with native La inversion"),
);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds an
// exact La-mode strategy over its native premultiplied gray/alpha samples.
declare_quantize_native_mode!(u8, "ImageLaPremultiplied8", ImageLaPremultiplied8);

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageLaPremultiplied8,
        super::super::ops::CROP_BORDER_INPUTS,
    );

pub(crate) type Input = crate::raster::RgbImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

// NO_PILLOW_SUPPORT: Pillow ImageOps.invert rejects `LAB`; this Rust strategy
// applies bytewise inversion to the native LAB samples.
declare_invert_mode!(
    ImageLab8,
    Input,
    super::default::invert_impl::execute,
    3,
    Some("Invert extends Pillow with native LAB inversion"),
);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds
// exact LAB strategy over its native three-byte samples.
declare_quantize_native_mode!(u8, "ImageLab8", ImageLab8);

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageLab8,
        super::super::ops::CROP_BORDER_INPUTS,
    );

pub(crate) type Input = crate::raster::GrayImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

declare_invert_native_mode!(One, Input, 1, 1);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this extends
// its operation to exact bilevel mode `1` using the native one-byte carrier.
declare_quantize_native_mode!(u8, "One", One);

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::One,
        super::super::ops::CROP_BORDER_INPUTS,
    );

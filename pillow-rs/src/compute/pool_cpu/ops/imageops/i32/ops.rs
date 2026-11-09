use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds
// native signed-I32 sample reduction as a Rust-only strategy.
declare_quantize_native_mode!(u8, "ImageI32", ImageI32);

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageI32,
        super::super::ops::CROP_BORDER_INPUTS,
    );

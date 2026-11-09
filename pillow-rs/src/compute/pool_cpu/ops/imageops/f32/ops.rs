use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds
// native scalar-F32 sample reduction as a Rust-only strategy.
declare_quantize_native_mode!(u8, "ImageF32", ImageF32);

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageF32,
        super::super::ops::CROP_BORDER_INPUTS,
    );

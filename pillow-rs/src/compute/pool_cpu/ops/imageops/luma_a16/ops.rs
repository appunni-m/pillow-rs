use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds
// native LA16 color/alpha reduction as a Rust-only strategy.
declare_quantize_native_mode!(u16, "ImageLumaA16", ImageLumaA16);

// NO_PILLOW_SUPPORT: this native mode extends ImageOps CropBorder beyond Pillow modes.
pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageLumaA16,
        super::super::ops::CROP_BORDER_INPUTS,
    );

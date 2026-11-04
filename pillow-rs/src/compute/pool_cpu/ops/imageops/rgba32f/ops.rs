use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds
// native RGBA32F color/alpha reduction as a Rust-only strategy.
declare_quantize_native_mode!(f32, "ImageRgba32F", ImageRgba32F);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds
// native RGB32F sample reduction as a Rust-only strategy.
declare_quantize_native_mode!(f32, "ImageRgb32F", ImageRgb32F);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds
// native signed-I32 sample reduction as a Rust-only strategy.
declare_quantize_native_mode!(u8, "ImageI32", ImageI32);

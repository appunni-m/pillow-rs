use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_default;

// NO_PILLOW_SUPPORT: this CoreOnly mode uses the existing quantizer
// contract while reading this exact mode through its Default strategy.
declare_quantize_native_default!(u8, 3);

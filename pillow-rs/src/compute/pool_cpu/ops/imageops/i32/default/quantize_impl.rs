use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_default;

// NO_PILLOW_SUPPORT: reduce each signed scalar directly to its clamped
// byte-grayscale value without reinterpreting its carrier bytes as RGB.
declare_quantize_native_default!(i32_scalar);

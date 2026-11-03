use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_default;

// NO_PILLOW_SUPPORT: reduce each little-endian float scalar directly through
// the raster float-to-byte projection instead of treating its bytes as RGB.
declare_quantize_native_default!(f32_scalar);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_default;

// NO_PILLOW_SUPPORT: RGBX keeps its padding sample out of color selection and
// emits an RGB palette through the exact-mode Default strategy.
declare_quantize_native_default!(u8, 4);

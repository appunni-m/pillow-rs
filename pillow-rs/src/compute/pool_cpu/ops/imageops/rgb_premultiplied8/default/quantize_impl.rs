use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_default;

// NO_PILLOW_SUPPORT: read the exact premultiplied RGBa buffer and unpremultiply
// each sample directly for the alpha-aware palette strategy.
declare_quantize_native_default!(premultiplied_rgba);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

declare_quantize_native_mode!(u8, "ImageF32", ImageF32);

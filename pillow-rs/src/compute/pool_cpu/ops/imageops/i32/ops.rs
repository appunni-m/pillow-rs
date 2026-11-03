use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

declare_quantize_native_mode!(u8, "ImageI32", ImageI32);

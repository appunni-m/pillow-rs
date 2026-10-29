use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

declare_invert_native_mode!(ImageRgb8, super::Input, 3, 3);

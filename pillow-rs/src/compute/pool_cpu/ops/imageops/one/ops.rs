use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

declare_invert_native_mode!(One, super::Input, 1, 1);

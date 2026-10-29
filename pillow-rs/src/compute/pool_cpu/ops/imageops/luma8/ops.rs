use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

declare_invert_native_mode!(ImageLuma8, super::Input, 1, 1);

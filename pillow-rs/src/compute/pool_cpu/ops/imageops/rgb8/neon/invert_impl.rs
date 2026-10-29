use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_architecture;

declare_invert_architecture!(ImageRgb8, ImageRgb8, super::super::Input, 3, false);

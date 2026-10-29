use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_architecture;

declare_invert_architecture!(ImageLuma8, ImageLuma8, super::super::Input, 1, false);

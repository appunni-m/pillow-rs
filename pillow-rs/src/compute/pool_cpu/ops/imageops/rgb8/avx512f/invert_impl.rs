use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_architecture_avx512f;

declare_invert_architecture_avx512f!(ImageRgb8, ImageRgb8, super::super::Input, 3, false);

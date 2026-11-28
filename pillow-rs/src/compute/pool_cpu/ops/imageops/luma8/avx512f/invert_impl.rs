use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_architecture_avx512f;

declare_invert_architecture_avx512f!(ImageLuma8, ImageLuma8, super::super::Input, 1, false);

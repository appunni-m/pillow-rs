use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_architecture_avx512f;

declare_invert_architecture_avx512f!(ImageCmyk8, ImageRgba8, super::super::Input, 4, true);

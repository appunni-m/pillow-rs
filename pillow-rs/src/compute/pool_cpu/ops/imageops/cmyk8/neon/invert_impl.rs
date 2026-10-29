use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_architecture;

declare_invert_architecture!(ImageCmyk8, ImageRgba8, super::super::Input, 4, true);

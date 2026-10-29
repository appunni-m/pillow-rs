use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_architecture;

declare_invert_architecture!(ImagePaletteAlpha8, ImageLumaA8, super::super::Input, 2, false);

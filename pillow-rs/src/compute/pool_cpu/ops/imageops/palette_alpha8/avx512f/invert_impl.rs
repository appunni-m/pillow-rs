use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_architecture_avx512f;

declare_invert_architecture_avx512f!(
    ImagePaletteAlpha8,
    ImageLumaA8,
    super::super::Input,
    2,
    true
);

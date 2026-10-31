crate::compute::pool_cpu::ops::filter::helpers::native_filter_mode_leaf!(
    ImageRgbx8,
    RgbaImage,
    4,
    false,
    false,
    crate::compute::pool_cpu::ops::filter::helpers::execute_filter5x5_native_bytes
);

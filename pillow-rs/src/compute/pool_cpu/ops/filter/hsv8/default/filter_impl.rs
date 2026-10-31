crate::compute::pool_cpu::ops::filter::helpers::native_filter_mode_leaf!(
    ImageHsv8,
    RgbImage,
    3,
    false,
    false,
    crate::compute::pool_cpu::ops::filter::helpers::execute_filter5x5_native_bytes
);

crate::compute::pool_cpu::ops::filter::helpers::native_filter_mode_leaf!(
    ImageRgb8,
    RgbImage,
    3,
    true,
    false,
    super::filter5x5_impl::execute
);

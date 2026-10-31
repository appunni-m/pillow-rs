crate::compute::pool_cpu::ops::filter::helpers::native_filter_mode_leaf!(
    ImageLuma8,
    GrayImage,
    1,
    true,
    false,
    super::filter5x5_impl::execute
);

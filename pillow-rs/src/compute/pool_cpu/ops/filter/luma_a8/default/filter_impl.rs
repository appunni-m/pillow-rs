crate::compute::pool_cpu::ops::filter::helpers::native_filter_mode_leaf!(
    ImageLumaA8,
    GrayAlphaImage,
    2,
    true,
    true,
    super::filter5x5_impl::execute
);

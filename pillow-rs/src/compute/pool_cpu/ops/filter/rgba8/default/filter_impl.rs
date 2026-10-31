crate::compute::pool_cpu::ops::filter::helpers::native_filter_mode_leaf!(
    ImageRgba8,
    RgbaImage,
    4,
    true,
    false,
    super::filter5x5_impl::execute
);

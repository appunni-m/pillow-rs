use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

declare_invert_mode!(ImageHsv8, super::Input, super::default::invert_impl::execute, 3);

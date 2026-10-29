use crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_native_luma16_transpose_mode;

declare_native_luma16_transpose_mode!(ImageI16L, super::Input, super::default::flip_impl::execute, super::default::mirror_impl::execute, super::default::transpose_impl::execute);

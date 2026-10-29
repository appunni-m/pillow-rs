// NO_PILLOW_SUPPORT: native ImageRgba32F transpose preserves internal samples.
use crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_transpose_mode;

declare_transpose_mode!(ImageRgba32F, super::Input, super::default::flip_impl::execute, super::default::mirror_impl::execute, super::default::transpose_impl::execute);

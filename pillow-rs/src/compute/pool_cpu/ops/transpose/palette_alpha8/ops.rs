use crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_native_byte_transpose_mode;

declare_native_byte_transpose_mode!(ImagePaletteAlpha8, super::Input, super::default::flip_impl::execute, super::default::mirror_impl::execute, super::default::transpose_impl::execute, 2);

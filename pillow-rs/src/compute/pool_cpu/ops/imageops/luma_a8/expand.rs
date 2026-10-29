//! ImageOps.expand strategy for exact ImageLumaA8 storage.

#[path = "default/expand_impl.rs"]
mod default;

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_expand_byte_mode;

declare_expand_byte_mode!(ImageLumaA8, 2);


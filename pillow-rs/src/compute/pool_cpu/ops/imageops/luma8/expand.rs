//! ImageOps.expand strategy for exact ImageLuma8 storage.

#[path = "default/expand_impl.rs"]
mod default;

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_expand_byte_mode;

declare_expand_byte_mode!(ImageLuma8, 1);


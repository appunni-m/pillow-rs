//! ImageOps.expand strategy for exact ImageYCbCr8 storage.

#[path = "default/expand_impl.rs"]
mod default;

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_expand_byte_mode;

declare_expand_byte_mode!(ImageYCbCr8, 3);


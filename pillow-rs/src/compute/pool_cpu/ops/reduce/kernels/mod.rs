//! Mode-specific and default CPU reduction kernels.

mod default;
mod f;
mod high_depth;
mod i;
mod luma16;
mod rgb;

pub(super) use default::execute_reduce_default;
pub(in crate::compute::pool_cpu::ops) use f::reduce_f_thumbnail;
pub(super) use high_depth::{reduce_f32_pixels, reduce_u16_pixels};
pub(in crate::compute::pool_cpu::ops) use i::reduce_i_thumbnail;
pub(super) use luma16::reduce_luma16;
pub(super) use rgb::execute_reduce_rgb;
#[cfg(test)]
pub(super) use rgb::execute_reduce_rgb_sparse_2x2;
pub(in crate::compute::pool_cpu::ops) use rgb::execute_reduce_rgb_with_sparse_rows;

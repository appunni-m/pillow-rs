//! Mode-specific and default CPU reduction kernels.

mod default;
mod f;
mod i;
mod rgb;

pub(super) use default::execute_reduce_default;
pub(in crate::compute::pool_cpu::ops) use f::reduce_f_thumbnail;
pub(in crate::compute::pool_cpu::ops) use i::reduce_i_thumbnail;
pub(super) use rgb::execute_reduce_rgb;
#[cfg(test)]
pub(super) use rgb::execute_reduce_rgb_sparse_2x2;
pub(in crate::compute::pool_cpu::ops) use rgb::execute_reduce_rgb_with_sparse_rows;

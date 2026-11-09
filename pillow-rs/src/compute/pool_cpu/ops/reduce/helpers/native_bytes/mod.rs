//! Native interleaved byte reducers shared by exact mode strategies.

mod general;
pub(crate) mod kernels;
mod rgb;

pub(crate) use general::reduce_bytes;
pub(crate) use rgb::{reduce_rgb, reduce_rgb_with_sparse_rows, reduce_sparse_2x2_if_applicable};

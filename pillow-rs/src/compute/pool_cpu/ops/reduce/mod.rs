//! CPU Reduce family module exports.

mod ops;

pub(crate) use ops::{execute_reduce, execute_reduce_with_mode};
pub(super) use ops::{
    execute_reduce_rgb_with_sparse_rows, reduce_f_thumbnail, reduce_i_thumbnail,
};

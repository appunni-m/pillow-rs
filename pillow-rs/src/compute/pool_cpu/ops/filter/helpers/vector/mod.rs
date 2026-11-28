//! Shared vector kernels used by Filter mode and architecture strategies.

mod filter3x3_vector;
mod filter5x5_rgb_vector;
mod filter5x5_vector;
mod rank_vector;

pub(crate) use filter3x3_vector::apply as apply_filter3x3_vector;
pub(super) use filter3x3_vector::apply_la_binomial;
pub(super) use filter5x5_rgb_vector::{apply_blur_ring, scan_uniform};
pub(crate) use filter5x5_vector::apply as apply_filter5x5_vector;
pub(super) use rank_vector::execute_row as execute_rank_vector_row;

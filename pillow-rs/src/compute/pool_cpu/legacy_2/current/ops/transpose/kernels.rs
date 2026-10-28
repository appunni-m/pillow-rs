//! Static entry points for Transpose kernels.

#[path = "kernels/native.rs"]
mod native;

pub(super) use native::execute_transpose;
#[cfg(test)]
pub(super) use native::{TRANSPOSE_TILE_SIZE, transpose_bytes_tiled};

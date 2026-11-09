//! Shared typed Offset kernels used by exact-mode strategies.

mod byte;
mod default;
mod luma16;

pub(crate) use byte::execute_bytes;
pub(crate) use default::offset_pixels;
pub(crate) use luma16::execute_luma16;

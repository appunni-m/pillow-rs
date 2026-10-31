//! Shared SIMD convolution kernels for exact native byte-mode strategies.

mod bounded_i32;
mod boxed;
mod constants;
mod convolution;
mod execute;
mod plan;
mod telemetry;

pub(crate) use execute::{execute, execute_boxed};

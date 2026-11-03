//! Shared native-sample kernels used by exact Composite mode strategies.

mod bytes;
mod execute;
mod mask;
mod numeric;
mod output;

pub(crate) use execute::{execute, execute_byte_selected};

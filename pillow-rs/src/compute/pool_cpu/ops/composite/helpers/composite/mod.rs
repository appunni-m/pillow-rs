//! Shared native-sample kernels used by exact Composite mode strategies.

#[derive(Debug, Clone, Copy)]
pub(crate) enum CompositeLayout {
    Byte { channels: u8 },
    U16 { channels: u8 },
    F32 { channels: u8 },
}

mod bytes;
mod execute;
mod mask;
mod numeric;
mod output;

pub(crate) use execute::{execute, execute_byte_selected};

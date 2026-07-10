//! Minimal `fvar` header parsing for named-instance face selection.

use crate::error::FontError;

/// Parsed variation-axis and named-instance counts.
#[derive(Debug, Clone, Copy)]
pub struct FvarTable {
    pub axis_count: u16,
    pub instance_count: u16,
}

pub fn parse_fvar(data: &[u8]) -> Result<FvarTable, FontError> {
    if data.len() < 16 {
        return Err(FontError::InvalidFont(
            "fvar table too short (need 16 bytes)".into(),
        ));
    }
    let major = u16::from_be_bytes([data[0], data[1]]);
    if major != 1 {
        return Err(FontError::InvalidFont("unsupported fvar version".into()));
    }
    Ok(FvarTable {
        axis_count: u16::from_be_bytes([data[8], data[9]]),
        instance_count: u16::from_be_bytes([data[12], data[13]]),
    })
}

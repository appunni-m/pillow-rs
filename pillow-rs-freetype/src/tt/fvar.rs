//! Minimal `fvar` parsing for named-instance face selection.

use crate::error::FontError;

/// Parsed variation-axis and named-instance metadata.
#[derive(Debug, Clone)]
pub struct FvarTable {
    pub axis_count: u16,
    pub instance_count: u16,
    pub axes: Vec<FvarAxis>,
    pub instances: Vec<FvarInstance>,
}

/// One axis record from the `fvar` table.
#[derive(Debug, Clone, Copy)]
pub struct FvarAxis {
    pub tag: u32,
    pub default_value: i32,
}

/// One named instance from the `fvar` table.
#[derive(Debug, Clone)]
pub struct FvarInstance {
    pub subfamily_name_id: u16,
    pub postscript_name_id: Option<u16>,
    pub coords: Vec<i32>,
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
    let axes_offset = u16::from_be_bytes([data[4], data[5]]) as usize;
    let axis_count = u16::from_be_bytes([data[8], data[9]]);
    let axis_size = u16::from_be_bytes([data[10], data[11]]) as usize;
    let instance_count = u16::from_be_bytes([data[12], data[13]]);
    let instance_size = u16::from_be_bytes([data[14], data[15]]) as usize;
    let axis_count_usize = usize::from(axis_count);
    let instance_count_usize = usize::from(instance_count);
    let instances_offset = axes_offset
        .checked_add(
            axis_count_usize
                .checked_mul(axis_size)
                .ok_or_else(|| FontError::InvalidFont("fvar axes array offset overflow".into()))?,
        )
        .ok_or_else(|| FontError::InvalidFont("fvar axes array offset overflow".into()))?;
    let instances_end = instances_offset
        .checked_add(
            instance_count_usize
                .checked_mul(instance_size)
                .ok_or_else(|| {
                    FontError::InvalidFont("fvar instance array offset overflow".into())
                })?,
        )
        .ok_or_else(|| FontError::InvalidFont("fvar instance array offset overflow".into()))?;
    if instances_end > data.len() {
        return Err(FontError::InvalidFont(
            "fvar instance array too short".into(),
        ));
    }

    if axis_count != 0 && axis_size < 20 {
        return Err(FontError::InvalidFont("fvar axis size too short".into()));
    }

    let mut axes = Vec::with_capacity(axis_count_usize);
    for index in 0..axis_count_usize {
        let off = axes_offset + index * axis_size;
        axes.push(FvarAxis {
            tag: u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]),
            default_value: i32::from_be_bytes([
                data[off + 8],
                data[off + 9],
                data[off + 10],
                data[off + 11],
            ]),
        });
    }

    let mut instances = Vec::with_capacity(instance_count_usize);
    let min_instance_size = 4 + axis_count_usize * 4;
    if instance_size < min_instance_size {
        return Err(FontError::InvalidFont(
            "fvar instance size too short".into(),
        ));
    }
    for index in 0..instance_count_usize {
        let off = instances_offset + index * instance_size;
        let subfamily_name_id = u16::from_be_bytes([data[off], data[off + 1]]);
        let coords = (0..axis_count_usize)
            .map(|axis| {
                let coord_off = off + 4 + axis * 4;
                i32::from_be_bytes([
                    data[coord_off],
                    data[coord_off + 1],
                    data[coord_off + 2],
                    data[coord_off + 3],
                ])
            })
            .collect();
        let postscript_name_id = if instance_size >= min_instance_size + 2 {
            let id = u16::from_be_bytes([
                data[off + min_instance_size],
                data[off + min_instance_size + 1],
            ]);
            (id != 0xFFFF).then_some(id)
        } else {
            None
        };
        instances.push(FvarInstance {
            subfamily_name_id,
            postscript_name_id,
            coords,
        });
    }

    Ok(FvarTable {
        axis_count,
        instance_count,
        axes,
        instances,
    })
}

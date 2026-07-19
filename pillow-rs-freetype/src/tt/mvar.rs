//! OpenType `MVAR` metric variation support.
//!
//! FreeType maps selected MVAR tags directly onto face metric fields in
//! `src/truetype/ttgxvar.c:1406-1472`, then reapplies those deltas after
//! variation coordinate changes in `tt_apply_mvar`.

use crate::error::FontError;

use super::varstore::ItemVariationStore;

const TAG_VASC: u32 = u32::from_be_bytes(*b"vasc");
const TAG_VDSC: u32 = u32::from_be_bytes(*b"vdsc");
const TAG_VLGP: u32 = u32::from_be_bytes(*b"vlgp");
const TAG_VCRS: u32 = u32::from_be_bytes(*b"vcrs");
const TAG_VCRN: u32 = u32::from_be_bytes(*b"vcrn");
const TAG_VCOF: u32 = u32::from_be_bytes(*b"vcof");

#[derive(Debug, Clone)]
pub struct MvarTable {
    item_store: ItemVariationStore,
    records: Vec<ValueRecord>,
}

#[derive(Debug, Clone, Copy)]
struct ValueRecord {
    tag: u32,
    outer_index: u16,
    inner_index: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VerticalHeaderDeltas {
    pub ascender: i32,
    pub descender: i32,
    pub line_gap: i32,
    pub caret_slope_rise: i32,
    pub caret_slope_run: i32,
    pub caret_offset: i32,
}

impl MvarTable {
    pub fn parse(data: &[u8], axis_count: usize) -> Result<Self, FontError> {
        if data.len() < 12 {
            return Err(FontError::InvalidFont("MVAR table too short".into()));
        }
        if read_u16(data, 0)? != 1 {
            return Err(FontError::InvalidFont("unsupported MVAR version".into()));
        }
        let value_record_size = usize::from(read_u16(data, 6)?);
        if value_record_size < 8 {
            return Err(FontError::InvalidFont(
                "MVAR value record size too small".into(),
            ));
        }
        let value_count = usize::from(read_u16(data, 8)?);
        let item_store_offset = read_u16(data, 10)? as usize;
        let item_store = ItemVariationStore::parse(data, item_store_offset, axis_count)?;

        let mut records = Vec::with_capacity(value_count);
        let mut pos = 12usize;
        for _ in 0..value_count {
            let tag = read_u32(data, pos)?;
            let outer_index = read_u16(data, pos + 4)?;
            let inner_index = read_u16(data, pos + 6)?;
            records.push(ValueRecord {
                tag,
                outer_index,
                inner_index,
            });
            pos = pos
                .checked_add(value_record_size)
                .ok_or_else(|| FontError::InvalidFont("MVAR value records overflow".into()))?;
        }

        Ok(Self {
            item_store,
            records,
        })
    }

    pub fn vertical_header_deltas(&self, normalized_coords: &[i16]) -> VerticalHeaderDeltas {
        let mut deltas = VerticalHeaderDeltas::default();
        for record in &self.records {
            let delta = self.item_store.item_delta(
                record.outer_index,
                record.inner_index,
                normalized_coords,
            );
            match record.tag {
                TAG_VASC => deltas.ascender = delta,
                TAG_VDSC => deltas.descender = delta,
                TAG_VLGP => deltas.line_gap = delta,
                TAG_VCRS => deltas.caret_slope_rise = delta,
                TAG_VCRN => deltas.caret_slope_run = delta,
                TAG_VCOF => deltas.caret_offset = delta,
                _ => {}
            }
        }
        deltas
    }
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| FontError::InvalidFont("MVAR u16 out of range".into()))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, FontError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| FontError::InvalidFont("MVAR u32 out of range".into()))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

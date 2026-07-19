//! OpenType `HVAR` horizontal metric variation support.
//!
//! FreeType loads only the advance-width item store for the public fast
//! horizontal advance adjustment path (`ttgxvar.c:ft_var_load_hvvar`); LSB/RSB
//! maps are intentionally not evaluated there.

use crate::error::FontError;

use super::varstore::{DeltaSetIndexMap, ItemVariationStore};

#[derive(Debug, Clone)]
pub struct HvarTable {
    item_store: ItemVariationStore,
    advance_width_map: Option<DeltaSetIndexMap>,
}

impl HvarTable {
    pub fn parse(data: &[u8], axis_count: usize) -> Result<Self, FontError> {
        if data.len() < 12 {
            return Err(FontError::InvalidFont("HVAR table too short".into()));
        }
        let major = read_u16(data, 0)?;
        if major != 1 {
            return Err(FontError::InvalidFont("unsupported HVAR version".into()));
        }
        let item_store_offset = read_u32(data, 4)? as usize;
        let advance_width_map_offset = read_u32(data, 8)? as usize;
        let item_store = ItemVariationStore::parse(data, item_store_offset, axis_count)?;
        let advance_width_map = if advance_width_map_offset == 0 {
            None
        } else {
            Some(DeltaSetIndexMap::parse(
                data,
                advance_width_map_offset,
                &item_store,
            )?)
        };
        Ok(Self {
            item_store,
            advance_width_map,
        })
    }

    pub fn advance_delta(&self, glyph_index: u16, normalized_coords: &[i16]) -> i32 {
        let (outer_index, inner_index) = self
            .advance_width_map
            .as_ref()
            .and_then(|map| map.get(usize::from(glyph_index)))
            .unwrap_or((0, glyph_index));
        self.item_store
            .item_delta(outer_index, inner_index, normalized_coords)
    }
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| FontError::InvalidFont("HVAR u16 out of range".into()))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, FontError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| FontError::InvalidFont("HVAR u32 out of range".into()))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

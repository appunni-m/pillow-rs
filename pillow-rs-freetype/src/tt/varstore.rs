//! OpenType item variation store support shared by HVAR/VVAR/MVAR-style tables.
//!
//! This follows FreeType's `src/truetype/ttgxvar.c`
//! `tt_var_load_item_variation_store`, `tt_var_load_delta_set_index_mapping`,
//! and `tt_var_get_item_delta` paths. The parser stores only the data needed
//! to evaluate public metric deltas in pure Rust.

use crate::error::FontError;

#[derive(Debug, Clone)]
pub struct ItemVariationStore {
    axis_count: usize,
    regions: Vec<VariationRegion>,
    data: Vec<ItemVariationData>,
}

#[derive(Debug, Clone)]
struct VariationRegion {
    axes: Vec<RegionAxis>,
}

#[derive(Debug, Clone, Copy)]
struct RegionAxis {
    start: i32,
    peak: i32,
    end: i32,
}

#[derive(Debug, Clone)]
struct ItemVariationData {
    item_count: usize,
    word_delta_count: usize,
    long_words: bool,
    region_indices: Vec<usize>,
    delta_set: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DeltaSetIndexMap {
    entries: Vec<(u16, u16)>,
}

impl ItemVariationStore {
    pub fn parse(
        data: &[u8],
        offset: usize,
        expected_axis_count: usize,
    ) -> Result<Self, FontError> {
        if read_u16(data, offset)? != 1 {
            return Err(FontError::InvalidFont(
                "unsupported item variation store format".into(),
            ));
        }
        let region_offset = read_u32(data, offset + 2)? as usize;
        let data_count = usize::from(read_u16(data, offset + 6)?);
        if data_count == 0 {
            return Err(FontError::InvalidFont(
                "item variation store has no varData".into(),
            ));
        }
        let offsets_start = offset + 8;
        let mut data_offsets = Vec::with_capacity(data_count);
        for index in 0..data_count {
            data_offsets.push(read_u32(data, offsets_start + index * 4)? as usize);
        }

        let region_list = offset + region_offset;
        let axis_count = usize::from(read_u16(data, region_list)?);
        let region_count = usize::from(read_u16(data, region_list + 2)?);
        if axis_count != expected_axis_count {
            return Err(FontError::InvalidFont(
                "item variation store axis count mismatch".into(),
            ));
        }
        if region_count >= 32768 {
            return Err(FontError::InvalidFont(
                "item variation store has too many regions".into(),
            ));
        }

        let mut regions = Vec::with_capacity(region_count);
        let mut region_pos = region_list + 4;
        for _ in 0..region_count {
            let mut axes = Vec::with_capacity(axis_count);
            for _ in 0..axis_count {
                let start = i32::from(read_i16(data, region_pos)?) << 2;
                let mut peak = i32::from(read_i16(data, region_pos + 2)?) << 2;
                let end = i32::from(read_i16(data, region_pos + 4)?) << 2;
                region_pos += 6;
                if (start < 0 && end > 0) || start > peak || peak > end {
                    peak = 0;
                }
                axes.push(RegionAxis { start, peak, end });
            }
            regions.push(VariationRegion { axes });
        }

        let mut var_data = Vec::with_capacity(data_count);
        for data_offset in data_offsets {
            let data_pos = offset + data_offset;
            let item_count = usize::from(read_u16(data, data_pos)?);
            let raw_word_delta_count = read_u16(data, data_pos + 2)?;
            let long_words = raw_word_delta_count & 0x8000 != 0;
            let word_delta_count = usize::from(raw_word_delta_count & 0x7FFF);
            let region_idx_count = usize::from(read_u16(data, data_pos + 4)?);
            if word_delta_count > region_idx_count || region_idx_count > region_count {
                return Err(FontError::InvalidFont(
                    "invalid item variation delta counts".into(),
                ));
            }
            let mut region_indices = Vec::with_capacity(region_idx_count);
            let region_indices_start = data_pos + 6;
            for index in 0..region_idx_count {
                let region_index = usize::from(read_u16(data, region_indices_start + index * 2)?);
                if region_index >= region_count {
                    return Err(FontError::InvalidFont(
                        "item variation region index out of range".into(),
                    ));
                }
                region_indices.push(region_index);
            }
            let per_region_size = if long_words {
                (word_delta_count + region_idx_count) * 2
            } else {
                word_delta_count + region_idx_count
            };
            let delta_start = region_indices_start + region_idx_count * 2;
            let delta_len = item_count.checked_mul(per_region_size).ok_or_else(|| {
                FontError::InvalidFont("item variation delta set too large".into())
            })?;
            let delta_set = data
                .get(delta_start..delta_start + delta_len)
                .ok_or_else(|| FontError::InvalidFont("item variation delta set truncated".into()))?
                .to_vec();
            var_data.push(ItemVariationData {
                item_count,
                word_delta_count,
                long_words,
                region_indices,
                delta_set,
            });
        }

        Ok(Self {
            axis_count,
            regions,
            data: var_data,
        })
    }

    pub fn item_delta(&self, outer_index: u16, inner_index: u16, normalized_coords: &[i16]) -> i32 {
        if outer_index == 0xFFFF && inner_index == 0xFFFF {
            return 0;
        }
        let Some(var_data) = self.data.get(usize::from(outer_index)) else {
            return 0;
        };
        let inner_index = usize::from(inner_index);
        if inner_index >= var_data.item_count || var_data.region_indices.is_empty() {
            return 0;
        }
        if normalized_coords.len() < self.axis_count {
            return 0;
        }

        let per_region_size = if var_data.long_words {
            (var_data.word_delta_count + var_data.region_indices.len()) * 2
        } else {
            var_data.word_delta_count + var_data.region_indices.len()
        };
        let mut pos = inner_index * per_region_size;
        let mut value = 0i64;
        for (master, region_index) in var_data.region_indices.iter().copied().enumerate() {
            let delta = if var_data.long_words {
                if master < var_data.word_delta_count {
                    let delta = read_i32(&var_data.delta_set, pos).unwrap_or(0);
                    pos += 4;
                    delta
                } else {
                    let delta = read_i16(&var_data.delta_set, pos).map_or(0, i32::from);
                    pos += 2;
                    delta
                }
            } else if master < var_data.word_delta_count {
                let delta = read_i16(&var_data.delta_set, pos).map_or(0, i32::from);
                pos += 2;
                delta
            } else {
                let delta = var_data
                    .delta_set
                    .get(pos)
                    .copied()
                    .map_or(0, |byte| i32::from(byte as i8));
                pos += 1;
                delta
            };
            let Some(region) = self.regions.get(region_index) else {
                continue;
            };
            let scalar = region.scalar(normalized_coords);
            if scalar != 0 {
                value += i64::from(delta) * i64::from(scalar);
            }
        }
        ((value + 0x8000) >> 16) as i32
    }
}

impl VariationRegion {
    fn scalar(&self, normalized_coords: &[i16]) -> i32 {
        let mut scalar = 0x1_0000i32;
        for (axis, coord) in self.axes.iter().zip(normalized_coords.iter().copied()) {
            let coord = i32::from(coord) << 2;
            if axis.peak == coord || axis.peak == 0 {
                continue;
            }
            if coord <= axis.start || coord >= axis.end {
                return 0;
            }
            scalar = if coord < axis.peak {
                mul_div_round(scalar, coord - axis.start, axis.peak - axis.start)
            } else {
                mul_div_round(scalar, axis.end - coord, axis.end - axis.peak)
            };
        }
        scalar
    }
}

impl DeltaSetIndexMap {
    pub fn parse(
        data: &[u8],
        offset: usize,
        item_store: &ItemVariationStore,
    ) -> Result<Self, FontError> {
        let format = *data
            .get(offset)
            .ok_or_else(|| FontError::InvalidFont("delta-set map missing format".into()))?;
        let entry_format = *data
            .get(offset + 1)
            .ok_or_else(|| FontError::InvalidFont("delta-set map missing entry format".into()))?;
        let (map_count, mut pos) = match format {
            0 => (usize::from(read_u16(data, offset + 2)?), offset + 4),
            1 => (read_u32(data, offset + 2)? as usize, offset + 6),
            _ => {
                return Err(FontError::InvalidFont(
                    "unsupported delta-set map format".into(),
                ));
            }
        };
        if entry_format & 0xC0 != 0 {
            return Err(FontError::InvalidFont(
                "invalid delta-set map entry format".into(),
            ));
        }
        let entry_size = usize::from((entry_format & 0x30) >> 4) + 1;
        let inner_bit_count = u32::from(entry_format & 0x0F) + 1;
        let inner_index_mask = (1u32 << inner_bit_count) - 1;
        let mut entries = Vec::with_capacity(map_count);
        for _ in 0..map_count {
            let mut map_data = 0u32;
            for _ in 0..entry_size {
                map_data = (map_data << 8)
                    | u32::from(*data.get(pos).ok_or_else(|| {
                        FontError::InvalidFont("delta-set map entry truncated".into())
                    })?);
                pos += 1;
            }
            if map_data == 0xFFFF_FFFF {
                entries.push((0xFFFF, 0xFFFF));
                continue;
            }
            let outer_index = (map_data >> inner_bit_count) as usize;
            let inner_index = (map_data & inner_index_mask) as usize;
            if outer_index >= item_store.data.len()
                || inner_index >= item_store.data[outer_index].item_count
            {
                return Err(FontError::InvalidFont(
                    "delta-set map index out of range".into(),
                ));
            }
            entries.push((outer_index as u16, inner_index as u16));
        }
        Ok(Self { entries })
    }

    pub fn get(&self, index: usize) -> Option<(u16, u16)> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries[index.min(self.entries.len() - 1)])
        }
    }
}

fn mul_div_round(a: i32, b: i32, c: i32) -> i32 {
    if c == 0 {
        return i32::MAX;
    }
    let sign = if (a < 0) ^ (b < 0) ^ (c < 0) { -1 } else { 1 };
    let a = i64::from(a).abs();
    let b = i64::from(b).abs();
    let c = i64::from(c).abs();
    let value = (a * b + (c >> 1)) / c;
    (value as i32) * sign
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| FontError::InvalidFont("u16 out of range".into()))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_i16(data: &[u8], offset: usize) -> Result<i16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| FontError::InvalidFont("i16 out of range".into()))?;
    Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_i32(data: &[u8], offset: usize) -> Result<i32, FontError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| FontError::InvalidFont("i32 out of range".into()))?;
    Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, FontError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| FontError::InvalidFont("u32 out of range".into()))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

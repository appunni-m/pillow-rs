//! Minimal `gvar` tuple-variation support for TrueType variable glyphs.
//!
//! This implements the OpenType tuple variation data model used by FreeType's
//! `src/truetype/ttgxvar.c`: shared/embedded tuples, optional intermediate
//! regions, packed point lists, and packed X/Y deltas.  The scaler applies the
//! resulting font-unit deltas before hinting.

use crate::error::FontError;
use crate::fixed::ft_mul_fix;
const TUPLES_SHARE_POINT_NUMBERS: u16 = 0x8000;
const TUPLE_COUNT_MASK: u16 = 0x0FFF;
const EMBEDDED_PEAK_TUPLE: u16 = 0x8000;
const INTERMEDIATE_REGION: u16 = 0x4000;
const PRIVATE_POINT_NUMBERS: u16 = 0x2000;
const TUPLE_INDEX_MASK: u16 = 0x0FFF;
const F2DOT14_ONE: i32 = 0x4000;
const F16DOT16_ONE: i32 = 0x1_0000;

/// Parsed glyph-variation table.
#[derive(Debug, Clone)]
pub struct GvarTable {
    axis_count: usize,
    shared_tuples: Vec<Vec<i16>>,
    glyph_offsets: Vec<u32>,
    data_offset: usize,
    data: Vec<u8>,
}

impl GvarTable {
    /// Return accumulated gvar deltas for `glyph_index` in 16.16 font units.
    pub fn glyph_deltas_fixed(
        &self,
        glyph_index: u16,
        point_count_with_phantoms: usize,
        normalized_coords: &[i16],
    ) -> Result<Option<Vec<(i32, i32)>>, FontError> {
        if normalized_coords.len() < self.axis_count {
            return Ok(None);
        }
        let glyph_index = usize::from(glyph_index);
        let Some((&start, &end)) = self
            .glyph_offsets
            .get(glyph_index)
            .zip(self.glyph_offsets.get(glyph_index + 1))
        else {
            return Ok(None);
        };
        if start == end {
            return Ok(None);
        }
        let start = self.data_offset + start as usize;
        let end = self.data_offset + end as usize;
        let glyph_data = self
            .data
            .get(start..end)
            .ok_or_else(|| FontError::InvalidFont("gvar glyph data out of range".into()))?;
        if glyph_data.len() < 4 {
            return Err(FontError::InvalidFont("gvar glyph data too short".into()));
        }

        let tuple_count_flags = read_u16(glyph_data, 0)?;
        let tuple_count = usize::from(tuple_count_flags & TUPLE_COUNT_MASK);
        let data_offset = usize::from(read_u16(glyph_data, 2)?);
        let tuple_headers_end = 4 + tuple_count * 4;
        if tuple_headers_end > glyph_data.len() || data_offset > glyph_data.len() {
            return Err(FontError::InvalidFont(
                "gvar tuple headers out of range".into(),
            ));
        }

        let mut tuple_headers = Vec::with_capacity(tuple_count);
        let mut header_pos = 4;
        for _ in 0..tuple_count {
            let variation_data_size = usize::from(read_u16(glyph_data, header_pos)?);
            let tuple_index = read_u16(glyph_data, header_pos + 2)?;
            header_pos += 4;
            let peak = if tuple_index & EMBEDDED_PEAK_TUPLE != 0 {
                let tuple = read_tuple(glyph_data, header_pos, self.axis_count)?;
                header_pos += self.axis_count * 2;
                tuple
            } else {
                let shared_index = usize::from(tuple_index & TUPLE_INDEX_MASK);
                self.shared_tuples
                    .get(shared_index)
                    .cloned()
                    .ok_or_else(|| {
                        FontError::InvalidFont("gvar shared tuple index out of range".into())
                    })?
            };
            let intermediate = if tuple_index & INTERMEDIATE_REGION != 0 {
                let start_tuple = read_tuple(glyph_data, header_pos, self.axis_count)?;
                header_pos += self.axis_count * 2;
                let end_tuple = read_tuple(glyph_data, header_pos, self.axis_count)?;
                header_pos += self.axis_count * 2;
                Some((start_tuple, end_tuple))
            } else {
                None
            };
            tuple_headers.push(TupleHeader {
                variation_data_size,
                tuple_index,
                peak,
                intermediate,
            });
        }
        if header_pos > data_offset {
            return Err(FontError::InvalidFont(
                "gvar tuple header exceeds data offset".into(),
            ));
        }

        let mut shared_points = None;
        let mut tuple_data_pos = data_offset;
        if tuple_count_flags & TUPLES_SHARE_POINT_NUMBERS != 0 {
            let (points, consumed) =
                read_point_numbers(&glyph_data[tuple_data_pos..], point_count_with_phantoms)?;
            tuple_data_pos += consumed;
            shared_points = Some(points);
        }

        let mut deltas = vec![(0i32, 0i32); point_count_with_phantoms];
        for header in tuple_headers {
            let tuple_data_end = tuple_data_pos + header.variation_data_size;
            let tuple_data = glyph_data
                .get(tuple_data_pos..tuple_data_end)
                .ok_or_else(|| FontError::InvalidFont("gvar tuple data out of range".into()))?;
            tuple_data_pos = tuple_data_end;
            let scalar = tuple_scalar(
                &header.peak,
                header.intermediate.as_ref(),
                &normalized_coords[..self.axis_count],
            );
            if scalar == 0 {
                continue;
            }
            let (points, delta_pos) = if header.tuple_index & PRIVATE_POINT_NUMBERS != 0 {
                read_point_numbers(tuple_data, point_count_with_phantoms)?
            } else if let Some(points) = &shared_points {
                (points.clone(), 0)
            } else {
                ((0..point_count_with_phantoms).collect(), 0)
            };
            let (x_deltas, x_consumed) =
                read_packed_deltas(&tuple_data[delta_pos..], points.len())?;
            let (y_deltas, _) =
                read_packed_deltas(&tuple_data[delta_pos + x_consumed..], points.len())?;
            for ((point_index, dx), dy) in points.into_iter().zip(x_deltas).zip(y_deltas) {
                if let Some((acc_x, acc_y)) = deltas.get_mut(point_index) {
                    *acc_x += ft_mul_fix(dx << 16, scalar);
                    *acc_y += ft_mul_fix(dy << 16, scalar);
                }
            }
        }

        Ok(Some(deltas))
    }

    /// Return accumulated gvar deltas rounded to integer font units.
    pub fn glyph_deltas(
        &self,
        glyph_index: u16,
        point_count_with_phantoms: usize,
        normalized_coords: &[i16],
    ) -> Result<Option<Vec<(i32, i32)>>, FontError> {
        Ok(self
            .glyph_deltas_fixed(glyph_index, point_count_with_phantoms, normalized_coords)?
            .map(|deltas| {
                deltas
                    .into_iter()
                    .map(|(x, y)| (fixed_to_int(x), fixed_to_int(y)))
                    .collect()
            }))
    }
}

#[derive(Debug)]
struct TupleHeader {
    variation_data_size: usize,
    tuple_index: u16,
    peak: Vec<i16>,
    intermediate: Option<(Vec<i16>, Vec<i16>)>,
}

pub fn parse_gvar(data: &[u8], glyph_count: u16) -> Result<GvarTable, FontError> {
    if data.len() < 20 {
        return Err(FontError::InvalidFont("gvar table too short".into()));
    }
    let major = read_u16(data, 0)?;
    let minor = read_u16(data, 2)?;
    if major != 1 || minor != 0 {
        return Err(FontError::InvalidFont("unsupported gvar version".into()));
    }
    let axis_count = usize::from(read_u16(data, 4)?);
    let shared_tuple_count = usize::from(read_u16(data, 6)?);
    let shared_tuple_offset = read_u32(data, 8)? as usize;
    let glyph_variation_count = read_u16(data, 12)?;
    let flags = read_u16(data, 14)?;
    let data_offset = read_u32(data, 16)? as usize;
    if glyph_variation_count != glyph_count {
        return Err(FontError::InvalidFont("gvar glyph count mismatch".into()));
    }
    let mut shared_tuples = Vec::with_capacity(shared_tuple_count);
    let mut shared_pos = shared_tuple_offset;
    for _ in 0..shared_tuple_count {
        shared_tuples.push(read_tuple(data, shared_pos, axis_count)?);
        shared_pos += axis_count * 2;
    }

    let offset_count = usize::from(glyph_variation_count) + 1;
    let mut glyph_offsets = Vec::with_capacity(offset_count);
    let offsets_start = 20;
    if flags & 1 != 0 {
        for index in 0..offset_count {
            glyph_offsets.push(read_u32(data, offsets_start + index * 4)?);
        }
    } else {
        for index in 0..offset_count {
            glyph_offsets.push(u32::from(read_u16(data, offsets_start + index * 2)?) * 2);
        }
    }
    Ok(GvarTable {
        axis_count,
        shared_tuples,
        glyph_offsets,
        data_offset,
        data: data.to_vec(),
    })
}

pub(crate) fn apply_deltas_to_outline(
    outline: &mut crate::tt::glyf::GlyphOutline,
    deltas: &[(i32, i32)],
) {
    for (point, (dx, dy)) in outline.points.iter_mut().zip(deltas.iter().copied()) {
        point.x += dx;
        point.y += dy;
    }
    recompute_outline_bounds(outline);
}

pub(crate) fn apply_fixed_deltas_to_outline(
    outline: &mut crate::tt::glyf::GlyphOutline,
    deltas: &[(i32, i32)],
) {
    let mut unrounded_points = Vec::with_capacity(outline.points.len());
    for (point, (dx, dy)) in outline.points.iter_mut().zip(deltas.iter().copied()) {
        let original_x = point.x;
        let original_y = point.y;
        unrounded_points.push(crate::tt::glyf::UnroundedPoint {
            x: (original_x << 6).wrapping_add(fixed_to_fdot6(dx)),
            y: (original_y << 6).wrapping_add(fixed_to_fdot6(dy)),
        });
        point.x = point.x.wrapping_add(fixed_to_int(dx));
        point.y = point.y.wrapping_add(fixed_to_int(dy));
    }
    outline.unrounded_points = Some(unrounded_points);
    recompute_outline_bounds(outline);
}

fn recompute_outline_bounds(outline: &mut crate::tt::glyf::GlyphOutline) {
    let Some(first) = outline.points.first().copied() else {
        outline.xmin = 0;
        outline.ymin = 0;
        outline.xmax = 0;
        outline.ymax = 0;
        outline.bbox_xmin = 0;
        return;
    };
    let mut xmin = first.x;
    let mut ymin = first.y;
    let mut xmax = first.x;
    let mut ymax = first.y;
    for crate::tt::glyf::OutlinePoint { x, y, .. } in &outline.points[1..] {
        xmin = xmin.min(*x);
        ymin = ymin.min(*y);
        xmax = xmax.max(*x);
        ymax = ymax.max(*y);
    }
    outline.xmin = xmin;
    outline.ymin = ymin;
    outline.xmax = xmax;
    outline.ymax = ymax;
    outline.bbox_xmin = xmin;
}

fn tuple_scalar(peak: &[i16], intermediate: Option<&(Vec<i16>, Vec<i16>)>, coords: &[i16]) -> i32 {
    let mut scalar = F16DOT16_ONE;
    for ((peak_coord, coord), axis_index) in
        peak.iter().copied().zip(coords.iter().copied()).zip(0..)
    {
        if peak_coord == 0 {
            continue;
        }
        let (start, end) = intermediate.map_or_else(
            || (peak_coord.min(0), peak_coord.max(0)),
            |(start, end)| (start[axis_index], end[axis_index]),
        );
        if coord < start || coord > end || (coord == 0 && peak_coord != 0) {
            return 0;
        }
        let factor = if coord == peak_coord {
            F16DOT16_ONE
        } else if coord < peak_coord {
            div_to_fixed(i32::from(coord - start), i32::from(peak_coord - start))
        } else {
            div_to_fixed(i32::from(end - coord), i32::from(end - peak_coord))
        };
        scalar = mul_fix_rounded(scalar, factor);
    }
    scalar
}

fn div_to_fixed(num: i32, den: i32) -> i32 {
    if den == 0 {
        return 0;
    }
    (((i64::from(num)) << 16) / i64::from(den)) as i32
}

pub(crate) fn fixed_to_int(value: i32) -> i32 {
    value.wrapping_add(0x8000) >> 16
}

pub(crate) fn fixed_to_fdot6(value: i32) -> i32 {
    value.wrapping_add(0x200) >> 10
}

fn mul_fix_rounded(value: i32, scalar: i32) -> i32 {
    (((i64::from(value) * i64::from(scalar)) + 0x8000) >> 16) as i32
}

fn read_point_numbers(data: &[u8], point_count: usize) -> Result<(Vec<usize>, usize), FontError> {
    let Some(first) = data.first().copied() else {
        return Err(FontError::InvalidFont(
            "gvar point run missing count".into(),
        ));
    };
    if first == 0 {
        return Ok(((0..point_count).collect(), 1));
    }
    let (count, mut pos) = if first & 0x80 != 0 {
        let second = data
            .get(1)
            .copied()
            .ok_or_else(|| FontError::InvalidFont("gvar point count truncated".into()))?;
        (
            (((usize::from(first & 0x7F)) << 8) | usize::from(second)),
            2,
        )
    } else {
        (usize::from(first), 1)
    };
    let mut points = Vec::with_capacity(count);
    let mut last = 0usize;
    while points.len() < count {
        let control = *data
            .get(pos)
            .ok_or_else(|| FontError::InvalidFont("gvar point run truncated".into()))?;
        pos += 1;
        let run_count = usize::from(control & 0x7F) + 1;
        let use_words = control & 0x80 != 0;
        for _ in 0..run_count.min(count - points.len()) {
            let delta = if use_words {
                let value = usize::from(read_u16(data, pos)?);
                pos += 2;
                value
            } else {
                let value =
                    usize::from(*data.get(pos).ok_or_else(|| {
                        FontError::InvalidFont("gvar point byte truncated".into())
                    })?);
                pos += 1;
                value
            };
            last += delta;
            points.push(last);
        }
    }
    Ok((points, pos))
}

fn read_packed_deltas(data: &[u8], count: usize) -> Result<(Vec<i32>, usize), FontError> {
    let mut result = Vec::with_capacity(count);
    let mut pos = 0;
    while result.len() < count {
        let control = *data
            .get(pos)
            .ok_or_else(|| FontError::InvalidFont("gvar delta run truncated".into()))?;
        pos += 1;
        let run_count = usize::from(control & 0x3F) + 1;
        if control & 0x80 != 0 {
            result.extend(std::iter::repeat_n(0, run_count.min(count - result.len())));
        } else if control & 0x40 != 0 {
            for _ in 0..run_count.min(count - result.len()) {
                result.push(i32::from(read_i16(data, pos)?));
                pos += 2;
            }
        } else {
            for _ in 0..run_count.min(count - result.len()) {
                let byte = *data
                    .get(pos)
                    .ok_or_else(|| FontError::InvalidFont("gvar delta byte truncated".into()))?;
                pos += 1;
                result.push(i32::from(byte as i8));
            }
        }
    }
    Ok((result, pos))
}

fn read_tuple(data: &[u8], offset: usize, axis_count: usize) -> Result<Vec<i16>, FontError> {
    let mut tuple = Vec::with_capacity(axis_count);
    for axis in 0..axis_count {
        tuple.push(read_i16(data, offset + axis * 2)?);
    }
    Ok(tuple)
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| FontError::InvalidFont("gvar u16 out of range".into()))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_i16(data: &[u8], offset: usize) -> Result<i16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| FontError::InvalidFont("gvar i16 out of range".into()))?;
    Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, FontError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| FontError::InvalidFont("gvar u32 out of range".into()))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// Convert one design coordinate into OpenType normalized 2.14 space.
pub fn normalize_axis_coord(design: i32, min: i32, default: i32, max: i32) -> i16 {
    let value = if design == default {
        0
    } else if design < default {
        if design <= min {
            -F2DOT14_ONE
        } else {
            -normalize_axis_delta(default - design, default - min)
        }
    } else if design >= max {
        // FreeType clamps out-of-range design coordinates before dividing by
        // the axis extent, so degenerate default==max axes still normalize to
        // +1.0 for values above the default.  See ttgxvar.c:2152-2211.
        F2DOT14_ONE
    } else {
        normalize_axis_delta(design - default, max - default)
    };
    value.clamp(-F2DOT14_ONE, F2DOT14_ONE) as i16
}

fn normalize_axis_delta(delta: i32, extent: i32) -> i32 {
    if extent <= 0 {
        return 0;
    }
    (((i64::from(delta)) * i64::from(F2DOT14_ONE)) / i64::from(extent)) as i32
}

//! Embedded bitmap strike metadata for EBLC/EBDT TrueType tables.

use crate::error::FontError;
use crate::tt::{TableDirectory, tag};

#[derive(Debug, Clone)]
pub struct SbitTable {
    kind: SbitTableKind,
    eblc: Vec<u8>,
    ebdt: Vec<u8>,
    strikes: Vec<SbitStrike>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbitTableKind {
    Eblc,
    Cblc,
}

#[derive(Debug, Clone, Copy)]
struct SbitStrike {
    x_ppem: u8,
    y_ppem: u8,
    bit_depth: u8,
    index_array_offset: u32,
    index_array_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbitStrikeMetrics {
    pub x_ppem: u16,
    pub y_ppem: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbitGlyph {
    pub metrics: SbitMetrics,
    pub bitmap: SbitBitmap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbitMetrics {
    pub width: i32,
    pub height: i32,
    pub hori_bearing_x: i32,
    pub hori_bearing_y: i32,
    pub hori_advance: i32,
    pub vert_bearing_x: i32,
    pub vert_bearing_y: i32,
    pub vert_advance: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbitBitmap {
    pub width: u32,
    pub rows: u32,
    pub pitch: i32,
    pub pixel_mode: SbitPixelMode,
    pub num_grays: u16,
    pub buffer: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbitPixelMode {
    Mono,
    Gray2,
    Gray4,
    Gray,
    Bgra,
}

#[derive(Debug, Clone, Copy)]
struct SbitImageRecord {
    format: u16,
    offset: u32,
    start: u32,
    end: u32,
    metrics: Option<SbitMetrics>,
}

pub fn parse_sbit(directory: &TableDirectory, data: &[u8]) -> Option<SbitTable> {
    let (kind, eblc) = if let Some(cblc) = directory.find(data, tag(b"CBLC")) {
        (SbitTableKind::Cblc, cblc)
    } else if let Some(eblc) = directory.find(data, tag(b"EBLC")) {
        (SbitTableKind::Eblc, eblc)
    } else {
        (SbitTableKind::Eblc, directory.find(data, tag(b"bloc"))?)
    };
    let ebdt = directory
        .find(data, tag(b"CBDT"))
        .or_else(|| directory.find(data, tag(b"EBDT")))
        .or_else(|| directory.find(data, tag(b"bdat")))?;
    if ebdt.is_empty() || eblc.len() < 8 || !valid_eblc_version(read_u32(eblc, 0)?) {
        return None;
    }

    let declared_strikes = read_u32(eblc, 4)?;
    if declared_strikes >= 0x1_0000 {
        return None;
    }
    let physical_strikes = (eblc.len().saturating_sub(8) / 48) as u32;
    let strike_count = declared_strikes.min(physical_strikes);
    let mut strikes = Vec::with_capacity(strike_count as usize);
    for i in 0..strike_count as usize {
        let offset = 8 + i * 48;
        strikes.push(SbitStrike {
            index_array_offset: read_u32(eblc, offset)?,
            index_array_count: read_u32(eblc, offset + 8)?,
            x_ppem: *eblc.get(offset + 44)?,
            y_ppem: *eblc.get(offset + 45)?,
            bit_depth: *eblc.get(offset + 46)?,
        });
    }

    Some(SbitTable {
        kind,
        eblc: eblc.to_vec(),
        ebdt: ebdt.to_vec(),
        strikes,
    })
}

impl SbitTable {
    pub fn kind(&self) -> SbitTableKind {
        self.kind
    }

    pub fn strike_count(&self) -> usize {
        self.strikes.len()
    }

    pub fn strike_metrics(&self, index: usize) -> Option<SbitStrikeMetrics> {
        self.strikes.get(index).map(|strike| SbitStrikeMetrics {
            x_ppem: u16::from(strike.x_ppem),
            y_ppem: u16::from(strike.y_ppem),
        })
    }

    pub fn load_glyph(
        &self,
        glyph_index: u16,
        x_ppem: u16,
        y_ppem: u16,
        recurse_count: u32,
    ) -> Result<SbitGlyph, FontError> {
        let strike = self
            .strikes
            .iter()
            .find(|strike| u16::from(strike.x_ppem) == x_ppem && u16::from(strike.y_ppem) == y_ppem)
            .ok_or_else(|| {
                FontError::InvalidArgument("embedded bitmap strike not selected".into())
            })?;

        strike.find_image(&self.eblc, &self.ebdt, glyph_index, recurse_count)
    }
}

impl SbitStrike {
    fn find_image(
        self,
        eblc: &[u8],
        ebdt: &[u8],
        glyph_index: u16,
        recurse_count: u32,
    ) -> Result<SbitGlyph, FontError> {
        let array_start = self.index_array_offset as usize;
        let count = self.index_array_count as usize;
        let array_len = count.checked_mul(8).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap range array too large".into())
        })?;
        let array_end = array_start.checked_add(array_len).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap range array too large".into())
        })?;
        let Some(array) = eblc.get(array_start..array_end) else {
            return Err(no_bitmap_error(recurse_count));
        };

        for range_index in 0..count {
            let record = range_index * 8;
            let start = read_u16(array, record).ok_or_else(|| no_bitmap_error(recurse_count))?;
            let end = read_u16(array, record + 2).ok_or_else(|| no_bitmap_error(recurse_count))?;
            if glyph_index < start || glyph_index > end {
                continue;
            }

            let subtable_offset =
                read_u32(array, record + 4).ok_or_else(|| no_bitmap_error(recurse_count))? as usize;
            let subtable_start = array_start.checked_add(subtable_offset).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap subtable offset overflow".into())
            })?;
            return find_image_in_subtable(
                self,
                eblc,
                ebdt,
                subtable_start,
                start,
                glyph_index,
                recurse_count,
            );
        }

        Err(no_bitmap_error(recurse_count))
    }
}

fn find_image_in_subtable(
    strike: SbitStrike,
    eblc: &[u8],
    ebdt: &[u8],
    subtable_start: usize,
    first_glyph: u16,
    glyph_index: u16,
    recurse_count: u32,
) -> Result<SbitGlyph, FontError> {
    let Some(header) = eblc.get(subtable_start..subtable_start.saturating_add(8)) else {
        return Err(no_bitmap_error(recurse_count));
    };
    let index_format = read_u16(header, 0).ok_or_else(|| no_bitmap_error(recurse_count))?;
    let image_format = read_u16(header, 2).ok_or_else(|| no_bitmap_error(recurse_count))?;
    let image_offset = read_u32(header, 4).ok_or_else(|| no_bitmap_error(recurse_count))?;

    // C: `tt_sbit_decoder_load_image` in `src/sfnt/ttsbit.c:1241-1441`
    // treats equal EBLC image offsets as NoBitmap; top-level misses return
    // Missing_Bitmap, while recursive misses return Invalid_Composite.
    match index_format {
        1 => {
            let offset_index = usize::from(glyph_index - first_glyph);
            let offsets_start = subtable_offset_start(subtable_start, offset_index, 4)?;
            let image_start =
                read_u32(eblc, offsets_start).ok_or_else(|| no_bitmap_error(recurse_count))?;
            let image_end =
                read_u32(eblc, offsets_start + 4).ok_or_else(|| no_bitmap_error(recurse_count))?;
            image_found_or_missing(
                strike,
                eblc,
                ebdt,
                SbitImageRecord {
                    format: image_format,
                    offset: image_offset,
                    start: image_start,
                    end: image_end,
                    metrics: None,
                },
                recurse_count,
            )
        }
        2 => {
            // C: `tt_sbit_decoder_load_image` in `src/sfnt/ttsbit.c:1302-1318`
            // reads a constant image size plus big metrics from the EBLC index
            // subtable, then derives the selected glyph's EBDT byte range from
            // `(glyph_index - start) * image_size`.
            let image_size =
                read_u32(eblc, subtable_start + 8).ok_or_else(|| no_bitmap_error(recurse_count))?;
            let _metrics = read_big_metrics(
                eblc.get(subtable_start + 12..subtable_start + 20)
                    .ok_or_else(|| no_bitmap_error(recurse_count))?,
            )?;
            let glyph_delta = u32::from(glyph_index - first_glyph);
            let image_start = image_size.checked_mul(glyph_delta).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap image offset overflow".into())
            })?;
            let image_end = image_start.checked_add(image_size).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap image offset overflow".into())
            })?;
            image_found_or_missing(
                strike,
                eblc,
                ebdt,
                SbitImageRecord {
                    format: image_format,
                    offset: image_offset,
                    start: image_start,
                    end: image_end,
                    metrics: None,
                },
                recurse_count,
            )
        }
        3 => {
            let offset_index = usize::from(glyph_index - first_glyph);
            let offsets_start = subtable_offset_start(subtable_start, offset_index, 2)?;
            let image_start = u32::from(
                read_u16(eblc, offsets_start).ok_or_else(|| no_bitmap_error(recurse_count))?,
            );
            let image_end = u32::from(
                read_u16(eblc, offsets_start + 2).ok_or_else(|| no_bitmap_error(recurse_count))?,
            );
            image_found_or_missing(
                strike,
                eblc,
                ebdt,
                SbitImageRecord {
                    format: image_format,
                    offset: image_offset,
                    start: image_start,
                    end: image_end,
                    metrics: None,
                },
                recurse_count,
            )
        }
        4 => {
            // C: `tt_sbit_decoder_load_image` in `src/sfnt/ttsbit.c:1332-1365`
            // scans sparse `(glyph, offset)` pairs and uses the following
            // pair's offset as the matched glyph's image end.
            let num_glyphs =
                read_u32(eblc, subtable_start + 8).ok_or_else(|| no_bitmap_error(recurse_count))?;
            let entries_start = subtable_start.checked_add(12).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap sparse array overflow".into())
            })?;
            let entries = usize::try_from(num_glyphs).map_err(|_| {
                FontError::InvalidFont("embedded bitmap sparse array too large".into())
            })?;
            let table_entries = entries.checked_add(1).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap sparse array too large".into())
            })?;
            let entries_len = table_entries.checked_mul(4).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap sparse array too large".into())
            })?;
            let entries_end = entries_start.checked_add(entries_len).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap sparse array too large".into())
            })?;
            eblc.get(entries_start..entries_end)
                .ok_or_else(|| no_bitmap_error(recurse_count))?;

            for entry_index in 0..entries {
                let entry = entries_start + entry_index * 4;
                let sparse_glyph =
                    read_u16(eblc, entry).ok_or_else(|| no_bitmap_error(recurse_count))?;
                if sparse_glyph != glyph_index {
                    continue;
                }
                let image_start = u32::from(
                    read_u16(eblc, entry + 2).ok_or_else(|| no_bitmap_error(recurse_count))?,
                );
                let image_end = u32::from(
                    read_u16(eblc, entry + 6).ok_or_else(|| no_bitmap_error(recurse_count))?,
                );
                return image_found_or_missing(
                    strike,
                    eblc,
                    ebdt,
                    SbitImageRecord {
                        format: image_format,
                        offset: image_offset,
                        start: image_start,
                        end: image_end,
                        metrics: None,
                    },
                    recurse_count,
                );
            }

            Err(no_bitmap_error(recurse_count))
        }
        5 => {
            // C: `tt_sbit_decoder_load_image` in `src/sfnt/ttsbit.c:1367-1401`
            // reads EBLC constant metrics and a sparse glyph-code array, then
            // image format 5 uses those metrics for bit-aligned EBDT payloads.
            let image_size =
                read_u32(eblc, subtable_start + 8).ok_or_else(|| no_bitmap_error(recurse_count))?;
            let metrics = read_big_metrics(
                eblc.get(subtable_start + 12..subtable_start + 20)
                    .ok_or_else(|| no_bitmap_error(recurse_count))?,
            )?;
            let num_glyphs = read_u32(eblc, subtable_start + 20)
                .ok_or_else(|| no_bitmap_error(recurse_count))?;
            let glyphs_start = subtable_start.checked_add(24).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap sparse glyph array overflow".into())
            })?;
            let entries = usize::try_from(num_glyphs).map_err(|_| {
                FontError::InvalidFont("embedded bitmap sparse glyph array too large".into())
            })?;
            let glyphs_len = entries.checked_mul(2).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap sparse glyph array too large".into())
            })?;
            let glyphs_end = glyphs_start.checked_add(glyphs_len).ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap sparse glyph array too large".into())
            })?;
            eblc.get(glyphs_start..glyphs_end)
                .ok_or_else(|| no_bitmap_error(recurse_count))?;
            for entry_index in 0..entries {
                let entry = glyphs_start + entry_index * 2;
                let sparse_glyph =
                    read_u16(eblc, entry).ok_or_else(|| no_bitmap_error(recurse_count))?;
                if sparse_glyph != glyph_index {
                    continue;
                }
                let glyph_offset = u32::try_from(entry_index).map_err(|_| {
                    FontError::InvalidFont("embedded bitmap sparse glyph array too large".into())
                })?;
                let image_start = image_size.checked_mul(glyph_offset).ok_or_else(|| {
                    FontError::InvalidFont("embedded bitmap image offset overflow".into())
                })?;
                let image_end = image_start.checked_add(image_size).ok_or_else(|| {
                    FontError::InvalidFont("embedded bitmap image offset overflow".into())
                })?;
                return image_found_or_missing(
                    strike,
                    eblc,
                    ebdt,
                    SbitImageRecord {
                        format: image_format,
                        offset: image_offset,
                        start: image_start,
                        end: image_end,
                        metrics: Some(metrics),
                    },
                    recurse_count,
                );
            }

            Err(no_bitmap_error(recurse_count))
        }
        _ => Err(no_bitmap_error(recurse_count)),
    }
}

fn subtable_offset_start(
    subtable_start: usize,
    offset_index: usize,
    offset_size: usize,
) -> Result<usize, FontError> {
    let relative = offset_index
        .checked_mul(offset_size)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap offset array too large".into()))?;
    let relative = relative
        .checked_add(8)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap offset array too large".into()))?;
    subtable_start
        .checked_add(relative)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap offset array too large".into()))
}

fn image_found_or_missing(
    strike: SbitStrike,
    eblc: &[u8],
    ebdt: &[u8],
    image: SbitImageRecord,
    recurse_count: u32,
) -> Result<SbitGlyph, FontError> {
    if image.start >= image.end {
        return Err(no_bitmap_error(recurse_count));
    }
    if image.format == 8 || image.format == 9 {
        return load_compound_image(strike, eblc, ebdt, image, recurse_count);
    }
    load_simple_image(strike, ebdt, image)
}

fn load_simple_image(
    strike: SbitStrike,
    ebdt: &[u8],
    image_record: SbitImageRecord,
) -> Result<SbitGlyph, FontError> {
    // FreeType `sfnt/ttsbit.c:544-589,700-743,828-955` routes image format 1
    // through the byte-aligned decoder and image format 5 through the
    // bit-aligned decoder after mapping bit depths to pixel modes.
    if image_record.format != 1 && image_record.format != 5 {
        return Err(FontError::InvalidFont(format!(
            "unsupported embedded bitmap image format {}",
            image_record.format
        )));
    }
    let start = image_record
        .offset
        .checked_add(image_record.start)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap image offset overflow".into()))?;
    let end = image_record
        .offset
        .checked_add(image_record.end)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap image offset overflow".into()))?;
    let start = usize::try_from(start).map_err(|_| {
        FontError::InvalidFont("embedded bitmap image offset does not fit usize".into())
    })?;
    let end = usize::try_from(end).map_err(|_| {
        FontError::InvalidFont("embedded bitmap image offset does not fit usize".into())
    })?;
    let image = ebdt
        .get(start..end)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap image exceeds data".into()))?;
    if image_record.format == 5 {
        return load_bit_aligned_image(
            strike,
            image,
            image_record.metrics.ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap bit-aligned metrics missing".into())
            })?,
        );
    }
    let raw_height = *image
        .first()
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap small metrics missing".into()))?;
    let raw_width = *image
        .get(1)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap small metrics missing".into()))?;
    let width = usize::from(raw_width);
    let (pixel_mode, row_bytes, num_grays) = bitmap_layout_for_bit_depth(strike.bit_depth, width)?;
    let metrics = read_small_metrics(image)?;
    let bitmap_start = 5usize;
    let rows = usize::from(raw_height);
    let bitmap_len = row_bytes
        .checked_mul(rows)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap buffer length overflow".into()))?;
    let bitmap_end = bitmap_start
        .checked_add(bitmap_len)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap buffer offset overflow".into()))?;
    let buffer = image
        .get(bitmap_start..bitmap_end)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap image data truncated".into()))?
        .to_vec();
    Ok(SbitGlyph {
        metrics,
        bitmap: SbitBitmap {
            width: u32::from(raw_width),
            rows: u32::from(raw_height),
            pitch: row_bytes as i32,
            pixel_mode,
            num_grays,
            buffer,
        },
    })
}

fn load_bit_aligned_image(
    strike: SbitStrike,
    image: &[u8],
    metrics: SbitMetrics,
) -> Result<SbitGlyph, FontError> {
    let width = metric_dimension(metrics.width);
    let rows = metric_dimension(metrics.height);
    let (pixel_mode, row_bytes, num_grays) = bitmap_layout_for_bit_depth(strike.bit_depth, width)?;
    let line_bits = width
        .checked_mul(usize::from(strike.bit_depth))
        .ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap bit-aligned line overflow".into())
        })?;
    let total_bits = line_bits.checked_mul(rows).ok_or_else(|| {
        FontError::InvalidFont("embedded bitmap bit-aligned buffer overflow".into())
    })?;
    let payload_len = total_bits.div_ceil(8);
    let payload = image.get(0..payload_len).ok_or_else(|| {
        FontError::InvalidFont("embedded bitmap bit-aligned image data truncated".into())
    })?;
    let mut buffer = vec![
        0;
        row_bytes.checked_mul(rows).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap bit-aligned target overflow".into())
        })?
    ];
    for bit_index in 0..total_bits {
        let source_byte = payload[bit_index / 8];
        let source_mask = 0x80u8 >> (bit_index & 7);
        if source_byte & source_mask == 0 {
            continue;
        }
        let row = bit_index / line_bits;
        let bit_in_row = bit_index % line_bits;
        let target_index = row
            .checked_mul(row_bytes)
            .and_then(|start| start.checked_add(bit_in_row / 8))
            .ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap bit-aligned target overflow".into())
            })?;
        buffer[target_index] |= 0x80u8 >> (bit_in_row & 7);
    }
    Ok(SbitGlyph {
        metrics,
        bitmap: SbitBitmap {
            width: width as u32,
            rows: rows as u32,
            pitch: row_bytes as i32,
            pixel_mode,
            num_grays,
            buffer,
        },
    })
}

fn load_compound_image(
    strike: SbitStrike,
    eblc: &[u8],
    ebdt: &[u8],
    image_record: SbitImageRecord,
    recurse_count: u32,
) -> Result<SbitGlyph, FontError> {
    let start = image_record
        .offset
        .checked_add(image_record.start)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap image offset overflow".into()))?;
    let end = image_record
        .offset
        .checked_add(image_record.end)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap image offset overflow".into()))?;
    let start = usize::try_from(start).map_err(|_| {
        FontError::InvalidFont("embedded bitmap image offset does not fit usize".into())
    })?;
    let end = usize::try_from(end).map_err(|_| {
        FontError::InvalidFont("embedded bitmap image offset does not fit usize".into())
    })?;
    let image = ebdt
        .get(start..end)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap image exceeds data".into()))?;
    let (metrics, component_start) = match image_record.format {
        8 => (read_small_metrics(image)?, 6usize),
        9 => (read_big_metrics(image)?, 8usize),
        _ => unreachable!("compound image loader only accepts image formats 8 and 9"),
    };
    let mut glyph = blank_compound_glyph(strike, metrics)?;
    let num_components = read_u16(image, component_start)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap compound count missing".into()))?;
    let records_start = component_start
        .checked_add(2)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap compound offset overflow".into()))?;
    let records_len = usize::from(num_components).checked_mul(4).ok_or_else(|| {
        FontError::InvalidFont("embedded bitmap compound record length overflow".into())
    })?;
    let records_end = records_start
        .checked_add(records_len)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap compound record overflow".into()))?;
    let records = image.get(records_start..records_end).ok_or_else(|| {
        FontError::InvalidFont("embedded bitmap compound record truncated".into())
    })?;
    // FreeType `sfnt/ttsbit.c:961-1012` allocates the root bitmap from the
    // compound metrics, ORs each recursively loaded component into that
    // canvas, then restores the root metrics.
    for component in records.chunks_exact(4) {
        let gindex = read_u16(component, 0).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap component glyph missing".into())
        })?;
        let dx = i32::from(component[2] as i8);
        let dy = i32::from(component[3] as i8);
        let component = strike.find_image(eblc, ebdt, gindex, recurse_count + 1)?;
        blit_component_bitmap(&mut glyph.bitmap, &component.bitmap, dx, dy)?;
    }
    Ok(glyph)
}

fn read_small_metrics(data: &[u8]) -> Result<SbitMetrics, FontError> {
    let bytes = data
        .get(0..5)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap small metrics missing".into()))?;
    Ok(SbitMetrics {
        height: i32::from(bytes[0]) * 64,
        width: i32::from(bytes[1]) * 64,
        hori_bearing_x: i32::from(bytes[2] as i8) * 64,
        hori_bearing_y: i32::from(bytes[3] as i8) * 64,
        hori_advance: i32::from(bytes[4]) * 64,
        vert_bearing_x: 0,
        vert_bearing_y: 0,
        vert_advance: 0,
    })
}

fn read_big_metrics(data: &[u8]) -> Result<SbitMetrics, FontError> {
    let bytes = data
        .get(0..8)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap big metrics missing".into()))?;
    Ok(SbitMetrics {
        height: i32::from(bytes[0]) * 64,
        width: i32::from(bytes[1]) * 64,
        hori_bearing_x: i32::from(bytes[2] as i8) * 64,
        hori_bearing_y: i32::from(bytes[3] as i8) * 64,
        hori_advance: i32::from(bytes[4]) * 64,
        vert_bearing_x: i32::from(bytes[5] as i8) * 64,
        vert_bearing_y: i32::from(bytes[6] as i8) * 64,
        vert_advance: i32::from(bytes[7]) * 64,
    })
}

fn blank_compound_glyph(strike: SbitStrike, metrics: SbitMetrics) -> Result<SbitGlyph, FontError> {
    let width = metric_dimension(metrics.width);
    let rows = metric_dimension(metrics.height);
    let (pixel_mode, row_bytes, num_grays) = bitmap_layout_for_bit_depth(strike.bit_depth, width)?;
    let len = row_bytes.checked_mul(rows).ok_or_else(|| {
        FontError::InvalidFont("embedded bitmap compound buffer length overflow".into())
    })?;
    Ok(SbitGlyph {
        metrics,
        bitmap: SbitBitmap {
            width: width as u32,
            rows: rows as u32,
            pitch: row_bytes as i32,
            pixel_mode,
            num_grays,
            buffer: vec![0; len],
        },
    })
}

fn metric_dimension(value: i32) -> usize {
    (value / 64) as usize
}

fn bitmap_layout_for_bit_depth(
    bit_depth: u8,
    width: usize,
) -> Result<(SbitPixelMode, usize, u16), FontError> {
    match bit_depth {
        1 => Ok((SbitPixelMode::Mono, width.div_ceil(8), 2)),
        2 => Ok((SbitPixelMode::Gray2, width.div_ceil(4), 4)),
        4 => Ok((SbitPixelMode::Gray4, width.div_ceil(2), 16)),
        8 => Ok((SbitPixelMode::Gray, width, 256)),
        32 => Ok((SbitPixelMode::Bgra, width * 4, 256)),
        depth => Err(FontError::InvalidFont(format!(
            "unsupported embedded bitmap bit depth {depth}"
        ))),
    }
}

fn blit_component_bitmap(
    target: &mut SbitBitmap,
    component: &SbitBitmap,
    dx: i32,
    dy: i32,
) -> Result<(), FontError> {
    if dx < 0 || dy < 0 {
        return Err(FontError::InvalidFont(
            "embedded bitmap compound component outside target".into(),
        ));
    }
    let dx = dx as u32;
    let dy = dy as u32;
    let right = dx + component.width;
    let bottom = dy + component.rows;
    if right > target.width || bottom > target.rows {
        return Err(FontError::InvalidFont(
            "embedded bitmap compound component outside target".into(),
        ));
    }

    let target_pitch = usize::try_from(target.pitch)
        .map_err(|_| FontError::InvalidFont("embedded bitmap target pitch invalid".into()))?;
    let component_pitch = usize::try_from(component.pitch)
        .map_err(|_| FontError::InvalidFont("embedded bitmap component pitch invalid".into()))?;
    if let Some(bit_depth) = packed_bit_depth(target.pixel_mode) {
        return blit_packed_component_bitmap(
            target,
            component,
            dx as usize,
            dy as usize,
            target_pitch,
            component_pitch,
            bit_depth,
        );
    }

    let bytes_per_pixel = match target.pixel_mode {
        SbitPixelMode::Bgra => 4,
        SbitPixelMode::Gray => 1,
        SbitPixelMode::Mono | SbitPixelMode::Gray2 | SbitPixelMode::Gray4 => unreachable!(),
    };
    let target_x = (dx as usize).checked_mul(bytes_per_pixel).ok_or_else(|| {
        FontError::InvalidFont("embedded bitmap compound x offset overflow".into())
    })?;
    let row_bytes = component_pitch;
    for row in 0..component.rows as usize {
        let target_start = (dy as usize + row)
            .checked_mul(target_pitch)
            .and_then(|start| start.checked_add(target_x))
            .ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap compound target offset overflow".into())
            })?;
        let component_start = row.checked_mul(component_pitch).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap compound component offset overflow".into())
        })?;
        let target_end = target_start.checked_add(row_bytes).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap compound target row overflow".into())
        })?;
        let component_end = component_start.checked_add(row_bytes).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap compound component row overflow".into())
        })?;
        let target_row = target
            .buffer
            .get_mut(target_start..target_end)
            .ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap compound target row truncated".into())
            })?;
        let component_row = component
            .buffer
            .get(component_start..component_end)
            .ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap compound component row truncated".into())
            })?;
        for (target_byte, component_byte) in target_row.iter_mut().zip(component_row) {
            *target_byte |= component_byte;
        }
    }
    Ok(())
}

fn packed_bit_depth(pixel_mode: SbitPixelMode) -> Option<usize> {
    match pixel_mode {
        SbitPixelMode::Mono => Some(1),
        SbitPixelMode::Gray2 => Some(2),
        SbitPixelMode::Gray4 => Some(4),
        SbitPixelMode::Gray | SbitPixelMode::Bgra => None,
    }
}

fn blit_packed_component_bitmap(
    target: &mut SbitBitmap,
    component: &SbitBitmap,
    dx: usize,
    dy: usize,
    target_pitch: usize,
    component_pitch: usize,
    bit_depth: usize,
) -> Result<(), FontError> {
    // FreeType `sfnt/ttsbit.c:730-782` treats compound x offsets as bit
    // shifts for byte-aligned packed SBIT components, then ORs shifted bytes
    // into the root bitmap.
    let line_bits = (component.width as usize)
        .checked_mul(bit_depth)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap compound line overflow".into()))?;
    if line_bits == 0 || component.rows == 0 {
        return Ok(());
    }
    let row_bytes = line_bits.div_ceil(8);
    let x_byte = dx >> 3;
    let x_shift = dx & 7;

    for row in 0..component.rows as usize {
        let target_start = dy
            .checked_add(row)
            .and_then(|y| y.checked_mul(target_pitch))
            .and_then(|start| start.checked_add(x_byte))
            .ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap compound target offset overflow".into())
            })?;
        let component_start = row.checked_mul(component_pitch).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap compound component offset overflow".into())
        })?;
        let component_end = component_start.checked_add(row_bytes).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap compound component row overflow".into())
        })?;
        let target_len = (x_shift + line_bits).div_ceil(8);
        let target_end = target_start.checked_add(target_len).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap compound target row overflow".into())
        })?;
        let target_row = target
            .buffer
            .get_mut(target_start..target_end)
            .ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap compound target row truncated".into())
            })?;
        let component_row = component
            .buffer
            .get(component_start..component_end)
            .ok_or_else(|| {
                FontError::InvalidFont("embedded bitmap compound component row truncated".into())
            })?;

        if x_shift == 0 {
            let mut index = 0usize;
            let mut remaining_bits = line_bits;
            while remaining_bits >= 8 {
                target_row[index] |= component_row[index];
                index += 1;
                remaining_bits -= 8;
            }
            if remaining_bits > 0 {
                let mask = 0xFF00u32 >> remaining_bits;
                target_row[index] |= (u32::from(component_row[index]) & mask) as u8;
            }
        } else {
            let mut source_index = 0usize;
            let mut target_index = 0usize;
            let mut remaining_bits = line_bits;
            let mut wval = 0u32;

            while remaining_bits >= 8 {
                wval |= u32::from(component_row[source_index]);
                target_row[target_index] |= (wval >> x_shift) as u8;
                target_index += 1;
                source_index += 1;
                wval <<= 8;
                remaining_bits -= 8;
            }

            if remaining_bits > 0 {
                let mask = 0xFF00u32 >> remaining_bits;
                wval |= u32::from(component_row[source_index]) & mask;
            }

            target_row[target_index] |= (wval >> x_shift) as u8;
            if x_shift + remaining_bits > 8 {
                target_index += 1;
                wval <<= 8;
                target_row[target_index] |= (wval >> x_shift) as u8;
            }
        }
    }

    Ok(())
}

fn no_bitmap_error(recurse_count: u32) -> FontError {
    if recurse_count == 0 {
        FontError::MissingBitmap
    } else {
        FontError::InvalidComposite
    }
}

fn valid_eblc_version(version: u32) -> bool {
    // C: `sfnt/ttsbit.c:116-125` also accepts the byte-swapped major field
    // used by FZShuSong-Z01, so `0x00000200` and `0x00000300` are valid too.
    let major = version & 0xFFFF_0000;
    let byte_swapped_major = version & 0x0000_FFFF;
    matches!(major, 0x0002_0000 | 0x0003_0000)
        || matches!(byte_swapped_major, 0x0000_0200 | 0x0000_0300)
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let bytes: [u8; 2] = data.get(offset..end)?.try_into().ok()?;
    Some(u16::from_be_bytes(bytes))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let bytes: [u8; 4] = data.get(offset..end)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

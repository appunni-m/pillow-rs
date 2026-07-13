//! Embedded bitmap strike metadata for EBLC/EBDT TrueType tables.

use crate::error::FontError;
use crate::tt::{TableDirectory, tag};

#[derive(Debug, Clone)]
pub struct SbitTable {
    eblc: Vec<u8>,
    ebdt: Vec<u8>,
    strikes: Vec<SbitStrike>,
}

#[derive(Debug, Clone, Copy)]
struct SbitStrike {
    x_ppem: u8,
    y_ppem: u8,
    bit_depth: u8,
    index_array_offset: u32,
    index_array_count: u32,
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
    pub num_grays: u16,
    pub buffer: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct SbitImageRecord {
    format: u16,
    offset: u32,
    start: u32,
    end: u32,
}

pub fn parse_sbit(directory: &TableDirectory, data: &[u8]) -> Option<SbitTable> {
    let eblc = directory
        .find(data, tag(b"EBLC"))
        .or_else(|| directory.find(data, tag(b"CBLC")))
        .or_else(|| directory.find(data, tag(b"bloc")))?;
    let ebdt = directory
        .find(data, tag(b"EBDT"))
        .or_else(|| directory.find(data, tag(b"CBDT")))
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
        eblc: eblc.to_vec(),
        ebdt: ebdt.to_vec(),
        strikes,
    })
}

impl SbitTable {
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

    pub fn load_glyph_status(
        &self,
        glyph_index: u16,
        x_ppem: u16,
        y_ppem: u16,
        recurse_count: u32,
    ) -> Result<(), FontError> {
        self.load_glyph(glyph_index, x_ppem, y_ppem, recurse_count)
            .map(|_| ())
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
                },
                recurse_count,
            )
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
    // FreeType `sfnt/ttsbit.c:1083-1130` routes image format 1 through the
    // byte-aligned decoder; for 8-bit strikes `sfnt/ttsbit.c:543-611` exposes
    // a gray bitmap with pitch equal to width.
    if image_record.format != 1 {
        return Err(FontError::InvalidFont(format!(
            "unsupported embedded bitmap image format {}",
            image_record.format
        )));
    }
    if strike.bit_depth != 8 {
        return Err(FontError::InvalidFont(format!(
            "unsupported embedded bitmap bit depth {}",
            strike.bit_depth
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
    let raw_height = *image
        .first()
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap small metrics missing".into()))?;
    let raw_width = *image
        .get(1)
        .ok_or_else(|| FontError::InvalidFont("embedded bitmap small metrics missing".into()))?;
    let metrics = read_small_metrics(image)?;
    let bitmap_start = 5usize;
    let row_bytes = usize::from(raw_width);
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
            pitch: i32::from(raw_width),
            num_grays: 256,
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
    let component_start = match image_record.format {
        8 => 6,
        9 => 8,
        _ => unreachable!("compound image loader only accepts image formats 8 and 9"),
    };
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
    for component in records.chunks_exact(4) {
        let gindex = read_u16(component, 0).ok_or_else(|| {
            FontError::InvalidFont("embedded bitmap component glyph missing".into())
        })?;
        strike.find_image(eblc, ebdt, gindex, recurse_count + 1)?;
    }
    Err(FontError::UnsupportedLoadFlags(
        "embedded bitmap compound image decoding".into(),
    ))
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

fn no_bitmap_error(recurse_count: u32) -> FontError {
    if recurse_count == 0 {
        FontError::MissingBitmap
    } else {
        FontError::InvalidComposite
    }
}

fn valid_eblc_version(version: u32) -> bool {
    let major = version & 0xFFFF_0000;
    major == 0x0002_0000 || major == 0x0003_0000
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

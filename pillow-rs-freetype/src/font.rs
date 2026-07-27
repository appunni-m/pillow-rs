//! FreeType-compatible font face API implemented in pure Rust.
//!
//! Runtime code follows FreeType glyph-slot behavior. Higher-level adapters,
//! including text layout or framework-specific packaging, live outside this
//! crate.

use crate::casts::{i16_from_i32, i32_from_f32, u32_from_i64, u32_from_usize, usize_from_i32};

use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_div, ft_mul_fix};
use crate::outline::{Outline, OutlinePoint};
use crate::scaler::{self, ft_pix_ceil, ft_pix_floor, ft_pix_round, pixel_round};
use crate::tables::FontData;
use crate::tt::hinter::NativeHintMode;
use crate::tt::{self, tag};
use std::ffi::{CStr, CString};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

/// FreeType glyph load behavior used by high-level render helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadMode {
    /// `FT_LOAD_RENDER`: use the font's native TrueType program when present.
    #[default]
    Default,
    /// `FT_LOAD_RENDER | FT_LOAD_FORCE_AUTOHINT`: force the auto-hinter.
    ForceAutoHint,
    /// `FT_LOAD_TARGET_LIGHT`: auto-hint with vertical-only light target behavior.
    TargetLight,
    /// `FT_LOAD_NO_HINTING`: scale outlines without native or automatic hinting.
    NoHinting,
    /// `FT_LOAD_NO_AUTOHINT`: prefer native hints, but do not fall back to autohinting.
    NoAutoHint,
}

/// Public `FT_Get_Kerning` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KerningMode {
    /// `FT_KERNING_DEFAULT`: scaled and grid-fitted 26.6 pixel values.
    Default,
    /// `FT_KERNING_UNFITTED`: scaled but un-grid-fitted 26.6 pixel values.
    Unfitted,
    /// `FT_KERNING_UNSCALED`: original font-unit values.
    Unscaled,
}

/// A loaded TrueType font at a given point size.
#[derive(Clone)]
pub struct Font {
    pub data: Arc<FontData>,
    pub size_pt: f32,
    pub load_mode: LoadMode,
    face_kind: FaceKind,
    type1_font_info: Option<Type1FontInfo>,
    type1_encoding: Option<Type1EncodingInfo>,
    type1_private: Option<Type1PrivateDict>,
    type1_charstrings: Vec<Type1CharString>,
    type1_multi_master: Option<Arc<Type1MultiMaster>>,
    type1_mm_weight_vector: Option<Vec<i32>>,
    type1_mm_variation_active: bool,
    /// Face-level global hinting data: per-glyph script assignment,
    /// lazy-computed per-style metrics (Latin, Greek, etc.).
    /// Matches FreeType's AF_FaceGlobals.
    pub face_globals: crate::autohint::globals::FaceGlobals,
    /// Whether the font is italic/oblique (from head.mac_style bit 1).
    pub is_italic: bool,
    family_name: String,
    subfamily_name: String,
    bdf_properties: Vec<BdfPropertyEntry>,
    size_metrics: SizeMetrics,
    selected_charmap: usize,
    bytecode_context: BytecodeContextCache,
    /// Reusable raster scratch space for gray rasterizer passes.
    /// Avoids allocating scanline cell vectors on every glyph render.
    pub(crate) raster_scratch: std::cell::RefCell<crate::grays::RasterScratch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BdfPropertyEntry {
    name: String,
    value: BdfPropertyValue,
    atom_c_string: Option<CString>,
}

/// Value returned for one parsed BDF font property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BdfPropertyValue {
    /// String atom property.
    Atom(String),
    /// Signed integer property.
    Integer(i32),
    /// Unsigned integer property.
    Cardinal(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaceKind {
    Sfnt,
    Bdf,
    Type1 { is_fixed_pitch: bool },
    WinFnt { header: WinFntHeader },
}

/// Parsed Windows FNT header returned by `FT_Get_WinFNT_Header`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WinFntHeader {
    pub version: u16,
    pub file_size: u32,
    pub copyright: [u8; 60],
    pub file_type: u16,
    pub nominal_point_size: u16,
    pub vertical_resolution: u16,
    pub horizontal_resolution: u16,
    pub ascent: u16,
    pub internal_leading: u16,
    pub external_leading: u16,
    pub italic: u8,
    pub underline: u8,
    pub strike_out: u8,
    pub weight: u16,
    pub charset: u8,
    pub pixel_width: u16,
    pub pixel_height: u16,
    pub pitch_and_family: u8,
    pub avg_width: u16,
    pub max_width: u16,
    pub first_char: u8,
    pub last_char: u8,
    pub default_char: u8,
    pub break_char: u8,
    pub bytes_per_row: u16,
    pub device_offset: u32,
    pub face_name_offset: u32,
    pub bits_pointer: u32,
    pub bits_offset: u32,
    pub reserved: u8,
    pub flags: u32,
    pub a_space: u16,
    pub b_space: u16,
    pub c_space: u16,
    pub color_table_offset: u32,
    pub reserved1: [u64; 4],
}

struct Type1Metadata {
    version: Option<String>,
    notice: Option<String>,
    full_name: Option<String>,
    font_name: String,
    family_name: String,
    style_name: String,
    italic_angle: i16,
    is_fixed_pitch: bool,
    underline_position: i16,
    underline_thickness: i16,
    bbox: BBox,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Type1FontInfo {
    pub version: Option<String>,
    pub notice: Option<String>,
    pub full_name: Option<String>,
    pub family_name: Option<String>,
    pub weight: Option<String>,
    pub italic_angle: i32,
    pub is_fixed_pitch: bool,
    pub underline_position: i16,
    pub underline_thickness: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Type1EncodingInfo {
    pub encoding_type: i32,
    pub entries: Vec<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Type1PrivateDict {
    pub unique_id: i32,
    pub len_iv: i32,
    pub num_blue_values: u8,
    pub num_other_blues: u8,
    pub num_family_blues: u8,
    pub num_family_other_blues: u8,
    pub blue_values: [i16; 14],
    pub other_blues: [i16; 10],
    pub family_blues: [i16; 14],
    pub family_other_blues: [i16; 10],
    pub blue_scale: i32,
    pub blue_shift: i32,
    pub blue_fuzz: i32,
    pub standard_width: [u16; 1],
    pub standard_height: [u16; 1],
    pub num_snap_widths: u8,
    pub num_snap_heights: u8,
    pub force_bold: bool,
    pub round_stem_up: bool,
    pub snap_widths: [i16; 13],
    pub snap_heights: [i16; 13],
    pub expansion_factor: i32,
    pub language_group: i64,
    pub password: i64,
    pub min_feature: [i16; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Type1CharString {
    name: String,
    encrypted: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Type1GlyphProgram {
    advance_width: i32,
    outline: Type1GlyphOutline,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Type1GlyphOutline {
    points: Vec<OutlinePoint>,
    contours: Vec<i16>,
}

struct BdfMetadata {
    family_name: String,
    pixel_width: i16,
    pixel_height: i16,
    x_offset: i16,
    y_offset: i16,
    glyph_count: u16,
    properties: Vec<BdfPropertyEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Type1MultiMaster {
    pub axes: Vec<Type1MultiMasterAxis>,
    pub num_designs: usize,
    pub default_weight_vector: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Type1MultiMasterAxis {
    pub name: String,
    pub minimum: i32,
    pub maximum: i32,
    design_map: Vec<Type1DesignMapPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Type1DesignMapPoint {
    design: i32,
    blend: i32,
}

fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn parse_winfnt_header(data: &[u8]) -> Result<WinFntHeader, FontError> {
    const WINFNT_V2_HEADER_SIZE: usize = 118;
    const WINFNT_V3_HEADER_SIZE: usize = 148;

    if data.len() < WINFNT_V2_HEADER_SIZE {
        return Err(FontError::InvalidFont("not a Windows FNT file".into()));
    }
    let version =
        read_u16_le(data, 0).ok_or_else(|| FontError::InvalidFont("short FNT header".into()))?;
    if version != 0x0200 && version != 0x0300 {
        return Err(FontError::InvalidFont("not a Windows FNT file".into()));
    }
    let required_size = if version == 0x0300 {
        WINFNT_V3_HEADER_SIZE
    } else {
        WINFNT_V2_HEADER_SIZE
    };
    if data.len() < required_size {
        return Err(FontError::InvalidFont("short Windows FNT header".into()));
    }
    let file_size = read_u32_le(data, 2)
        .ok_or_else(|| FontError::InvalidFont("missing Windows FNT file size".into()))?;
    let declared_size = usize::try_from(file_size)
        .map_err(|_| FontError::InvalidFont("Windows FNT file size out of range".into()))?;
    // FreeType winfnt.c:fnt_font_load reads `file_size` bytes into the FNT
    // frame after accepting versions 0x200/0x300; a truncated stream fails
    // before the WINFNT service can expose the copied header.
    if file_size < required_size as u32 || declared_size > data.len() {
        return Err(FontError::InvalidFont(
            "invalid Windows FNT file size".into(),
        ));
    }
    let file_type = read_u16_le(data, 66)
        .ok_or_else(|| FontError::InvalidFont("missing Windows FNT file type".into()))?;
    if file_type & 1 != 0 {
        return Err(FontError::InvalidFont(
            "Windows FNT vector fonts are unsupported".into(),
        ));
    }

    let pixel_height = read_u16_le(data, 88)
        .ok_or_else(|| FontError::InvalidFont("missing Windows FNT pixel height".into()))?;
    let first_char = *data
        .get(95)
        .ok_or_else(|| FontError::InvalidFont("missing Windows FNT first char".into()))?;
    let last_char = *data
        .get(96)
        .ok_or_else(|| FontError::InvalidFont("missing Windows FNT last char".into()))?;
    let face_name_offset = read_u32_le(data, 105)
        .ok_or_else(|| FontError::InvalidFont("missing Windows FNT face name offset".into()))?;
    if pixel_height == 0
        || last_char < first_char
        || usize::try_from(face_name_offset).map_or(true, |offset| offset >= declared_size)
    {
        return Err(FontError::InvalidFont(
            "invalid Windows FNT face metadata".into(),
        ));
    }

    let mut copyright = [0u8; 60];
    copyright.copy_from_slice(&data[6..66]);
    Ok(WinFntHeader {
        version,
        file_size,
        copyright,
        file_type,
        nominal_point_size: read_u16_le(data, 68).unwrap_or(0),
        vertical_resolution: read_u16_le(data, 70).unwrap_or(0),
        horizontal_resolution: read_u16_le(data, 72).unwrap_or(0),
        ascent: read_u16_le(data, 74).unwrap_or(0),
        internal_leading: read_u16_le(data, 76).unwrap_or(0),
        external_leading: read_u16_le(data, 78).unwrap_or(0),
        italic: data[80],
        underline: data[81],
        strike_out: data[82],
        weight: read_u16_le(data, 83).unwrap_or(0),
        charset: data[85],
        pixel_width: read_u16_le(data, 86).unwrap_or(0),
        pixel_height,
        pitch_and_family: data[90],
        avg_width: read_u16_le(data, 91).unwrap_or(0),
        max_width: read_u16_le(data, 93).unwrap_or(0),
        first_char,
        last_char,
        default_char: data[97],
        break_char: data[98],
        bytes_per_row: read_u16_le(data, 99).unwrap_or(0),
        device_offset: read_u32_le(data, 101).unwrap_or(0),
        face_name_offset,
        bits_pointer: read_u32_le(data, 109).unwrap_or(0),
        bits_offset: read_u32_le(data, 113).unwrap_or(0),
        reserved: data[117],
        flags: if version == 0x0300 {
            read_u32_le(data, 118).unwrap_or(0)
        } else {
            0
        },
        a_space: if version == 0x0300 {
            read_u16_le(data, 122).unwrap_or(0)
        } else {
            0
        },
        b_space: if version == 0x0300 {
            read_u16_le(data, 124).unwrap_or(0)
        } else {
            0
        },
        c_space: if version == 0x0300 {
            read_u16_le(data, 126).unwrap_or(0)
        } else {
            0
        },
        color_table_offset: if version == 0x0300 {
            read_u32_le(data, 128).unwrap_or(0)
        } else {
            0
        },
        reserved1: if version == 0x0300 {
            // FreeType winfnt.c reads 16 raw bytes into FT_ULong reserved1[4].
            // On the maintained LP64 ABI that fills the first two 64-bit
            // elements and leaves the remaining elements zeroed.
            [
                u64::from_le_bytes([
                    data[132], data[133], data[134], data[135], data[136], data[137], data[138],
                    data[139],
                ]),
                u64::from_le_bytes([
                    data[140], data[141], data[142], data[143], data[144], data[145], data[146],
                    data[147],
                ]),
                0,
                0,
            ]
        } else {
            [0; 4]
        },
    })
}

fn winfnt_family_name(data: &[u8], header: &WinFntHeader) -> String {
    let start = usize::try_from(header.face_name_offset).unwrap_or(data.len());
    let bytes = data.get(start..usize::try_from(header.file_size).unwrap_or(data.len()));
    let Some(bytes) = bytes else {
        return "Windows FNT".into();
    };
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn winfnt_font_data(data: &[u8], size_pt: f32, header: &WinFntHeader) -> Arc<FontData> {
    #[allow(clippy::arc_with_non_send_sync)]
    let font_data = Arc::new(FontData {
        raw_data: data.to_vec(),
        face_offset: 0,
        face_index: 0,
        num_faces: 1,
        table_directory: tt::TableDirectory {
            records: Vec::new(),
        },
        cmap: tt::cmap::CmapTable::default(),
        fvar: None,
        gvar: None,
        design_variation_coords: Vec::new(),
        normalized_variation_coords: Vec::new(),
        blend_variation_coords_16_16: Vec::new(),
        variation_coordinates_set: false,
        gasp: None,
        head: tt::head::HeadTable {
            units_per_em: header.pixel_height.max(1),
            x_min: 0,
            y_min: 0,
            x_max: i16_from_i32(i32::from(header.max_width)),
            y_max: i16_from_i32(i32::from(header.pixel_height)),
            index_to_loc_format: 0,
            flags: 0,
            mac_style: u16::from(header.italic != 0) << 1,
            lowest_rec_ppem: 0,
        },
        hhea: tt::hhea::HheaTable {
            ascent: i16_from_i32(i32::from(header.ascent)),
            descent: i16_from_i32(i32::from(header.ascent) - i32::from(header.pixel_height)),
            line_gap: i16_from_i32(i32::from(header.external_leading)),
            advance_width_max: header.max_width,
            num_hmetrics: 1,
        },
        hvar: None,
        mvar: None,
        hmtx: tt::hmtx::HmtxTable {
            h_metrics: vec![tt::hmtx::LongHorMetric {
                advance_width: header.avg_width.max(header.max_width),
                lsb: 0,
            }],
            left_side_bearings: Vec::new(),
        },
        maxp: tt::maxp::MaxpTable {
            num_glyphs: u16::from(header.last_char - header.first_char) + 1,
            ..tt::maxp::MaxpTable::default()
        },
        name: tt::name::NameTable {
            format: 0,
            family: winfnt_family_name(data, header),
            subfamily: "Regular".into(),
            postscript_name: None,
            records: Vec::new(),
            lang_tags: Vec::new(),
        },
        os2: None,
        post: None,
        vhea: None,
        vmtx: None,
        hdmx: None,
        kern: None,
        sbit: None,
        cff: None,
        loca_data: Vec::new(),
        glyf_data: Vec::new(),
        size_pt: std::cell::Cell::new(size_pt),
        size_public_x_scale: std::cell::Cell::new(0),
        size_public_y_scale: std::cell::Cell::new(0),
        size_x_scale: std::cell::Cell::new(0),
        size_y_scale: std::cell::Cell::new(0),
        size_tt_scale: std::cell::Cell::new(0),
        size_tt_ppem: std::cell::Cell::new(0),
        size_tt_x_ratio: std::cell::Cell::new(0x1_0000),
        size_tt_y_ratio: std::cell::Cell::new(0x1_0000),
        size_tt_point_size: std::cell::Cell::new(0),
        transform_xx: std::cell::Cell::new(0x1_0000),
        transform_xy: std::cell::Cell::new(0),
        transform_yx: std::cell::Cell::new(0),
        transform_yy: std::cell::Cell::new(0x1_0000),
        transform_dx: std::cell::Cell::new(0),
        transform_dy: std::cell::Cell::new(0),
        fpgm: None,
        prep: None,
        cvt: None,
        glyph_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
        self_arc: std::sync::OnceLock::new(),
    });
    let _ = font_data.self_arc.set(font_data.clone());
    font_data
}

fn parse_bdf_font_bounding_box(line: &str) -> Option<(i16, i16, i16, i16)> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "FONTBOUNDINGBOX" {
        return None;
    }
    let width = parts.next()?.parse().ok()?;
    let height = parts.next()?.parse().ok()?;
    let x_offset = parts.next()?.parse().ok()?;
    let y_offset = parts.next()?.parse().ok()?;
    Some((width, height, x_offset, y_offset))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BdfPropertyFormat {
    Atom,
    Integer,
    Cardinal,
}

fn bdf_property_format(name: &str) -> BdfPropertyFormat {
    match name {
        "DEFAULT_CHAR" | "DESTINATION" | "RELATIVE_SETWIDTH" | "RELATIVE_WEIGHT"
        | "RESOLUTION_X" | "RESOLUTION_Y" | "WEIGHT" => BdfPropertyFormat::Cardinal,
        "AVERAGE_WIDTH"
        | "AVG_CAPITAL_WIDTH"
        | "AVG_LOWERCASE_WIDTH"
        | "CAP_HEIGHT"
        | "END_SPACE"
        | "FIGURE_WIDTH"
        | "FONT_ASCENT"
        | "FONT_DESCENT"
        | "ITALIC_ANGLE"
        | "MAX_SPACE"
        | "MIN_SPACE"
        | "NORM_SPACE"
        | "PIXEL_SIZE"
        | "POINT_SIZE"
        | "QUAD_WIDTH"
        | "RAW_ASCENT"
        | "RAW_AVERAGE_WIDTH"
        | "RAW_AVG_CAPITAL_WIDTH"
        | "RAW_AVG_LOWERCASE_WIDTH"
        | "RAW_CAP_HEIGHT"
        | "RAW_DESCENT"
        | "RAW_END_SPACE"
        | "RAW_FIGURE_WIDTH"
        | "RAW_MAX_SPACE"
        | "RAW_MIN_SPACE"
        | "RAW_NORM_SPACE"
        | "RAW_PIXEL_SIZE"
        | "RAW_POINT_SIZE"
        | "RAW_PIXELSIZE"
        | "RAW_POINTSIZE"
        | "RAW_QUAD_WIDTH"
        | "RAW_SMALL_CAP_SIZE"
        | "RAW_STRIKEOUT_ASCENT"
        | "RAW_STRIKEOUT_DESCENT"
        | "RAW_SUBSCRIPT_SIZE"
        | "RAW_SUBSCRIPT_X"
        | "RAW_SUBSCRIPT_Y"
        | "RAW_SUPERSCRIPT_SIZE"
        | "RAW_SUPERSCRIPT_X"
        | "RAW_SUPERSCRIPT_Y"
        | "RAW_UNDERLINE_POSITION"
        | "RAW_UNDERLINE_THICKNESS"
        | "RAW_X_HEIGHT"
        | "RESOLUTION"
        | "SMALL_CAP_SIZE"
        | "STRIKEOUT_ASCENT"
        | "STRIKEOUT_DESCENT"
        | "SUBSCRIPT_SIZE"
        | "SUBSCRIPT_X"
        | "SUBSCRIPT_Y"
        | "SUPERSCRIPT_SIZE"
        | "SUPERSCRIPT_X"
        | "SUPERSCRIPT_Y"
        | "UNDERLINE_POSITION"
        | "UNDERLINE_THICKNESS"
        | "X_HEIGHT"
        | "_MULE_BASELINE_OFFSET"
        | "_MULE_RELATIVE_COMPOSE" => BdfPropertyFormat::Integer,
        _ => BdfPropertyFormat::Atom,
    }
}

fn parse_bdf_atom(raw_value: &str) -> String {
    raw_value.trim().trim_matches('"').to_string()
}

fn parse_bdf_property_line(line: &str) -> Option<BdfPropertyEntry> {
    let (name, raw_value) = line.split_once(char::is_whitespace)?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let format = bdf_property_format(name);
    let value = match format {
        BdfPropertyFormat::Atom => BdfPropertyValue::Atom(parse_bdf_atom(raw_value)),
        BdfPropertyFormat::Integer => {
            let parsed = raw_value.trim().parse::<i64>().ok()? as i32;
            BdfPropertyValue::Integer(parsed)
        }
        BdfPropertyFormat::Cardinal => {
            let parsed = raw_value.trim().parse::<u64>().ok()? as u32;
            BdfPropertyValue::Cardinal(parsed)
        }
    };
    let atom_c_string = match &value {
        BdfPropertyValue::Atom(atom) => CString::new(atom.as_str()).ok(),
        _ => None,
    };
    Some(BdfPropertyEntry {
        name: name.to_string(),
        value,
        atom_c_string,
    })
}

fn parse_bdf_metadata(text: &str) -> Result<BdfMetadata, FontError> {
    let mut family_name = "BDF".to_string();
    let mut bbox = None;
    let mut glyph_count = 0u16;
    let mut properties = Vec::new();
    let mut in_properties = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with("STARTPROPERTIES ") {
            in_properties = true;
        } else if line == "ENDPROPERTIES" {
            in_properties = false;
        } else if in_properties {
            if let Some(entry) = parse_bdf_property_line(line) {
                if let Some(existing_index) = properties
                    .iter()
                    .position(|existing: &BdfPropertyEntry| existing.name == entry.name)
                {
                    properties[existing_index] = entry;
                } else {
                    properties.push(entry);
                }
            }
        } else if let Some(name) = line.strip_prefix("FONT ") {
            family_name = name.trim().to_string();
        } else if line.starts_with("FONTBOUNDINGBOX ") {
            bbox = parse_bdf_font_bounding_box(line);
        } else if line.starts_with("STARTCHAR ") {
            glyph_count = glyph_count.saturating_add(1);
        }
    }

    let Some((pixel_width, pixel_height, x_offset, y_offset)) = bbox else {
        return Err(FontError::BdfMissingFontboundingboxField);
    };
    Ok(BdfMetadata {
        family_name,
        pixel_width,
        pixel_height,
        x_offset,
        y_offset,
        glyph_count: glyph_count.max(1),
        properties,
    })
}

fn bdf_font_data(data: &[u8], size_pt: f32, metadata: &BdfMetadata) -> Arc<FontData> {
    let pixel_width = metadata.pixel_width.max(1);
    let pixel_height = metadata.pixel_height.max(1);
    let x_min = metadata.x_offset;
    let y_min = metadata.y_offset;
    let x_max = x_min.saturating_add(pixel_width);
    let y_max = y_min.saturating_add(pixel_height);

    #[allow(clippy::arc_with_non_send_sync)]
    let font_data = Arc::new(FontData {
        raw_data: data.to_vec(),
        face_offset: 0,
        face_index: 0,
        num_faces: 1,
        table_directory: tt::TableDirectory {
            records: Vec::new(),
        },
        cmap: tt::cmap::CmapTable::default(),
        fvar: None,
        gvar: None,
        design_variation_coords: Vec::new(),
        normalized_variation_coords: Vec::new(),
        blend_variation_coords_16_16: Vec::new(),
        variation_coordinates_set: false,
        gasp: None,
        head: tt::head::HeadTable {
            // BDF is a bitmap strike, not a scalable outline.  Use a nonzero
            // synthetic UPEM only so shared size-metric helpers avoid division
            // by zero; `FaceKind::Bdf` keeps FT_FACE_FLAG_SCALABLE clear.
            units_per_em: u16::try_from(pixel_height).unwrap_or(1),
            x_min,
            y_min,
            x_max,
            y_max,
            index_to_loc_format: 0,
            flags: 0,
            mac_style: 0,
            lowest_rec_ppem: 0,
        },
        hhea: tt::hhea::HheaTable {
            ascent: y_max,
            descent: y_min,
            line_gap: 0,
            advance_width_max: u16::try_from(pixel_width).unwrap_or(1),
            num_hmetrics: 1,
        },
        hvar: None,
        mvar: None,
        hmtx: tt::hmtx::HmtxTable {
            h_metrics: vec![tt::hmtx::LongHorMetric {
                advance_width: u16::try_from(pixel_width).unwrap_or(1),
                lsb: x_min,
            }],
            left_side_bearings: Vec::new(),
        },
        maxp: tt::maxp::MaxpTable {
            num_glyphs: metadata.glyph_count,
            ..tt::maxp::MaxpTable::default()
        },
        name: tt::name::NameTable {
            format: 0,
            family: metadata.family_name.clone(),
            subfamily: "Regular".into(),
            postscript_name: None,
            records: Vec::new(),
            lang_tags: Vec::new(),
        },
        os2: None,
        post: None,
        vhea: None,
        vmtx: None,
        hdmx: None,
        kern: None,
        sbit: None,
        cff: None,
        loca_data: Vec::new(),
        glyf_data: Vec::new(),
        size_pt: std::cell::Cell::new(size_pt),
        size_public_x_scale: std::cell::Cell::new(0),
        size_public_y_scale: std::cell::Cell::new(0),
        size_x_scale: std::cell::Cell::new(0),
        size_y_scale: std::cell::Cell::new(0),
        size_tt_scale: std::cell::Cell::new(0),
        size_tt_ppem: std::cell::Cell::new(0),
        size_tt_x_ratio: std::cell::Cell::new(0x1_0000),
        size_tt_y_ratio: std::cell::Cell::new(0x1_0000),
        size_tt_point_size: std::cell::Cell::new(0),
        transform_xx: std::cell::Cell::new(0x1_0000),
        transform_xy: std::cell::Cell::new(0),
        transform_yx: std::cell::Cell::new(0),
        transform_yy: std::cell::Cell::new(0x1_0000),
        transform_dx: std::cell::Cell::new(0),
        transform_dy: std::cell::Cell::new(0),
        fpgm: None,
        prep: None,
        cvt: None,
        glyph_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
        self_arc: std::sync::OnceLock::new(),
    });
    let _ = font_data.self_arc.set(font_data.clone());
    font_data
}

fn bdf_text(data: &[u8]) -> Option<&str> {
    std::str::from_utf8(data).ok()
}

fn first_bdf_keyword(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .and_then(|line| line.split_whitespace().next())
}

fn is_bdf_probe_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "STARTFONT"
            | "COMMENT"
            | "FONT"
            | "SIZE"
            | "FONTBOUNDINGBOX"
            | "CHARS"
            | "STARTCHAR"
            | "ENCODING"
            | "SWIDTH"
            | "DWIDTH"
            | "BBX"
            | "BITMAP"
            | "ENDCHAR"
            | "ENDFONT"
    )
}

fn parse_bdf_bbx(line: &str) -> Option<(i64, i64)> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "BBX" {
        return None;
    }
    let width = parts.next()?.parse().ok()?;
    let height = parts.next()?.parse().ok()?;
    Some((width, height))
}

fn bdf_bitmap_too_large(width: i64, height: i64) -> bool {
    if width <= 0 || height <= 0 {
        return false;
    }
    let bytes_per_row = (width + 7) / 8;
    bytes_per_row > 0xFFFF || bytes_per_row.saturating_mul(height) > 0xFFFF
}

// FreeType's BDF driver classifies these malformed inputs during
// FT_New_Memory_Face before a usable face exists.  This mirrors the
// constructor-time state checks in bdf/bdflib.c for public error parity only;
// successful BDF face loading/rendering remains intentionally unsupported.
fn parse_bdf_constructor_error(data: &[u8]) -> Option<FontError> {
    let text = bdf_text(data)?;
    let first_keyword = first_bdf_keyword(text)?;
    if first_keyword != "STARTFONT" {
        // FreeType first rejects this in bdflib.c:bdf_parse_start_ as
        // Missing_Startfont_Field, then the public FT_New_Memory_Face path
        // surfaces error 85 for the maintained BDF-like fixture.  Keep this
        // detection narrow so arbitrary UTF-8 non-BDF data can still fall
        // through to the Type1/SFNT probes.
        return is_bdf_probe_keyword(first_keyword)
            .then_some(FontError::BdfMissingStartfontStreamOperation);
    }
    let mut has_font = false;
    let mut has_size = false;
    let mut has_font_bounding_box = false;
    let mut saw_chars = false;
    let mut in_glyph = false;
    let mut glyph_has_encoding = false;
    let mut glyph_has_bbx = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let keyword = line.split_whitespace().next().unwrap_or("");
        match keyword {
            "FONT" => has_font = true,
            "SIZE" => has_size = true,
            "FONTBOUNDINGBOX" => has_font_bounding_box = true,
            "CHARS" => {
                saw_chars = true;
                if !has_font {
                    return Some(FontError::BdfMissingFontField);
                }
                if !has_size {
                    return Some(FontError::BdfMissingSizeField);
                }
                if !has_font_bounding_box {
                    return Some(FontError::BdfMissingFontboundingboxField);
                }
            }
            "STARTCHAR" => {
                if !saw_chars || in_glyph {
                    return Some(FontError::BdfMissingStartcharField);
                }
                in_glyph = true;
                glyph_has_encoding = false;
                glyph_has_bbx = false;
            }
            "ENCODING" => {
                if !in_glyph {
                    return Some(FontError::BdfMissingStartcharField);
                }
                glyph_has_encoding = true;
            }
            "BBX" => {
                if !in_glyph {
                    return Some(FontError::BdfMissingStartcharField);
                }
                let Some((width, height)) = parse_bdf_bbx(line) else {
                    return Some(FontError::BdfCorruptedFontGlyphs);
                };
                if bdf_bitmap_too_large(width, height) {
                    return Some(FontError::BdfBbxTooBig);
                }
                glyph_has_bbx = true;
            }
            "BITMAP" => {
                if !in_glyph {
                    return Some(FontError::BdfMissingStartcharField);
                }
                if !glyph_has_encoding {
                    return Some(FontError::BdfMissingEncodingField);
                }
                if !glyph_has_bbx {
                    return Some(FontError::BdfMissingBbxField);
                }
            }
            "ENDCHAR" => {
                if !in_glyph {
                    return Some(FontError::BdfMissingStartcharField);
                }
                if !glyph_has_encoding {
                    return Some(FontError::BdfMissingEncodingField);
                }
                if !glyph_has_bbx {
                    return Some(FontError::BdfMissingBbxField);
                }
                in_glyph = false;
            }
            "ENDFONT" if in_glyph => {
                return Some(FontError::BdfCorruptedFontGlyphs);
            }
            _ => {}
        }
    }

    // FreeType bdflib.c reports header corruption when a BDF stream reaches
    // EOF before CHARS after a valid STARTFONT line.
    if !saw_chars {
        return Some(FontError::BdfCorruptedFontHeader);
    }
    if in_glyph {
        return Some(FontError::BdfCorruptedFontGlyphs);
    }
    None
}

fn type1_cleartext(data: &[u8]) -> Option<&[u8]> {
    if data.len() >= 6 && data[0] == 0x80 && data[1] == 0x01 {
        let len = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;
        let clear = data.get(6..6 + len)?;
        return clear.starts_with(b"%!").then_some(clear);
    }
    data.starts_with(b"%!").then_some(data)
}

fn type1_eexec_private_text(data: &[u8]) -> Option<String> {
    Some(String::from_utf8_lossy(&type1_eexec_private_bytes(data)?).into_owned())
}

fn type1_eexec_private_bytes(data: &[u8]) -> Option<Vec<u8>> {
    // PFB segment type 2 is the eexec-encrypted private program.  FreeType's
    // Type 1 loader decrypts this program before filling `PS_PrivateRec`.
    let mut offset = 0usize;
    while offset.checked_add(6)? <= data.len() && data[offset] == 0x80 {
        let segment_type = data[offset + 1];
        let len = u32::from_le_bytes([
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
        ]) as usize;
        let start = offset.checked_add(6)?;
        let end = start.checked_add(len)?;
        let segment = data.get(start..end)?;
        if segment_type == 2 {
            return type1_decrypt_eexec(segment);
        }
        offset = end;
    }
    None
}

fn type1_decrypt_eexec(cipher: &[u8]) -> Option<Vec<u8>> {
    const C1: u32 = 52845;
    const C2: u32 = 22719;
    let mut r = 55665u32;
    let mut plain = Vec::with_capacity(cipher.len());
    for &cipher_byte in cipher {
        let plain_byte = cipher_byte ^ ((r >> 8) as u8);
        r = ((u32::from(cipher_byte) + r) * C1 + C2) & 0xffff;
        plain.push(plain_byte);
    }
    Some(plain.get(4..)?.to_vec())
}

fn parse_type1_metadata(cleartext: &[u8]) -> Result<Type1Metadata, FontError> {
    let text = std::str::from_utf8(cleartext)
        .map_err(|_| FontError::InvalidFont("Type 1 clear-text dictionary is not UTF-8".into()))?;
    let font_name = type1_name_token(text, "FontName")
        .ok_or_else(|| FontError::InvalidFont("Type 1 missing FontName".into()))?;
    let version = type1_string_value(text, "version");
    let notice = type1_string_value(text, "Notice");
    let full_name = type1_string_value(text, "FullName");
    let family_name = type1_string_value(text, "FamilyName").unwrap_or_else(|| font_name.clone());
    let style_name = type1_string_value(text, "Weight").unwrap_or_else(|| "Regular".into());
    let bbox =
        type1_bbox(text).ok_or_else(|| FontError::InvalidFont("Type 1 missing FontBBox".into()))?;
    Ok(Type1Metadata {
        version,
        notice,
        full_name,
        font_name,
        family_name,
        style_name,
        italic_angle: type1_i16_value(text, "ItalicAngle").unwrap_or(0),
        is_fixed_pitch: type1_bool_value(text, "isFixedPitch").unwrap_or(false),
        underline_position: type1_i16_value(text, "UnderlinePosition").unwrap_or(0),
        underline_thickness: type1_i16_value(text, "UnderlineThickness").unwrap_or(0),
        bbox,
    })
}

fn parse_type1_private(data: &[u8]) -> Option<Type1PrivateDict> {
    let text = type1_eexec_private_text(data)?;
    let mut private = Type1PrivateDict {
        len_iv: type1_i32_value(&text, "lenIV").unwrap_or(4),
        // FreeType Type 1 `t1tokens.h` declares BlueScale as
        // `T1_FIELD_FIXED_1000`; `t1load.c` initializes the default as
        // `0.039625 * 0x10000 * 1000`, so public `PS_PrivateRec` stores
        // 1000x the normal 16.16 fixed value.
        blue_scale: type1_fixed_1000_value(&text, "BlueScale").unwrap_or(2_596_864),
        blue_shift: type1_i32_value(&text, "BlueShift").unwrap_or(7),
        blue_fuzz: type1_i32_value(&text, "BlueFuzz").unwrap_or(1),
        force_bold: type1_bool_value(&text, "ForceBold").unwrap_or(false),
        expansion_factor: type1_fixed_value(&text, "ExpansionFactor").unwrap_or(3_932),
        language_group: i64::from(type1_i32_value(&text, "LanguageGroup").unwrap_or(0)),
        password: i64::from(type1_i32_value(&text, "password").unwrap_or(5839)),
        ..Type1PrivateDict::default()
    };
    if let Some(unique_id) = type1_i32_value(&text, "UniqueID") {
        private.unique_id = unique_id;
    }
    copy_i16_array(
        type1_number_array(&text, "BlueValues").as_deref(),
        &mut private.blue_values,
        &mut private.num_blue_values,
    );
    copy_i16_array(
        type1_number_array(&text, "OtherBlues").as_deref(),
        &mut private.other_blues,
        &mut private.num_other_blues,
    );
    copy_i16_array(
        type1_number_array(&text, "FamilyBlues").as_deref(),
        &mut private.family_blues,
        &mut private.num_family_blues,
    );
    copy_i16_array(
        type1_number_array(&text, "FamilyOtherBlues").as_deref(),
        &mut private.family_other_blues,
        &mut private.num_family_other_blues,
    );
    if let Some(std_hw) = first_u16(&text, "StdHW") {
        private.standard_width[0] = std_hw;
    }
    if let Some(std_vw) = first_u16(&text, "StdVW") {
        private.standard_height[0] = std_vw;
    }
    copy_i16_array(
        type1_number_array(&text, "StemSnapV").as_deref(),
        &mut private.snap_heights,
        &mut private.num_snap_heights,
    );
    copy_i16_array(
        type1_number_array(&text, "StemSnapH").as_deref(),
        &mut private.snap_widths,
        &mut private.num_snap_widths,
    );
    Some(private)
}

fn parse_type1_encoding(cleartext: &[u8]) -> Option<Type1EncodingInfo> {
    let text = std::str::from_utf8(cleartext).ok()?;
    let tail = type1_exact_key_tail(text, "Encoding")?;
    if tail.starts_with("StandardEncoding") {
        return Some(Type1EncodingInfo {
            encoding_type: 2,
            entries: Vec::new(),
        });
    }
    if tail.starts_with("ISOLatin1Encoding") {
        return Some(Type1EncodingInfo {
            encoding_type: 3,
            entries: Vec::new(),
        });
    }
    if tail.starts_with("ExpertEncoding") {
        return Some(Type1EncodingInfo {
            encoding_type: 4,
            entries: Vec::new(),
        });
    }
    if !tail.starts_with("256 array") {
        return Some(Type1EncodingInfo {
            encoding_type: 0,
            entries: Vec::new(),
        });
    }

    let mut entries = vec![Some(".notdef".to_string()); 256];
    let tokens = tail.split_whitespace().collect::<Vec<_>>();
    for window in tokens.windows(4) {
        if window[0] != "dup" || window[3] != "put" {
            continue;
        }
        let Some(index) = window[1]
            .parse::<usize>()
            .ok()
            .filter(|index| *index < entries.len())
        else {
            continue;
        };
        if let Some(name) = window[2].strip_prefix('/') {
            entries[index] = Some(name.to_string());
        }
    }
    Some(Type1EncodingInfo {
        encoding_type: 1,
        entries,
    })
}

fn parse_type1_charstrings(data: &[u8], len_iv: i32) -> Vec<Type1CharString> {
    let Some(private) = type1_eexec_private_bytes(data) else {
        return Vec::new();
    };
    let Some(mut offset) = find_bytes(&private, b"/CharStrings") else {
        return Vec::new();
    };
    let Some(begin) = find_bytes(&private[offset..], b"begin") else {
        return Vec::new();
    };
    offset += begin + b"begin".len();
    let end = find_bytes(&private[offset..], b"\nend").map_or(private.len(), |end| offset + end);
    let mut charstrings = Vec::new();
    while offset < end {
        skip_ascii_space(&private, &mut offset);
        if offset >= end {
            break;
        }
        if private[offset] != b'/' {
            offset += 1;
            continue;
        }
        offset += 1;
        let name_start = offset;
        while offset < end && !private[offset].is_ascii_whitespace() {
            offset += 1;
        }
        let name = String::from_utf8_lossy(&private[name_start..offset]).into_owned();
        skip_ascii_space(&private, &mut offset);
        let length_start = offset;
        while offset < end && private[offset].is_ascii_digit() {
            offset += 1;
        }
        let Some(length) = std::str::from_utf8(&private[length_start..offset])
            .ok()
            .and_then(|length| length.parse::<usize>().ok())
        else {
            continue;
        };
        skip_ascii_space(&private, &mut offset);
        while offset < end && !private[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset < end && private[offset].is_ascii_whitespace() {
            offset += 1;
        }
        let Some(encrypted) = private.get(offset..offset.saturating_add(length)) else {
            break;
        };
        let decrypted = decrypt_type1_charstring(encrypted, len_iv);
        charstrings.push(Type1CharString {
            name,
            encrypted: decrypted,
        });
        offset = offset.saturating_add(length);
    }
    charstrings
}

fn decrypt_type1_charstring(cipher: &[u8], len_iv: i32) -> Vec<u8> {
    if len_iv < 0 {
        return cipher.to_vec();
    }
    const C1: u32 = 52845;
    const C2: u32 = 22719;
    let mut r = 4330u32;
    let mut plain = Vec::with_capacity(cipher.len());
    for &cipher_byte in cipher {
        let plain_byte = cipher_byte ^ ((r >> 8) as u8);
        r = ((u32::from(cipher_byte) + r) * C1 + C2) & 0xffff;
        plain.push(plain_byte);
    }
    let skip = usize::try_from(len_iv).unwrap_or(0).min(plain.len());
    plain[skip..].to_vec()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn skip_ascii_space(bytes: &[u8], offset: &mut usize) {
    while *offset < bytes.len() && bytes[*offset].is_ascii_whitespace() {
        *offset += 1;
    }
}

fn parse_type1_glyph_program(charstring: &[u8]) -> Result<Type1GlyphProgram, FontError> {
    let mut stack = Vec::<i32>::new();
    let mut outline = Type1GlyphOutline::default();
    let mut x = 0i32;
    let mut y = 0i32;
    let mut advance_width = 0i32;
    let mut open_contour = false;
    let mut offset = 0usize;
    while offset < charstring.len() {
        let op = charstring[offset];
        offset += 1;
        match op {
            1 | 3 | 10 | 11 | 15 | 19 | 20 => stack.clear(),
            4 => {
                let dy = type1_pop_one(&mut stack)?;
                type1_finish_contour(&mut outline, &mut open_contour)?;
                y = y.saturating_add(dy);
                type1_push_point(&mut outline, x, y);
                open_contour = true;
                stack.clear();
            }
            5 => {
                for pair in stack.chunks_exact(2) {
                    x = x.saturating_add(pair[0]);
                    y = y.saturating_add(pair[1]);
                    type1_push_point(&mut outline, x, y);
                }
                stack.clear();
            }
            6 => {
                for dx in stack.drain(..) {
                    x = x.saturating_add(dx);
                    type1_push_point(&mut outline, x, y);
                }
            }
            7 => {
                for dy in stack.drain(..) {
                    y = y.saturating_add(dy);
                    type1_push_point(&mut outline, x, y);
                }
            }
            8 => {
                for curve in stack.chunks_exact(6) {
                    x = x.saturating_add(curve[0]);
                    y = y.saturating_add(curve[1]);
                    outline.points.push(OutlinePoint {
                        x,
                        y,
                        on_curve: false,
                    });
                    x = x.saturating_add(curve[2]);
                    y = y.saturating_add(curve[3]);
                    outline.points.push(OutlinePoint {
                        x,
                        y,
                        on_curve: false,
                    });
                    x = x.saturating_add(curve[4]);
                    y = y.saturating_add(curve[5]);
                    type1_push_point(&mut outline, x, y);
                }
                stack.clear();
            }
            9 => {
                type1_finish_contour(&mut outline, &mut open_contour)?;
                stack.clear();
            }
            12 => {
                let Some(escape) = charstring.get(offset).copied() else {
                    return Err(FontError::InvalidOutline(
                        "Type 1 truncated escape operator".into(),
                    ));
                };
                offset += 1;
                match escape {
                    // Flex/hint and counter operators do not change the public
                    // outline for the compact fixtures currently parsed here.
                    0 | 1 | 2 | 6 | 7 | 12 | 16 | 17 => stack.clear(),
                    _ => {
                        return Err(FontError::InvalidOutline(format!(
                            "unsupported Type 1 escaped charstring operator 12 {escape}"
                        )));
                    }
                }
            }
            13 => {
                if stack.len() < 2 {
                    return Err(FontError::InvalidOutline(
                        "Type 1 hsbw stack underflow".into(),
                    ));
                }
                x = stack[0];
                y = 0;
                advance_width = stack[1];
                stack.clear();
            }
            14 => {
                type1_finish_contour(&mut outline, &mut open_contour)?;
                return Ok(Type1GlyphProgram {
                    advance_width,
                    outline,
                });
            }
            21 => {
                if stack.len() < 2 {
                    return Err(FontError::InvalidOutline(
                        "Type 1 rmoveto stack underflow".into(),
                    ));
                }
                let dx = stack[stack.len() - 2];
                let dy = stack[stack.len() - 1];
                type1_finish_contour(&mut outline, &mut open_contour)?;
                x = x.saturating_add(dx);
                y = y.saturating_add(dy);
                type1_push_point(&mut outline, x, y);
                open_contour = true;
                stack.clear();
            }
            22 => {
                let dx = type1_pop_one(&mut stack)?;
                type1_finish_contour(&mut outline, &mut open_contour)?;
                x = x.saturating_add(dx);
                type1_push_point(&mut outline, x, y);
                open_contour = true;
                stack.clear();
            }
            30 | 31 => {
                return Err(FontError::InvalidOutline(
                    "Type 1 vhcurveto/hvcurveto unsupported".into(),
                ));
            }
            32..=255 => {
                offset -= 1;
                let value = decode_type1_number(charstring, &mut offset)?;
                stack.push(value);
            }
            _ => {
                return Err(FontError::InvalidOutline(format!(
                    "unsupported Type 1 charstring operator {op}"
                )));
            }
        }
    }
    type1_finish_contour(&mut outline, &mut open_contour)?;
    Ok(Type1GlyphProgram {
        advance_width,
        outline,
    })
}

fn type1_pop_one(stack: &mut Vec<i32>) -> Result<i32, FontError> {
    stack
        .pop()
        .ok_or_else(|| FontError::InvalidOutline("Type 1 charstring stack underflow".into()))
}

fn type1_push_point(outline: &mut Type1GlyphOutline, x: i32, y: i32) {
    outline.points.push(OutlinePoint {
        x,
        y,
        on_curve: true,
    });
}

fn type1_finish_contour(
    outline: &mut Type1GlyphOutline,
    open_contour: &mut bool,
) -> Result<(), FontError> {
    if !*open_contour {
        return Ok(());
    }
    let Some(last) = outline.points.len().checked_sub(1) else {
        *open_contour = false;
        return Ok(());
    };
    outline.contours.push(
        i16::try_from(last).map_err(|_| {
            FontError::InvalidOutline("Type 1 contour endpoint out of range".into())
        })?,
    );
    *open_contour = false;
    Ok(())
}

fn decode_type1_number(bytes: &[u8], offset: &mut usize) -> Result<i32, FontError> {
    let Some(first) = bytes.get(*offset).copied() else {
        return Err(FontError::InvalidOutline("Type 1 number missing".into()));
    };
    *offset += 1;
    match first {
        32..=246 => Ok(i32::from(first) - 139),
        247..=250 => {
            let next = type1_next_byte(bytes, offset)?;
            Ok((i32::from(first) - 247) * 256 + i32::from(next) + 108)
        }
        251..=254 => {
            let next = type1_next_byte(bytes, offset)?;
            Ok(-((i32::from(first) - 251) * 256) - i32::from(next) - 108)
        }
        255 => {
            let bytes = [
                type1_next_byte(bytes, offset)?,
                type1_next_byte(bytes, offset)?,
                type1_next_byte(bytes, offset)?,
                type1_next_byte(bytes, offset)?,
            ];
            Ok(i32::from_be_bytes(bytes))
        }
        _ => Err(FontError::InvalidOutline(
            "Type 1 operator cannot be decoded as number".into(),
        )),
    }
}

fn type1_next_byte(bytes: &[u8], offset: &mut usize) -> Result<u8, FontError> {
    let Some(byte) = bytes.get(*offset).copied() else {
        return Err(FontError::InvalidOutline("Type 1 number truncated".into()));
    };
    *offset += 1;
    Ok(byte)
}

fn type1_scale_font_unit(value: i32, scale: i32) -> i32 {
    // FreeType's Type 1 loader scales decrypted CharString coordinates through
    // the PS hinter/decoder path before slot metric grid fitting.  For the
    // maintained Type 1 MM fixture this behaves as a truncating 16.16 multiply;
    // using the rounded TrueType `FT_MulFix` path shifts right/top fractional
    // edges by one 26.6 unit and changes smooth-raster coverage.
    ((i64::from(value) * i64::from(scale)) >> 16) as i32
}

fn type1_exact_key_tail<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("/{key}");
    let mut search_start = 0usize;
    while let Some(relative_start) = text[search_start..].find(&marker) {
        let start = search_start + relative_start;
        let after = start + marker.len();
        if text[after..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            return Some(text[after..].trim_start());
        }
        search_start = after;
    }
    None
}

fn copy_i16_array(values: Option<&[f64]>, out: &mut [i16], count: &mut u8) {
    let Some(values) = values else {
        return;
    };
    let mut written = 0usize;
    for (dst, value) in out.iter_mut().zip(values.iter().copied()) {
        if let Some(parsed) = i16_from_f64(value) {
            *dst = parsed;
            written += 1;
        }
    }
    *count = u8::try_from(written).unwrap_or(u8::MAX);
}

fn first_u16(text: &str, key: &str) -> Option<u16> {
    let value = type1_number_array(text, key)?.first().copied()?;
    u16_from_f64(value)
}

fn parse_type1_multi_master(cleartext: &[u8]) -> Option<Type1MultiMaster> {
    let text = std::str::from_utf8(cleartext).ok()?;
    let axes = type1_name_array(text, "BlendAxisTypes")?;
    if axes.is_empty() || axes.len() > 4 {
        return None;
    }
    let design_positions = type1_nested_number_array(text, "BlendDesignPositions")?;
    let design_maps = type1_nested_number_array(text, "BlendDesignMap")?;
    let weight_vector = type1_number_array(text, "WeightVector")?
        .into_iter()
        .map(type1_weight_to_fixed)
        .collect::<Option<Vec<_>>>()?;
    let num_designs = 1usize.checked_shl(u32::try_from(axes.len()).ok()?)?;
    if design_positions.len() != num_designs
        || weight_vector.len() != num_designs
        || design_maps.len() != axes.len()
        || design_positions
            .iter()
            .any(|position| position.len() != axes.len())
    {
        return None;
    }
    let axes = axes
        .into_iter()
        .zip(design_maps)
        .map(|(name, map)| {
            if map.len() < 2 || map.len() % 2 != 0 {
                return None;
            }
            Some(Type1MultiMasterAxis {
                name,
                minimum: i32_from_f64(map[0])?,
                maximum: i32_from_f64(map[map.len() - 2])?,
                design_map: map
                    .chunks_exact(2)
                    .map(|pair| {
                        Some(Type1DesignMapPoint {
                            design: i32_from_f64(pair[0])?,
                            blend: type1_weight_to_fixed(pair[1])?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Type1MultiMaster {
        axes,
        num_designs,
        default_weight_vector: weight_vector,
    })
}

fn type1_font_data(data: &[u8], size_pt: f32, metadata: &Type1Metadata) -> Arc<FontData> {
    let mac_style = u16::from(metadata.italic_angle != 0) << 1
        | if matches!(metadata.style_name.as_str(), "Bold" | "Black") {
            1u16
        } else {
            0u16
        };
    #[allow(clippy::arc_with_non_send_sync)]
    let font_data = Arc::new(FontData {
        raw_data: data.to_vec(),
        face_offset: 0,
        face_index: 0,
        num_faces: 1,
        table_directory: tt::TableDirectory {
            records: Vec::new(),
        },
        cmap: tt::cmap::CmapTable::default(),
        fvar: None,
        gvar: None,
        design_variation_coords: Vec::new(),
        normalized_variation_coords: Vec::new(),
        blend_variation_coords_16_16: Vec::new(),
        variation_coordinates_set: false,
        gasp: None,
        head: tt::head::HeadTable {
            units_per_em: 1000,
            x_min: i16_from_i32(metadata.bbox.x_min),
            y_min: i16_from_i32(metadata.bbox.y_min),
            x_max: i16_from_i32(metadata.bbox.x_max),
            y_max: i16_from_i32(metadata.bbox.y_max),
            index_to_loc_format: 0,
            flags: 0,
            mac_style,
            lowest_rec_ppem: 0,
        },
        hhea: tt::hhea::HheaTable {
            ascent: i16_from_i32(metadata.bbox.y_max),
            descent: i16_from_i32(metadata.bbox.y_min),
            line_gap: i16_from_i32(
                1200i32.saturating_sub(metadata.bbox.y_max - metadata.bbox.y_min),
            ),
            advance_width_max: u16::try_from(metadata.bbox.x_max.max(0)).unwrap_or(u16::MAX),
            num_hmetrics: 1,
        },
        hvar: None,
        mvar: None,
        hmtx: tt::hmtx::HmtxTable {
            h_metrics: vec![tt::hmtx::LongHorMetric {
                advance_width: u16::try_from(metadata.bbox.x_max.max(0)).unwrap_or(u16::MAX),
                lsb: 0,
            }],
            left_side_bearings: Vec::new(),
        },
        maxp: tt::maxp::MaxpTable {
            num_glyphs: 2,
            ..tt::maxp::MaxpTable::default()
        },
        name: tt::name::NameTable {
            format: 0,
            family: metadata.family_name.clone(),
            subfamily: metadata.style_name.clone(),
            postscript_name: Some(metadata.font_name.clone()),
            records: Vec::new(),
            lang_tags: Vec::new(),
        },
        os2: None,
        post: None,
        vhea: None,
        vmtx: None,
        hdmx: None,
        kern: None,
        sbit: None,
        cff: None,
        loca_data: Vec::new(),
        glyf_data: Vec::new(),
        size_pt: std::cell::Cell::new(size_pt),
        size_public_x_scale: std::cell::Cell::new(0),
        size_public_y_scale: std::cell::Cell::new(0),
        size_x_scale: std::cell::Cell::new(0),
        size_y_scale: std::cell::Cell::new(0),
        size_tt_scale: std::cell::Cell::new(0),
        size_tt_ppem: std::cell::Cell::new(0),
        size_tt_x_ratio: std::cell::Cell::new(0x1_0000),
        size_tt_y_ratio: std::cell::Cell::new(0x1_0000),
        size_tt_point_size: std::cell::Cell::new(0),
        transform_xx: std::cell::Cell::new(0x1_0000),
        transform_xy: std::cell::Cell::new(0),
        transform_yx: std::cell::Cell::new(0),
        transform_yy: std::cell::Cell::new(0x1_0000),
        transform_dx: std::cell::Cell::new(0),
        transform_dy: std::cell::Cell::new(0),
        fpgm: None,
        prep: None,
        cvt: None,
        glyph_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
        self_arc: std::sync::OnceLock::new(),
    });
    let _ = font_data.self_arc.set(font_data.clone());
    font_data
}

fn type1_value_tail<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("/{key}");
    let start = text.find(&marker)? + marker.len();
    Some(text[start..].trim_start())
}

fn type1_bracket_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let tail = type1_value_tail(text, key)?;
    let start = tail.find('[')?;
    let mut depth = 0usize;
    for (index, ch) in tail[start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&tail[start..start + index + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

fn type1_name_array(text: &str, key: &str) -> Option<Vec<String>> {
    let value = type1_bracket_value(text, key)?;
    value
        .trim_matches(['[', ']'])
        .split_whitespace()
        .map(|item| item.strip_prefix('/').map(str::to_owned))
        .collect::<Option<Vec<_>>>()
}

fn type1_number_array(text: &str, key: &str) -> Option<Vec<f64>> {
    let value = type1_bracket_value(text, key)?;
    parse_type1_numbers(value)
}

fn type1_nested_number_array(text: &str, key: &str) -> Option<Vec<Vec<f64>>> {
    let value = type1_bracket_value(text, key)?;
    let inner = value.strip_prefix('[')?.strip_suffix(']')?;
    let mut rows = Vec::new();
    let mut depth = 0usize;
    let mut row_start = None;
    for (index, ch) in inner.char_indices() {
        match ch {
            '[' => {
                if depth == 0 {
                    row_start = Some(index);
                }
                depth += 1;
            }
            ']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let start = row_start.take()?;
                    rows.push(parse_type1_numbers(&inner[start..=index])?);
                }
            }
            _ => {}
        }
    }
    (depth == 0 && !rows.is_empty()).then_some(rows)
}

fn parse_type1_numbers(value: &str) -> Option<Vec<f64>> {
    let normalized = value.replace(['[', ']'], " ");
    normalized
        .split_whitespace()
        .map(|item| item.parse::<f64>().ok())
        .collect()
}

fn i32_from_f64(value: f64) -> Option<i32> {
    if !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    i32::try_from(value as i64).ok()
}

fn i16_from_f64(value: f64) -> Option<i16> {
    if !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    i16::try_from(value as i64).ok()
}

fn u16_from_f64(value: f64) -> Option<u16> {
    if !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    u16::try_from(value as i64).ok()
}

fn type1_real_to_fixed(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    i32::try_from((value * 65_536.0).round() as i64).ok()
}

fn type1_weight_to_fixed(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let fixed = (value * 65_536.0).round();
    if (fixed - value * 65_536.0).abs() > f64::EPSILON {
        return None;
    }
    i32::try_from(fixed as i64).ok()
}

fn type1_fixed_value(text: &str, key: &str) -> Option<i32> {
    type1_real_to_fixed(type1_number_token(text, key)?.parse::<f64>().ok()?)
}

fn type1_fixed_1000_value(text: &str, key: &str) -> Option<i32> {
    type1_real_to_fixed(type1_number_token(text, key)?.parse::<f64>().ok()? * 1000.0)
}

fn type1_mm_design_to_blend(map: &[Type1DesignMapPoint], design: i32) -> i32 {
    let Some(first) = map.first() else {
        return 0;
    };
    let mut before: Option<&Type1DesignMapPoint> = None;
    for point in map {
        if design == point.design {
            return point.blend;
        }
        if design < point.design {
            return before.map_or(point.blend, |prev| {
                crate::fixed::ft_mul_div(
                    design - prev.design,
                    point.blend - prev.blend,
                    point.design - prev.design,
                )
            });
        }
        before = Some(point);
    }
    before.map_or(first.blend, |point| point.blend)
}

fn type1_mm_axis_unmap(map: &[Type1DesignMapPoint], ncv: i32) -> i32 {
    let Some(first) = map.first() else {
        return 0;
    };
    if ncv <= first.blend {
        return first.design.saturating_mul(65_536);
    }
    for pair in map.windows(2) {
        let prev = &pair[0];
        let point = &pair[1];
        if ncv <= point.blend {
            let delta = crate::fixed::ft_mul_div(
                ncv - prev.blend,
                point.design - prev.design,
                point.blend - prev.blend,
            );
            return (prev.design + delta).saturating_mul(65_536);
        }
    }
    map.last()
        .map_or(first.design, |point| point.design)
        .saturating_mul(65_536)
}

fn type1_mm_weights_from_blends(blends: &[i32], active_axis_count: usize) -> Vec<i32> {
    let num_designs = 1usize.checked_shl(blends.len() as u32).unwrap_or(0);
    (0..num_designs)
        .map(|design_index| {
            let mut result = 65_536;
            for (axis_index, mut factor) in blends.iter().copied().enumerate() {
                if axis_index >= active_axis_count {
                    result >>= 1;
                    continue;
                }
                if (design_index & (1usize << axis_index)) == 0 {
                    factor = 65_536 - factor;
                }
                if factor <= 0 {
                    return 0;
                }
                if factor < 65_536 {
                    result = crate::fixed::ft_mul_fix(result, factor);
                }
            }
            result
        })
        .collect()
}

fn type1_mm_weights_unmap(weights: &[i32], axis_count: usize) -> Vec<i32> {
    let mut out = vec![0; axis_count];
    match axis_count {
        0 => {}
        1 => out[0] = weights.get(1).copied().unwrap_or(0),
        2 => {
            out[0] = weights.get(3).copied().unwrap_or(0) + weights.get(1).copied().unwrap_or(0);
            out[1] = weights.get(3).copied().unwrap_or(0) + weights.get(2).copied().unwrap_or(0);
        }
        3 => {
            out[0] = [7, 5, 3, 1]
                .into_iter()
                .map(|index| weights.get(index).copied().unwrap_or(0))
                .sum();
            out[1] = [7, 6, 3, 2]
                .into_iter()
                .map(|index| weights.get(index).copied().unwrap_or(0))
                .sum();
            out[2] = [7, 6, 5, 4]
                .into_iter()
                .map(|index| weights.get(index).copied().unwrap_or(0))
                .sum();
        }
        _ => {
            out[0] = [15, 13, 11, 9, 7, 5, 3, 1]
                .into_iter()
                .map(|index| weights.get(index).copied().unwrap_or(0))
                .sum();
            out[1] = [15, 14, 11, 10, 7, 6, 3, 2]
                .into_iter()
                .map(|index| weights.get(index).copied().unwrap_or(0))
                .sum();
            out[2] = [15, 14, 13, 12, 7, 6, 5, 4]
                .into_iter()
                .map(|index| weights.get(index).copied().unwrap_or(0))
                .sum();
            out[3] = [15, 14, 13, 12, 11, 10, 9, 8]
                .into_iter()
                .map(|index| weights.get(index).copied().unwrap_or(0))
                .sum();
        }
    }
    out
}

fn type1_name_token(text: &str, key: &str) -> Option<String> {
    let tail = type1_value_tail(text, key)?;
    let tail = tail.strip_prefix('/')?;
    let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
    Some(tail[..end].to_string())
}

fn type1_string_value(text: &str, key: &str) -> Option<String> {
    let tail = type1_value_tail(text, key)?;
    let tail = tail.strip_prefix('(')?;
    let end = tail.find(')')?;
    Some(tail[..end].to_string())
}

fn type1_i16_value(text: &str, key: &str) -> Option<i16> {
    type1_number_token(text, key)?.parse().ok()
}

fn type1_i32_value(text: &str, key: &str) -> Option<i32> {
    type1_number_token(text, key)?.parse().ok()
}

fn type1_bool_value(text: &str, key: &str) -> Option<bool> {
    let token = type1_number_token(text, key)?;
    match token {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn type1_number_token<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let tail = type1_value_tail(text, key)?;
    let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
    Some(&tail[..end])
}

fn type1_bbox(text: &str) -> Option<BBox> {
    let tail = type1_value_tail(text, "FontBBox")?;
    let close = if tail.starts_with('{') { '}' } else { ']' };
    let tail = tail.strip_prefix('{').or_else(|| tail.strip_prefix('['))?;
    let end = tail.find(close)?;
    let mut values = tail[..end]
        .split_whitespace()
        .filter_map(|item| item.parse::<i32>().ok());
    Some(BBox {
        x_min: values.next()?,
        y_min: values.next()?,
        x_max: values.next()?,
        y_max: values.next()?,
    })
}

#[derive(Clone, Default)]
struct BytecodeContextCache {
    normal: Arc<OnceLock<tt::hinter::exec::ExecContext>>,
    mono: Arc<OnceLock<tt::hinter::exec::ExecContext>>,
    lcd: Arc<OnceLock<tt::hinter::exec::ExecContext>>,
    lcd_v: Arc<OnceLock<tt::hinter::exec::ExecContext>>,
}

impl BytecodeContextCache {
    fn slot(&self, mode: NativeHintMode) -> &OnceLock<tt::hinter::exec::ExecContext> {
        match mode {
            NativeHintMode::Normal => &self.normal,
            NativeHintMode::Mono => &self.mono,
            NativeHintMode::Lcd => &self.lcd,
            NativeHintMode::LcdV => &self.lcd_v,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ActiveSizeState {
    size_pt: f32,
    size_metrics: SizeMetrics,
    face_globals: crate::autohint::globals::FaceGlobals,
    bytecode_context: BytecodeContextCache,
}

/// FreeType-style bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BBox {
    /// Minimum x coordinate.
    pub x_min: i32,
    /// Minimum y coordinate.
    pub y_min: i32,
    /// Maximum x coordinate.
    pub x_max: i32,
    /// Maximum y coordinate.
    pub y_max: i32,
}

/// FreeType-like size metrics for the active size object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeMetrics {
    /// Horizontal pixels per EM.
    pub x_ppem: u16,
    /// Vertical pixels per EM.
    pub y_ppem: u16,
    /// Horizontal font-unit to 26.6 scale in 16.16.
    pub x_scale: i32,
    /// Vertical font-unit to 26.6 scale in 16.16.
    pub y_scale: i32,
    /// Scaled ascender in 26.6 pixels.
    pub ascender: i32,
    /// Scaled descender in 26.6 pixels.
    pub descender: i32,
    /// Scaled line height in 26.6 pixels.
    pub height: i32,
    /// Scaled maximum horizontal advance in 26.6 pixels.
    pub max_advance: i32,
    /// Requested horizontal DPI.
    pub x_dpi: u32,
    /// Requested vertical DPI.
    pub y_dpi: u32,
    /// Requested character width in 26.6 points.
    pub char_width: i32,
    /// Requested character height in 26.6 points.
    pub char_height: i32,
}

/// Request kind accepted by `FT_Request_Size`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeRequestType {
    Nominal,
    RealDim,
    BBox,
    Cell,
    Scales,
}

/// Validated FreeType-style size request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeRequest {
    pub request_type: SizeRequestType,
    pub width: i64,
    pub height: i64,
    pub hori_resolution: u32,
    pub vert_resolution: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeRequestError {
    DivideByZero,
    InvalidPixelSize,
    InvalidPpem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectSizeError {
    NoFixedSizes,
    InvalidArgument,
}

/// Public face metadata matching the scalar fields exposed by `FT_Face`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceInfo {
    pub num_faces: usize,
    pub face_index: usize,
    pub family_name: String,
    pub style_name: String,
    pub postscript_name: Option<String>,
    pub font_format: &'static str,
    pub units_per_em: u16,
    pub num_glyphs: u16,
    pub bbox: BBox,
    pub ascender: i16,
    pub descender: i16,
    pub height: i16,
    pub max_advance_width: i32,
    pub max_advance_height: i32,
    pub underline_position: i16,
    pub underline_thickness: i16,
    pub face_flags: u32,
    pub style_flags: u32,
    pub fs_type_flags: u16,
}

/// A selectable charmap descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharmapInfo {
    pub index: usize,
    pub platform_id: u16,
    pub encoding_id: u16,
    pub format: u16,
    pub language_id: u32,
}

/// Raw SFNT table descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SfntTableInfo {
    pub index: usize,
    pub tag: u32,
    pub offset: u32,
    pub length: u32,
}

/// A rendered glyph alpha mask.
#[derive(Debug, Clone)]
pub struct GlyphMask {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Left bearing offset in pixels (bbox xmin, may be negative).
    /// Used by the compositor to place the glyph horizontally.
    pub xmin: i32,
    /// Top bearing offset in pixels (bbox ymin, FreeType y-up coordinate space).
    pub ymin: i32,
    /// Rounded horizontal advance in integer pixels.
    pub advance_width: i32,
}

/// FreeType glyph slot metrics in 26.6 pixel units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphSlotMetrics {
    pub width: i32,
    pub height: i32,
    pub hori_bearing_x: i32,
    pub hori_bearing_y: i32,
    pub hori_advance: i32,
    pub vert_bearing_x: i32,
    pub vert_bearing_y: i32,
    pub vert_advance: i32,
}

/// Composite subglyph transform returned by FreeType's `FT_Get_SubGlyph_Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubGlyphTransform {
    pub xx: i32,
    pub xy: i32,
    pub yx: i32,
    pub yy: i32,
}

/// Composite subglyph record returned by FreeType's `FT_Get_SubGlyph_Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubGlyphInfo {
    pub index: u16,
    pub flags: u16,
    pub arg1: i32,
    pub arg2: i32,
    pub transform: SubGlyphTransform,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GlyphSlotLoad {
    pub metrics: GlyphSlotMetrics,
    pub format: GlyphSlotLoadFormat,
    pub outline_cbox: BBox,
    pub outline_bbox: BBox,
    pub subglyphs: Vec<SubGlyphInfo>,
    pub slot_outline: Option<Outline>,
    pub render_outline: Option<LoadedOutline>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlyphSlotLoadFormat {
    Outline,
    Composite,
}

fn subglyphs_from_components(components: &[tt::glyf::CompositeComponent]) -> Vec<SubGlyphInfo> {
    components
        .iter()
        .map(|component| SubGlyphInfo {
            index: component.glyph_index,
            flags: component.flags,
            arg1: component.arg1,
            arg2: component.arg2,
            transform: SubGlyphTransform {
                xx: component.transform.xx,
                xy: component.transform.xy,
                yx: component.transform.yx,
                yy: component.transform.yy,
            },
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoadedOutline {
    pub outline: Outline,
    pub left: i32,
    pub bottom: i32,
    pub top: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricsGridFit {
    None,
    Horizontal,
    Vertical,
}

impl Font {
    /// Load a FreeType-style memory face from raw bytes.
    ///
    /// This accepts the supported SFNT faces plus the compact Type 1
    /// non-SFNT fixtures needed for public face/name error-path parity.
    pub(crate) fn memory_face(
        data: &[u8],
        face_index: usize,
        size_pt: f32,
    ) -> Result<Self, FontError> {
        if matches!(read_u16_le(data, 0), Some(0x0200 | 0x0300)) {
            return Self::winfnt_face(data, face_index, size_pt);
        }
        if bdf_text(data)
            .and_then(first_bdf_keyword)
            .is_some_and(is_bdf_probe_keyword)
        {
            if let Some(error) = parse_bdf_constructor_error(data) {
                return Err(error);
            }
            return Self::bdf_face(data, face_index, size_pt);
        }
        if type1_cleartext(data).is_some() {
            return Self::type1_face(data, face_index, size_pt);
        }
        Self::truetype_face(data, face_index, size_pt)
    }

    /// Load a TrueType/OpenType font from raw bytes at a given point size.
    ///
    /// Parses all required tables eagerly. Matches `FT_New_Memory_Face` +
    /// `FT_Set_Char_Size` for the table subset this crate exposes.
    ///
    /// # Errors
    ///
    /// Returns [`FontError::InvalidFont`] if the data is not a valid
    /// TrueType/OpenType font, or if any required table is missing or
    /// malformed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use fontdone::Font;
    /// let font_data = std::fs::read("DejaVuSans.ttf").unwrap();
    /// let font = Font::truetype(&font_data, 10.0).unwrap();
    /// assert_eq!(font.getname(), ("DejaVu Sans", "Book"));
    /// ```
    pub fn truetype(data: &[u8], size_pt: f32) -> Result<Self, FontError> {
        Self::truetype_with_load_mode(data, size_pt, LoadMode::Default)
    }

    /// Load a TrueType/OpenType font with an explicit FreeType load mode.
    pub fn truetype_with_load_mode(
        data: &[u8],
        size_pt: f32,
        load_mode: LoadMode,
    ) -> Result<Self, FontError> {
        Self::truetype_face_with_load_mode(data, 0, size_pt, load_mode)
    }

    /// Load a specific face from raw SFNT/TTC bytes.
    ///
    /// `face_index` follows FreeType's zero-based face selection semantics.
    pub fn truetype_face(data: &[u8], face_index: usize, size_pt: f32) -> Result<Self, FontError> {
        Self::truetype_face_with_load_mode(data, face_index, size_pt, LoadMode::Default)
    }

    fn type1_face(data: &[u8], face_index: usize, size_pt: f32) -> Result<Self, FontError> {
        if face_index != 0 {
            return Err(FontError::InvalidFont(format!(
                "face index {face_index} out of range for 1 face(s)"
            )));
        }
        let cleartext = type1_cleartext(data)
            .ok_or_else(|| FontError::InvalidFont("missing Type 1 clear-text dictionary".into()))?;
        let metadata = parse_type1_metadata(cleartext)?;
        let type1_multi_master = parse_type1_multi_master(cleartext).map(Arc::new);
        let type1_font_info = Type1FontInfo {
            version: metadata.version.clone(),
            notice: metadata.notice.clone(),
            full_name: metadata.full_name.clone(),
            family_name: Some(metadata.family_name.clone()),
            weight: Some(metadata.style_name.clone()),
            // FreeType Type 1 `t1tokens.h` stores FontInfo `ItalicAngle` as
            // fixed-point; the public `PS_FontInfoRec` exposes 16.16 units.
            italic_angle: i32::from(metadata.italic_angle) << 16,
            is_fixed_pitch: metadata.is_fixed_pitch,
            underline_position: metadata.underline_position,
            underline_thickness: u16::try_from(metadata.underline_thickness.max(0)).unwrap_or(0),
        };
        let type1_private = parse_type1_private(data);
        let type1_encoding = parse_type1_encoding(cleartext);
        let type1_charstrings = parse_type1_charstrings(
            data,
            type1_private.as_ref().map_or(4, |private| private.len_iv),
        );
        let type1_mm_weight_vector = type1_multi_master
            .as_ref()
            .map(|master| master.default_weight_vector.clone());
        let font_data = type1_font_data(data, size_pt, &metadata);
        let is_italic = metadata.italic_angle != 0;
        let face_globals = crate::autohint::globals::FaceGlobals::new(font_data.clone(), is_italic);
        let size_metrics = SizeMetrics::from_char_size(
            i32_from_f32((size_pt * 64.0).round()),
            i32_from_f32((size_pt * 64.0).round()),
            72,
            72,
            font_data.as_ref(),
        );
        sync_active_size_metrics(&font_data, size_metrics);
        let family_name = font_data.name.family.clone();
        let subfamily_name = font_data.name.subfamily.clone();

        Ok(Font {
            data: font_data,
            size_pt,
            load_mode: LoadMode::Default,
            face_kind: FaceKind::Type1 {
                is_fixed_pitch: metadata.is_fixed_pitch,
            },
            type1_font_info: Some(type1_font_info),
            type1_encoding,
            type1_private,
            type1_charstrings,
            type1_multi_master,
            type1_mm_weight_vector,
            type1_mm_variation_active: false,
            face_globals,
            is_italic,
            family_name,
            subfamily_name,
            bdf_properties: Vec::new(),
            size_metrics,
            selected_charmap: 0,
            bytecode_context: BytecodeContextCache::default(),
            raster_scratch: std::cell::RefCell::new(crate::grays::RasterScratch::new()),
        })
    }

    fn bdf_face(data: &[u8], face_index: usize, size_pt: f32) -> Result<Self, FontError> {
        if face_index != 0 {
            return Err(FontError::InvalidFont(format!(
                "face index {face_index} out of range for 1 face(s)"
            )));
        }
        let text = bdf_text(data)
            .ok_or_else(|| FontError::InvalidFont("BDF stream is not text".into()))?;
        let metadata = parse_bdf_metadata(text)?;
        let font_data = bdf_font_data(data, size_pt, &metadata);
        let face_globals = crate::autohint::globals::FaceGlobals::new(font_data.clone(), false);
        let size_metrics = SizeMetrics::from_pixel_size(
            u32::from(u16::try_from(metadata.pixel_width.max(1)).unwrap_or(1)),
            u32::from(u16::try_from(metadata.pixel_height.max(1)).unwrap_or(1)),
            font_data.as_ref(),
        );
        sync_active_size_metrics(&font_data, size_metrics);
        let family_name = font_data.name.family.clone();
        let subfamily_name = font_data.name.subfamily.clone();
        let bdf_properties = metadata.properties;

        Ok(Font {
            data: font_data,
            size_pt,
            load_mode: LoadMode::Default,
            face_kind: FaceKind::Bdf,
            type1_font_info: None,
            type1_encoding: None,
            type1_private: None,
            type1_charstrings: Vec::new(),
            type1_multi_master: None,
            type1_mm_weight_vector: None,
            type1_mm_variation_active: false,
            face_globals,
            is_italic: false,
            family_name,
            subfamily_name,
            bdf_properties,
            size_metrics,
            selected_charmap: 0,
            bytecode_context: BytecodeContextCache::default(),
            raster_scratch: std::cell::RefCell::new(crate::grays::RasterScratch::new()),
        })
    }

    fn winfnt_face(data: &[u8], face_index: usize, size_pt: f32) -> Result<Self, FontError> {
        if face_index != 0 {
            return Err(FontError::InvalidFont(format!(
                "face index {face_index} out of range for 1 face(s)"
            )));
        }
        let header = parse_winfnt_header(data)?;
        let font_data = winfnt_font_data(data, size_pt, &header);
        let is_italic = header.italic != 0;
        let face_globals = crate::autohint::globals::FaceGlobals::new(font_data.clone(), is_italic);
        let size_metrics = SizeMetrics::from_char_size(
            i32_from_f32((size_pt * 64.0).round()),
            i32_from_f32((size_pt * 64.0).round()),
            72,
            72,
            font_data.as_ref(),
        );
        sync_active_size_metrics(&font_data, size_metrics);
        let family_name = font_data.name.family.clone();
        let subfamily_name = font_data.name.subfamily.clone();

        Ok(Font {
            data: font_data,
            size_pt,
            load_mode: LoadMode::Default,
            face_kind: FaceKind::WinFnt { header },
            type1_font_info: None,
            type1_encoding: None,
            type1_private: None,
            type1_charstrings: Vec::new(),
            type1_multi_master: None,
            type1_mm_weight_vector: None,
            type1_mm_variation_active: false,
            face_globals,
            is_italic,
            family_name,
            subfamily_name,
            bdf_properties: Vec::new(),
            size_metrics,
            selected_charmap: 0,
            bytecode_context: BytecodeContextCache::default(),
            raster_scratch: std::cell::RefCell::new(crate::grays::RasterScratch::new()),
        })
    }

    /// Load a specific face from raw SFNT/TTC bytes with an explicit load mode.
    pub fn truetype_face_with_load_mode(
        data: &[u8],
        face_index: usize,
        size_pt: f32,
        load_mode: LoadMode,
    ) -> Result<Self, FontError> {
        Self::truetype_face_with_load_mode_and_design_coords(
            data, face_index, size_pt, load_mode, None, None, false,
        )
    }

    fn truetype_face_with_load_mode_and_design_coords(
        data: &[u8],
        face_index: usize,
        size_pt: f32,
        load_mode: LoadMode,
        design_coords: Option<&[i32]>,
        blend_coords_16_16: Option<&[i32]>,
        variation_coordinates_set: bool,
    ) -> Result<Self, FontError> {
        // FreeType stores a 1-based named-instance selector in bits 16..30;
        // the low 16 bits still select the collection face (ftobjs.c).
        let collection_face_index = face_index & 0xFFFF;
        let (num_faces, face_offset) = tt::resolve_face_index(data, collection_face_index)?;
        let dir = tt::parse_table_directory_at(data, face_offset)?;
        let fvar = dir
            .find(data, tag(b"fvar"))
            .and_then(|bytes| tt::fvar::parse_fvar(bytes).ok());
        let named_instance = (face_index >> 16) & 0x7FFF;
        if named_instance != 0
            && fvar
                .as_ref()
                .is_none_or(|table| named_instance > usize::from(table.instance_count))
        {
            return Err(FontError::InvalidFont(format!(
                "named instance {named_instance} is unavailable"
            )));
        }

        let head_bytes = dir
            .find(data, tag(b"head"))
            .ok_or_else(|| FontError::InvalidFont("missing 'head' table".into()))?;
        let head = tt::head::parse_head(head_bytes)?;

        // tt_face_load_maxp reads its optional 26-byte frame from the SFNT
        // stream after goto_table, without constraining reads to table length.
        let maxp_record = dir
            .record(tag(b"maxp"))
            .ok_or_else(|| FontError::InvalidFont("missing 'maxp' table".into()))?;
        let maxp_bytes = data
            .get(maxp_record.offset as usize..)
            .ok_or_else(|| FontError::InvalidFont("invalid 'maxp' table offset".into()))?;
        // sfnt_load_face intentionally continues after a maxp load error.
        let maxp = tt::maxp::parse_maxp(maxp_bytes).unwrap_or_default();

        let cmap_bytes = dir
            .find(data, tag(b"cmap"))
            .ok_or_else(|| FontError::InvalidFont("missing 'cmap' table".into()))?;
        let cmap = tt::cmap::parse_cmap(cmap_bytes)?;

        let hhea_bytes = dir
            .find(data, tag(b"hhea"))
            .ok_or_else(|| FontError::InvalidFont("missing 'hhea' table".into()))?;
        let hhea = tt::hhea::parse_hhea(hhea_bytes)?;

        let hmtx_bytes = dir
            .find(data, tag(b"hmtx"))
            .ok_or_else(|| FontError::InvalidFont("missing 'hmtx' table".into()))?;
        // tt_face_load_hmtx records the table offset and size; malformed
        // metrics are observed later as zero advance and bearing values.
        let hmtx = tt::hmtx::parse_hmtx(hmtx_bytes, hhea.num_hmetrics, maxp.num_glyphs)
            .unwrap_or_default();

        let mut name = match dir.find(data, tag(b"name")) {
            Some(d) => tt::name::parse_name(d)?,
            None => crate::tt::name::NameTable {
                format: 0,
                family: "Unknown".into(),
                subfamily: "Regular".into(),
                postscript_name: None,
                records: Vec::new(),
                lang_tags: Vec::new(),
            },
        };
        if named_instance != 0 {
            if let Some(instance_name) =
                named_instance_postscript_name(&name, &fvar, named_instance)
            {
                name.postscript_name = Some(instance_name);
            }
        }

        let os2 = dir.find(data, tag(b"OS/2")).and_then(tt::os2::parse_os2);
        let is_wws_only = os2.as_ref().is_some_and(tt::os2::Os2Table::is_wws_only);
        name.family = tt::name::family_name(&name, false, is_wws_only);
        name.subfamily = tt::name::subfamily_name(&name, false, is_wws_only);
        let post = dir.find(data, tag(b"post")).and_then(tt::post::parse_post);
        // `tt_face_load_gasp` calls `goto_table` with a null length pointer and
        // then reads frames from the stream, so the SFNT table record length
        // does not cap readable bytes for this optional table.
        let gasp = dir
            .record(tag(b"gasp"))
            .and_then(|record| data.get(record.offset as usize..))
            .and_then(|d| tt::gasp::parse_gasp(d).ok());
        let vhea = match dir.find(data, tag(b"vhea")) {
            Some(bytes) => Some(tt::vhea::parse_vhea(bytes)?),
            None => None,
        };
        let vmtx = vhea.as_ref().and_then(|vhea| {
            dir.find(data, tag(b"vmtx")).map(|d| {
                tt::vmtx::parse_vmtx(d, vhea.num_vmetrics, maxp.num_glyphs).unwrap_or_default()
            })
        });
        let hdmx = dir
            .find(data, tag(b"hdmx"))
            .and_then(|d| tt::hdmx::parse_hdmx(d, maxp.num_glyphs).ok());
        let gvar = dir
            .find(data, tag(b"gvar"))
            .and_then(|d| tt::gvar::parse_gvar(d, maxp.num_glyphs).ok());
        let design_variation_coords = if let Some(coords) = design_coords {
            design_variation_coords_for_design_coords(&fvar, coords)
        } else {
            design_variation_coords_for_named_instance(&fvar, named_instance)
        };
        let normalized_variation_coords = if design_coords.is_some() {
            normalized_variation_coords_for_design_coords(&fvar, &design_variation_coords)
        } else {
            normalized_variation_coords_for_named_instance(&fvar, named_instance)
        };
        let blend_variation_coords_16_16 = blend_coords_16_16.map_or_else(
            || {
                normalized_variation_coords
                    .iter()
                    .map(|coord| i32::from(*coord) << 2)
                    .collect()
            },
            |coords| blend_variation_coords_for_blend_coords_16_16(&fvar, coords),
        );
        let hvar = dir.find(data, tag(b"HVAR")).and_then(|d| {
            fvar.as_ref()
                .and_then(|fvar| tt::hvar::HvarTable::parse(d, fvar.axes.len()).ok())
        });
        let mvar = dir.find(data, tag(b"MVAR")).and_then(|d| {
            fvar.as_ref()
                .and_then(|fvar| tt::mvar::MvarTable::parse(d, fvar.axes.len()).ok())
        });
        let kern = dir
            .find(data, tag(b"kern"))
            .and_then(|d| tt::kern::parse_kern(d).ok());
        let sbit = tt::sbit::parse_sbit(&dir, data);
        let cff = dir
            .find(data, tag(b"CFF "))
            .map(tt::cff::parse_cff)
            .transpose()?;

        let loca_data = match dir.find(data, tag(b"loca")) {
            Some(bytes) => bytes.to_vec(),
            None if cff.is_some() => Vec::new(),
            None => return Err(FontError::InvalidFont("missing 'loca' table".into())),
        };
        let glyf_data = match dir.find(data, tag(b"glyf")) {
            Some(bytes) => bytes.to_vec(),
            None if cff.is_some() => Vec::new(),
            None => return Err(FontError::InvalidFont("missing 'glyf' table".into())),
        };

        // Bytecode tables are optional. When present they are used by the
        // native TrueType path to match FreeType's default load behavior.
        // Missing tables fall back to unhinted scaling (same behavior as FreeType
        // without TT_USE_BYTECODE_INTERPRETER).
        let fpgm = dir
            .find(data, tag(b"fpgm"))
            .map(crate::tt::hinter::tables::parse_fpgm);
        let prep = dir
            .find(data, tag(b"prep"))
            .map(crate::tt::hinter::tables::parse_prep);
        let cvt = dir
            .find(data, tag(b"cvt "))
            .and_then(|d| crate::tt::hinter::tables::parse_cvt(d).ok());

        // Build FontData first, then compute Latin autohinter metrics.
        // `FaceGlobals` and scaler paths share `FontData` through `Arc`; the
        // face itself is not a cross-thread type.
        #[allow(clippy::arc_with_non_send_sync)]
        let font_data = Arc::new(FontData {
            raw_data: data.to_vec(),
            face_offset,
            face_index,
            num_faces,
            table_directory: dir,
            cmap,
            fvar,
            gvar,
            design_variation_coords,
            normalized_variation_coords,
            blend_variation_coords_16_16,
            variation_coordinates_set,
            head,
            hhea,
            hvar,
            mvar,
            hmtx,
            maxp,
            name,
            os2,
            post,
            gasp,
            vhea,
            vmtx,
            hdmx,
            kern,
            sbit,
            cff,
            loca_data,
            glyf_data,
            size_pt: std::cell::Cell::new(size_pt),
            size_public_x_scale: std::cell::Cell::new(0),
            size_public_y_scale: std::cell::Cell::new(0),
            size_x_scale: std::cell::Cell::new(0),
            size_y_scale: std::cell::Cell::new(0),
            size_tt_scale: std::cell::Cell::new(0),
            size_tt_ppem: std::cell::Cell::new(0),
            size_tt_x_ratio: std::cell::Cell::new(0x1_0000),
            size_tt_y_ratio: std::cell::Cell::new(0x1_0000),
            size_tt_point_size: std::cell::Cell::new(0),
            transform_xx: std::cell::Cell::new(0x1_0000),
            transform_xy: std::cell::Cell::new(0),
            transform_yx: std::cell::Cell::new(0),
            transform_yy: std::cell::Cell::new(0x1_0000),
            transform_dx: std::cell::Cell::new(0),
            transform_dy: std::cell::Cell::new(0),
            fpgm,
            prep,
            cvt,
            glyph_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            self_arc: std::sync::OnceLock::new(),
        });
        // Set the self-referencing Arc pointer so scaler paths can clone it cheaply.
        let _ = font_data.self_arc.set(font_data.clone());

        let _upem = font_data.head.units_per_em as i32;
        let is_italic = (font_data.head.mac_style & 2) != 0;
        let face_globals = crate::autohint::globals::FaceGlobals::new(font_data.clone(), is_italic);
        let size_metrics = SizeMetrics::from_char_size(
            i32_from_f32((size_pt * 64.0).round()),
            i32_from_f32((size_pt * 64.0).round()),
            72,
            72,
            font_data.as_ref(),
        );
        sync_active_size_metrics(&font_data, size_metrics);

        let selected_charmap = default_unicode_charmap_index(&font_data.cmap).unwrap_or(0);
        let family_name = font_data.name.family.clone();
        let subfamily_name = font_data.name.subfamily.clone();

        Ok(Font {
            data: font_data,
            size_pt,
            load_mode,
            face_kind: FaceKind::Sfnt,
            type1_font_info: None,
            type1_encoding: None,
            type1_private: None,
            type1_charstrings: Vec::new(),
            type1_multi_master: None,
            type1_mm_weight_vector: None,
            type1_mm_variation_active: false,
            face_globals,
            is_italic,
            family_name,
            subfamily_name,
            bdf_properties: Vec::new(),
            size_metrics,
            selected_charmap,
            bytecode_context: BytecodeContextCache::default(),
            raster_scratch: std::cell::RefCell::new(crate::grays::RasterScratch::new()),
        })
    }

    /// Return the number of faces in an SFNT/TTC byte slice.
    pub fn face_count(data: &[u8]) -> Result<usize, FontError> {
        Ok(tt::face_offsets(data)?.len())
    }

    /// Return the active face index.
    pub fn face_index(&self) -> usize {
        self.data.face_index
    }

    /// Select or clear a named instance, equivalent to `FT_Set_Named_Instance`.
    pub fn set_named_instance(&mut self, instance_index: usize) -> Result<(), FontError> {
        if self.type1_multi_master.is_some() {
            // C parity: src/type1/t1load.c:T1_Reset_MM_Blend ignores the
            // instance index for Adobe MM faces and resets the design by
            // restoring the default WeightVector.
            self.set_type1_mm_weight_vector(None)?;
            return Ok(());
        }
        let base_face_index = self.data.face_index & 0xFFFF;
        let next_face_index = base_face_index | (instance_index << 16);
        let mut next = Self::truetype_face_with_load_mode(
            &self.data.raw_data,
            next_face_index,
            self.size_pt,
            self.load_mode,
        )?;
        next.selected_charmap = next
            .data
            .cmap
            .charmaps
            .len()
            .checked_sub(1)
            .map_or(0, |last| self.selected_charmap.min(last));
        *self = next;
        Ok(())
    }

    /// Set explicit OpenType design coordinates, equivalent to
    /// `FT_Set_Var_Design_Coordinates` for TrueType/OpenType variation faces.
    pub(crate) fn set_var_design_coordinates(&mut self, coords: &[i32]) -> Result<(), FontError> {
        if self.type1_multi_master.is_some() {
            let mm_coords = coords.iter().map(|coord| coord >> 16).collect::<Vec<_>>();
            return self.set_type1_mm_design_coordinates(&mm_coords, !coords.is_empty());
        }
        let base_face_index = self.data.face_index & 0xFFFF;
        // C parity: src/base/ftmm.c:281-360 clears FT_FACE_FLAG_VARIATION after
        // a successful zero-count FT_Set_Var_Design_Coordinates reset while the
        // TrueType service recomputes default design/blend coordinates.
        let variation_coordinates_set = !coords.is_empty();
        let mut next = Self::truetype_face_with_load_mode_and_design_coords(
            &self.data.raw_data,
            base_face_index,
            self.size_pt,
            self.load_mode,
            Some(coords),
            None,
            variation_coordinates_set,
        )?;
        next.selected_charmap = next
            .data
            .cmap
            .charmaps
            .len()
            .checked_sub(1)
            .map_or(0, |last| self.selected_charmap.min(last));
        *self = next;
        Ok(())
    }

    /// Return active OpenType design coordinates, equivalent to
    /// `FT_Get_Var_Design_Coordinates` for TrueType/OpenType variation faces.
    pub(crate) fn var_design_coordinates(&self) -> Result<&[i32], FontError> {
        if self.type1_multi_master.is_some() {
            return Err(FontError::InvalidArgument(
                "Type 1 MM design coordinates require fixed output synthesis".into(),
            ));
        }
        if self.data.fvar.is_none() {
            return Err(FontError::InvalidArgument(
                "face has no variation design coordinates".into(),
            ));
        }
        Ok(&self.data.design_variation_coords)
    }

    /// Return active normalized blend coordinates in FreeType's 16.16 public
    /// representation, equivalent to `FT_Get_MM_Blend_Coordinates`.
    pub(crate) fn var_blend_coordinates_16_16(&self) -> Result<Vec<i32>, FontError> {
        if self.type1_multi_master.is_some() {
            return self.type1_mm_blend_coordinates_16_16(
                self.type1_multi_master
                    .as_ref()
                    .map_or(0, |master| master.axes.len()),
            );
        }
        if self.data.fvar.is_none() {
            return Err(FontError::InvalidArgument(
                "face has no variation blend coordinates".into(),
            ));
        }
        Ok(self.data.blend_variation_coords_16_16.to_vec())
    }

    pub(crate) fn type1_multi_master(&self) -> Option<&Type1MultiMaster> {
        self.type1_multi_master.as_deref()
    }

    pub(crate) fn type1_font_info(&self) -> Option<&Type1FontInfo> {
        self.type1_font_info.as_ref()
    }

    pub(crate) fn postscript_font_info(&self) -> Option<&Type1FontInfo> {
        self.type1_font_info
            .as_ref()
            .or_else(|| self.data.cff.as_ref().map(tt::cff::CffTable::font_info))
    }

    pub(crate) fn has_postscript_glyph_names(&self) -> bool {
        const FT_FACE_FLAG_GLYPH_NAMES: u32 = 1 << 9;
        self.type1_font_info.is_some()
            || (self.data.cff.is_some() && self.face_flags() & FT_FACE_FLAG_GLYPH_NAMES != 0)
    }

    pub(crate) fn type1_encoding(&self) -> Option<&Type1EncodingInfo> {
        self.type1_encoding.as_ref()
    }

    pub(crate) fn type1_private(&self) -> Option<&Type1PrivateDict> {
        self.type1_private.as_ref()
    }

    pub(crate) fn type1_mm_weight_vector(&self) -> Result<&[i32], FontError> {
        self.type1_mm_weight_vector
            .as_deref()
            .ok_or_else(|| FontError::InvalidArgument("face has no Type 1 MM weight vector".into()))
    }

    pub(crate) fn type1_mm_design_coordinates(&self, count: usize) -> Result<Vec<i32>, FontError> {
        let Some(master) = self.type1_multi_master.as_ref() else {
            return Err(FontError::InvalidArgument(
                "face has no Type 1 MM design coordinates".into(),
            ));
        };
        let weights = self.type1_mm_weight_vector()?;
        let axis_coords = type1_mm_weights_unmap(weights, master.axes.len());
        let mut out = Vec::with_capacity(count);
        for (axis, coord) in master.axes.iter().zip(axis_coords).take(count) {
            out.push(type1_mm_axis_unmap(&axis.design_map, coord));
        }
        out.resize(count, 0);
        Ok(out)
    }

    pub(crate) fn type1_mm_default_design_coordinates(&self) -> Result<Vec<i32>, FontError> {
        let Some(master) = self.type1_multi_master.as_ref() else {
            return Err(FontError::InvalidArgument(
                "face has no Type 1 MM default design coordinates".into(),
            ));
        };
        let axis_coords = type1_mm_weights_unmap(&master.default_weight_vector, master.axes.len());
        Ok(master
            .axes
            .iter()
            .zip(axis_coords)
            .map(|(axis, coord)| type1_mm_axis_unmap(&axis.design_map, coord))
            .collect())
    }

    pub(crate) fn type1_mm_blend_coordinates_16_16(
        &self,
        count: usize,
    ) -> Result<Vec<i32>, FontError> {
        let Some(master) = self.type1_multi_master.as_ref() else {
            return Err(FontError::InvalidArgument(
                "face has no Type 1 MM blend coordinates".into(),
            ));
        };
        let weights = self.type1_mm_weight_vector()?;
        let axis_coords = type1_mm_weights_unmap(weights, master.axes.len());
        let mut out = Vec::with_capacity(count);
        out.extend(axis_coords.into_iter().take(count));
        out.resize(count, 0x8000);
        Ok(out)
    }

    pub(crate) fn set_type1_mm_design_coordinates(
        &mut self,
        coords: &[i32],
        variation_active: bool,
    ) -> Result<(), FontError> {
        let Some(master) = self.type1_multi_master.as_ref() else {
            return Err(FontError::InvalidArgument(
                "face has no Type 1 MM design coordinates".into(),
            ));
        };
        let blends = master
            .axes
            .iter()
            .enumerate()
            .map(|(index, axis)| {
                let design = coords.get(index).copied().unwrap_or_else(|| {
                    let first = axis.design_map.first().map_or(0, |point| point.design);
                    let last = axis.design_map.last().map_or(first, |point| point.design);
                    // C parity: src/type1/t1load.c:T1_Set_MM_Design uses
                    // `(last - first) / 2` as the missing-coordinate default.
                    (last - first) / 2
                });
                type1_mm_design_to_blend(&axis.design_map, design)
            })
            .collect::<Vec<_>>();
        self.type1_mm_weight_vector = Some(type1_mm_weights_from_blends(&blends, blends.len()));
        self.type1_mm_variation_active = variation_active;
        Ok(())
    }

    pub(crate) fn set_type1_mm_weight_vector(
        &mut self,
        weightvector: Option<&[i32]>,
    ) -> Result<(), FontError> {
        let Some(master) = self.type1_multi_master.as_ref() else {
            return Err(FontError::InvalidArgument(
                "face has no Type 1 MM weight vector".into(),
            ));
        };
        let next = match weightvector {
            None => master.default_weight_vector.clone(),
            Some(weightvector) => {
                // C parity: src/type1/t1load.c:T1_Set_MM_WeightVector
                // copies up to `num_designs`, zero-fills missing design weights,
                // ignores excess values, and does not normalize the weight sum.
                let mut next = vec![0; master.num_designs];
                let copy_len = weightvector.len().min(master.num_designs);
                next[..copy_len].copy_from_slice(&weightvector[..copy_len]);
                next
            }
        };
        self.type1_mm_weight_vector = Some(next);
        self.type1_mm_variation_active = weightvector.is_some();
        Ok(())
    }

    pub(crate) fn set_type1_mm_blend_coordinates(
        &mut self,
        coords_16_16: &[i32],
        variation_active: bool,
    ) -> Result<(), FontError> {
        let Some(master) = self.type1_multi_master.as_ref() else {
            return Err(FontError::InvalidArgument(
                "face has no Type 1 MM blend coordinates".into(),
            ));
        };
        let axis_count = master.axes.len();
        let mut blends = vec![0x8000; axis_count];
        let copy_len = coords_16_16.len().min(axis_count);
        blends[..copy_len].copy_from_slice(&coords_16_16[..copy_len]);
        // C parity: src/type1/t1load.c:t1_set_mm_blend clamps num_coords to
        // num_axis, ignores extra coordinates, and treats omitted axes as
        // the default 0.5 blend factor while recomputing WeightVector.
        self.type1_mm_weight_vector = Some(type1_mm_weights_from_blends(&blends, axis_count));
        self.type1_mm_variation_active = variation_active;
        Ok(())
    }

    /// Set normalized blend coordinates, equivalent to
    /// `FT_Set_MM_Blend_Coordinates` / `FT_Set_Var_Blend_Coordinates`.
    pub(crate) fn set_var_blend_coordinates(
        &mut self,
        coords_16_16: &[i32],
    ) -> Result<(), FontError> {
        let Some(fvar) = &self.data.fvar else {
            return Err(FontError::InvalidArgument(
                "face has no variation blend coordinates".into(),
            ));
        };
        if coords_16_16.is_empty() {
            // C parity: TT_Set_MM_Blend(num_coords=0, coords=NULL) doesn't
            // overwrite the existing blend/design arrays; the public wrapper
            // only clears FT_FACE_FLAG_VARIATION.  See
            // freetype/src/base/ftmm.c:465-525 and
            // freetype/src/truetype/ttgxvar.c:2918-3184.
            let base_face_index = self.data.face_index & 0xFFFF;
            let mut next = Self::truetype_face_with_load_mode_and_design_coords(
                &self.data.raw_data,
                base_face_index,
                self.size_pt,
                self.load_mode,
                Some(&self.data.design_variation_coords),
                Some(&self.data.blend_variation_coords_16_16),
                false,
            )?;
            next.selected_charmap = next
                .data
                .cmap
                .charmaps
                .len()
                .checked_sub(1)
                .map_or(0, |last| self.selected_charmap.min(last));
            *self = next;
            return Ok(());
        }
        let design_coords = fvar
            .axes
            .iter()
            .enumerate()
            .map(|(index, axis)| {
                let blend = coords_16_16.get(index).copied().unwrap_or(0);
                design_coord_for_normalized_blend_16_16(blend, axis)
            })
            .collect::<Vec<_>>();
        let variation_coordinates_set = coords_16_16.iter().any(|coord| *coord != 0);
        let base_face_index = self.data.face_index & 0xFFFF;
        let mut next = Self::truetype_face_with_load_mode_and_design_coords(
            &self.data.raw_data,
            base_face_index,
            self.size_pt,
            self.load_mode,
            Some(&design_coords),
            Some(coords_16_16),
            variation_coordinates_set,
        )?;
        next.selected_charmap = next
            .data
            .cmap
            .charmaps
            .len()
            .checked_sub(1)
            .map_or(0, |last| self.selected_charmap.min(last));
        *self = next;
        Ok(())
    }

    pub(crate) fn mvar_vertical_header_deltas(
        &self,
    ) -> Option<crate::tt::mvar::VerticalHeaderDeltas> {
        self.data.mvar_vertical_header_deltas()
    }

    pub(crate) fn normalized_variation_coords(&self) -> &[i16] {
        &self.data.normalized_variation_coords
    }

    /// Return the number of faces in the original font resource.
    pub fn num_faces(&self) -> usize {
        self.data.num_faces
    }

    /// Return whether this face uses the SFNT storage scheme.
    pub(crate) fn is_sfnt(&self) -> bool {
        self.face_kind == FaceKind::Sfnt
    }

    /// Return the parsed Windows FNT header for WinFNT faces.
    pub fn winfnt_header(&self) -> Option<&WinFntHeader> {
        match &self.face_kind {
            FaceKind::WinFnt { header } => Some(header),
            _ => None,
        }
    }

    /// Return one parsed BDF font property for bitmap BDF faces.
    pub fn bdf_property(&self, name: &str) -> Option<&BdfPropertyValue> {
        if self.face_kind != FaceKind::Bdf {
            return None;
        }
        self.bdf_properties
            .iter()
            .find(|property| property.name == name)
            .map(|property| &property.value)
    }

    pub(crate) fn bdf_property_atom_c_str(&self, name: &str) -> Option<&CStr> {
        if self.face_kind != FaceKind::Bdf {
            return None;
        }
        self.bdf_properties
            .iter()
            .find(|property| property.name == name)
            .and_then(|property| property.atom_c_string.as_deref())
    }

    /// Return scalar face metadata.
    pub fn face_info(&self) -> FaceInfo {
        let (ascender, descender, height) = face_metric_values(&self.data);
        let (family_name, style_name) = self.getname_with_options();
        FaceInfo {
            num_faces: self.num_faces(),
            face_index: self.face_index(),
            family_name,
            style_name,
            postscript_name: self.data.name.postscript_name.clone(),
            font_format: self.font_format(),
            units_per_em: self.data.head.units_per_em,
            num_glyphs: self.data.maxp.num_glyphs,
            bbox: BBox {
                x_min: i32::from(self.data.head.x_min),
                y_min: i32::from(self.data.head.y_min),
                x_max: i32::from(self.data.head.x_max),
                y_max: i32::from(self.data.head.y_max),
            },
            ascender: i16_from_i32(ascender),
            descender: i16_from_i32(descender),
            height: i16_from_i32(height),
            max_advance_width: i32::from(self.data.hhea.advance_width_max),
            max_advance_height: self
                .data
                .vhea
                .as_ref()
                // FreeType `sfnt_init_face` sets `max_advance_height` from
                // `vhea.advance_Height_Max` when vertical metrics exist, and
                // falls back to the face height for scalable faces without
                // vertical info (`src/sfnt/sfobjs.c`, max_advance_height).
                .map_or(height, |vhea| i32::from(vhea.advance_height_max)),
            underline_position: self
                .data
                .post
                .as_ref()
                // FreeType converts TrueType `post.underlinePosition` from
                // top edge to stroke center by subtracting half the underline
                // thickness (`src/sfnt/sfobjs.c`, underline_position).
                .map_or(0, |post| {
                    post.underline_position - post.underline_thickness / 2
                }),
            underline_thickness: self
                .data
                .post
                .as_ref()
                .map_or(0, |post| post.underline_thickness),
            face_flags: self.face_flags(),
            style_flags: self.style_flags(),
            fs_type_flags: self.get_fstype_flags(),
        }
    }

    /// Equivalent to `FT_Get_Font_Format` for the supported SFNT wrappers.
    pub fn font_format(&self) -> &'static str {
        match self.face_kind {
            FaceKind::Bdf => return "BDF",
            FaceKind::Type1 { .. } => return "Type 1",
            FaceKind::WinFnt { .. } => return "Windows FNT",
            FaceKind::Sfnt => {}
        }
        let tag = u32::from_be_bytes([
            self.data.raw_data[self.data.face_offset],
            self.data.raw_data[self.data.face_offset + 1],
            self.data.raw_data[self.data.face_offset + 2],
            self.data.raw_data[self.data.face_offset + 3],
        ]);
        if tag == tt::OTTO_MAGIC {
            "CFF"
        } else {
            "TrueType"
        }
    }

    /// Equivalent to `FT_Get_Postscript_Name`.
    pub fn postscript_name(&self) -> Option<&str> {
        self.data.name.postscript_name.as_deref()
    }

    /// Equivalent to `FT_Get_Glyph_Name` for supported SFNT `post` names.
    pub fn glyph_name(&self, glyph_index: u32) -> Option<&str> {
        let glyph_index = usize::try_from(glyph_index).ok()?;
        if self.is_type1_face() {
            return self
                .type1_charstrings
                .get(glyph_index)
                .map(|charstring| charstring.name.as_str());
        }
        let post = self.data.post.as_ref()?;
        // C `FT_Get_Glyph_Name` checks `FT_FACE_FLAG_GLYPH_NAMES` before
        // dispatching to `tt_face_get_ps_name`; SFNT sets that flag only for
        // accepted post formats 1.0, 2.0, and 2.5 (`sfobjs.c:1118-1121`).
        if !matches!(post.format_type, 0x0001_0000 | 0x0002_0000 | 0x0002_5000) {
            return None;
        }
        post.glyph_name(glyph_index, self.data.maxp.num_glyphs)
    }

    /// Equivalent to `FT_Get_Name_Index` for supported SFNT `post` names.
    pub fn name_index(&self, glyph_name: &str) -> u32 {
        if self.is_type1_face() {
            return self
                .type1_charstrings
                .iter()
                .position(|charstring| charstring.name == glyph_name)
                .and_then(|index| u32::try_from(index).ok())
                .unwrap_or(0);
        }
        let Some(post) = self.data.post.as_ref() else {
            return 0;
        };
        if !matches!(post.format_type, 0x0001_0000 | 0x0002_0000 | 0x0002_5000) {
            return 0;
        }
        (0..u32::from(self.data.maxp.num_glyphs))
            .find(|glyph_index| self.glyph_name(*glyph_index) == Some(glyph_name))
            .unwrap_or(0)
    }

    /// Equivalent to `FT_Get_FSType_Flags`.
    pub fn get_fstype_flags(&self) -> u16 {
        self.data.os2.as_ref().map_or(0, |os2| os2.fs_type)
    }

    /// Equivalent to `FT_Get_Gasp`.
    pub fn get_gasp(&self, ppem: u32) -> i32 {
        self.data
            .gasp
            .as_ref()
            .map_or(tt::gasp::FT_GASP_NO_TABLE, |gasp| gasp.get(ppem))
    }

    /// Equivalent to `FT_Get_Kerning` for legacy horizontal `kern` tables.
    pub fn kerning_by_glyphs(&self, left: u32, right: u32, mode: KerningMode) -> (i32, i32) {
        let raw_x = left
            .try_into()
            .ok()
            .zip(right.try_into().ok())
            .and_then(|(left, right)| self.data.kern.as_ref().map(|kern| kern.get(left, right)))
            .map_or(0, i32::from);
        let mut x = raw_x;
        let mut y = 0;
        if mode != KerningMode::Unscaled {
            x = ft_mul_fix(x, self.size_metrics.x_scale);
            y = ft_mul_fix(y, self.size_metrics.y_scale);
            if mode == KerningMode::Default {
                // FreeType `FT_Get_Kerning` scales default-mode kerning down
                // below 25 ppem before `FT_PIX_ROUND` to avoid oversized
                // rounded distances at small sizes.
                if self.size_metrics.x_ppem < 25 {
                    x = ft_mul_div(x, i32::from(self.size_metrics.x_ppem), 25);
                }
                if self.size_metrics.y_ppem < 25 {
                    y = ft_mul_div(y, i32::from(self.size_metrics.y_ppem), 25);
                }
                x = ft_pix_round(x);
                y = ft_pix_round(y);
            }
        }
        (x, y)
    }

    pub(crate) fn os2_table(&self) -> Option<&tt::os2::Os2Table> {
        self.data.os2.as_ref()
    }

    pub(crate) fn is_cid_keyed(&self) -> bool {
        self.data
            .cff
            .as_ref()
            .is_some_and(tt::cff::CffTable::is_cid_keyed)
    }

    pub(crate) fn cid_for_glyph_index(&self, glyph_index: u32) -> Option<u16> {
        self.data
            .cff
            .as_ref()
            .and_then(tt::cff::CffTable::cid_info)
            .and_then(|cid| cid.cid_for_glyph_index(glyph_index))
    }

    pub(crate) fn cid_registry_ordering_supplement(&self) -> Option<(&str, &str, i32)> {
        self.data
            .cff
            .as_ref()
            .and_then(tt::cff::CffTable::cid_info)
            .map(|cid| (cid.registry(), cid.ordering(), cid.supplement()))
    }

    /// Number of raw SFNT name records exposed by `FT_Get_Sfnt_Name_Count`.
    pub fn sfnt_name_count(&self) -> usize {
        self.data.name.records.len()
    }

    /// Return one raw SFNT name record by index.
    pub fn sfnt_name(&self, index: usize) -> Option<&tt::name::SfntNameRecord> {
        self.data.name.records.get(index)
    }

    /// Raw SFNT name table format field.
    pub fn sfnt_name_format(&self) -> u16 {
        self.data.name.format
    }

    /// Return one raw SFNT language-tag record by index.
    pub fn sfnt_lang_tag(&self, index: usize) -> Option<&tt::name::SfntLangTagRecord> {
        self.data.name.lang_tags.get(index)
    }

    /// Approximate `FT_FaceRec::face_flags` for supported SFNT outline faces.
    pub fn face_flags(&self) -> u32 {
        const FT_FACE_FLAG_SCALABLE: u32 = 1 << 0;
        const FT_FACE_FLAG_FIXED_SIZES: u32 = 1 << 1;
        const FT_FACE_FLAG_FIXED_WIDTH: u32 = 1 << 2;
        const FT_FACE_FLAG_SFNT: u32 = 1 << 3;
        const FT_FACE_FLAG_HORIZONTAL: u32 = 1 << 4;
        const FT_FACE_FLAG_VERTICAL: u32 = 1 << 5;
        const FT_FACE_FLAG_KERNING: u32 = 1 << 6;
        const FT_FACE_FLAG_MULTIPLE_MASTERS: u32 = 1 << 8;
        const FT_FACE_FLAG_GLYPH_NAMES: u32 = 1 << 9;
        const FT_FACE_FLAG_HINTER: u32 = 1 << 11;
        const FT_FACE_FLAG_VARIATION: u32 = 1 << 15;

        if self.face_kind == FaceKind::Bdf {
            // FreeType's BDF driver exposes bitmap-only faces as horizontal
            // fixed-size strikes.  Pinned 2.14.3 reports face_flags=18 for
            // the maintained BDF macro control: FIXED_SIZES | HORIZONTAL.
            return FT_FACE_FLAG_FIXED_SIZES | FT_FACE_FLAG_HORIZONTAL;
        }

        if let FaceKind::WinFnt { header } = self.face_kind {
            // FreeType winfnt.c:fnt_size_select exposes fixed bitmap sizes
            // rather than scalable outlines for Windows FNT faces.
            let mut flags = FT_FACE_FLAG_FIXED_SIZES | FT_FACE_FLAG_HORIZONTAL;
            if header.pixel_width != 0 || header.avg_width == header.max_width {
                flags |= FT_FACE_FLAG_FIXED_WIDTH;
            }
            return flags;
        }

        if let FaceKind::Type1 { is_fixed_pitch } = self.face_kind {
            // FreeType `src/type1/t1objs.c:383-389` sets these flags for a
            // loaded Type 1 face, adding FIXED_WIDTH only for `isFixedPitch`.
            let mut flags = FT_FACE_FLAG_SCALABLE
                | FT_FACE_FLAG_HORIZONTAL
                | FT_FACE_FLAG_GLYPH_NAMES
                | FT_FACE_FLAG_HINTER;
            if self.type1_multi_master.is_some() {
                flags |= FT_FACE_FLAG_MULTIPLE_MASTERS;
            }
            if self.type1_mm_variation_active {
                flags |= FT_FACE_FLAG_VARIATION;
            }
            if is_fixed_pitch {
                flags |= FT_FACE_FLAG_FIXED_WIDTH;
            }
            return flags;
        }

        let mut flags = FT_FACE_FLAG_SCALABLE | FT_FACE_FLAG_SFNT | FT_FACE_FLAG_HORIZONTAL;
        if self
            .data
            .sbit
            .as_ref()
            .is_some_and(|sbit| sbit.strike_count() != 0)
        {
            flags |= FT_FACE_FLAG_FIXED_SIZES;
        }
        if self
            .data
            .post
            .as_ref()
            .is_some_and(|post| post.is_fixed_pitch != 0)
        {
            flags |= FT_FACE_FLAG_FIXED_WIDTH;
        }
        // CFF sets the glyph-name face flag in `src/cff/cffobjs.c:994-998`
        // for non-CID CFF faces independently of the SFNT `post` table.
        // Non-CFF SFNT faces use `sfobjs.c:1118-1121`, exposing glyph names
        // only if `tt_face_load_post` accepted a named `post` format.
        if (self.data.cff.is_some() && !self.is_cid_keyed())
            || self.data.post.as_ref().is_some_and(|post| {
                matches!(post.format_type, 0x0001_0000 | 0x0002_0000 | 0x0002_5000)
            })
        {
            flags |= FT_FACE_FLAG_GLYPH_NAMES;
        }
        if self.data.vhea.is_some() && self.data.vmtx.is_some() {
            flags |= FT_FACE_FLAG_VERTICAL;
        }
        if self.data.kern.as_ref().is_some_and(|kern| !kern.is_empty()) {
            flags |= FT_FACE_FLAG_KERNING;
        }
        // FreeType's CID service can report SFNT-wrapped CFF faces as
        // internally CID keyed while `FT_IS_CID_KEYED(face)` remains false
        // because the public `face_flags` bit is not set for this path in
        // pinned 2.14.3.
        // sfobjs.c:642-657 rejects zero-axis `fvar` tables before setting
        // TT_FACE_FLAG_VAR_FVAR; sfobjs.c:1141-1144 derives the public MM flag.
        if self
            .data
            .fvar
            .as_ref()
            .is_some_and(|fvar| fvar.axis_count != 0)
        {
            flags |= FT_FACE_FLAG_MULTIPLE_MASTERS;
        }
        // FreeType exposes FT_FACE_FLAG_VARIATION after explicit variation
        // coordinate selection, observed through ftmm.c's set-coordinate path.
        if self.data.variation_coordinates_set {
            flags |= FT_FACE_FLAG_VARIATION;
        }
        if self.data.table_directory.record(tag(b"glyf")).is_some() {
            flags |= FT_FACE_FLAG_HINTER;
        }
        flags
    }

    /// Approximate `FT_FaceRec::style_flags` from `head.macStyle`.
    pub fn style_flags(&self) -> u32 {
        const FT_STYLE_FLAG_ITALIC: u32 = 1 << 0;
        const FT_STYLE_FLAG_BOLD: u32 = 1 << 1;

        let mut flags = 0;
        if self.data.head.mac_style & 2 != 0 {
            flags |= FT_STYLE_FLAG_ITALIC;
        }
        if self.data.head.mac_style & 1 != 0 {
            flags |= FT_STYLE_FLAG_BOLD;
        }
        flags
    }

    /// Selected size metrics.
    pub fn size_metrics(&self) -> SizeMetrics {
        self.size_metrics
    }

    pub(crate) fn active_size_state(&self) -> ActiveSizeState {
        ActiveSizeState {
            size_pt: self.size_pt,
            size_metrics: self.size_metrics,
            face_globals: self.face_globals.clone(),
            bytecode_context: self.bytecode_context.clone(),
        }
    }

    pub(crate) fn activate_size_state(&mut self, state: &ActiveSizeState) {
        self.size_pt = state.size_pt;
        self.size_metrics = state.size_metrics;
        self.face_globals = state.face_globals.clone();
        self.bytecode_context = state.bytecode_context.clone();
        self.data.size_pt.set(self.size_pt);
        sync_active_size_metrics(&self.data, self.size_metrics);
    }

    pub(crate) fn reset_size_to_undefined(&mut self) {
        // C `FT_New_Memory_Face` creates an active size whose public metrics
        // remain zero until the first size request (`base/ftobjs.c`). The
        // high-level Rust constructor accepts a convenience point size, so
        // the FreeType FFI constructor resets that state after parsing.
        self.size_pt = 0.0;
        self.size_metrics = SizeMetrics {
            x_ppem: 0,
            y_ppem: 0,
            x_scale: 0,
            y_scale: 0,
            ascender: 0,
            descender: 0,
            height: 0,
            max_advance: 0,
            x_dpi: 0,
            y_dpi: 0,
            char_width: 0,
            char_height: 0,
        };
        self.data.size_pt.set(0.0);
        sync_active_size_metrics(&self.data, self.size_metrics);
        self.face_globals =
            crate::autohint::globals::FaceGlobals::new(self.data.clone(), self.is_italic);
        self.bytecode_context = BytecodeContextCache::default();
    }

    pub(crate) fn reset_probe_size_request_metrics(&mut self) {
        self.reset_size_to_undefined();
        self.size_metrics.x_scale = 1 << 16;
        self.size_metrics.y_scale = 1 << 16;
        sync_active_size_metrics(&self.data, self.size_metrics);
    }

    pub(crate) fn clone_with_load_mode(&self, load_mode: LoadMode) -> Self {
        let mut font = self.clone();
        font.load_mode = load_mode;
        font
    }

    /// Equivalent to `FT_Set_Char_Size`.
    pub fn set_char_size(&mut self, char_width: i32, char_height: i32, x_dpi: u32, y_dpi: u32) {
        let _ = self.try_set_char_size(char_width, char_height, x_dpi, y_dpi);
    }

    pub(crate) fn try_set_char_size(
        &mut self,
        char_width: i32,
        char_height: i32,
        x_dpi: u32,
        y_dpi: u32,
    ) -> Result<(), SizeRequestError> {
        let (width, height) = normalize_char_size_dimensions(char_width, char_height);
        let (x_dpi, y_dpi) = normalize_size_resolutions(x_dpi, y_dpi);
        let size_metrics =
            SizeMetrics::try_from_char_size(width, height, x_dpi, y_dpi, &self.data)?;
        self.size_pt = height as f32 / 64.0;
        self.size_metrics = size_metrics;
        self.data.size_pt.set(self.size_pt);
        sync_active_size_metrics(&self.data, self.size_metrics);
        self.face_globals =
            crate::autohint::globals::FaceGlobals::new(self.data.clone(), self.is_italic);
        // C keeps TrueType bytecode execution state on the active size object
        // (`ttobjs.c:tt_size_run_prep`).  A size request invalidates the
        // prepared CVT/prep state; reusing it keeps stale scale values.
        self.bytecode_context = BytecodeContextCache::default();
        Ok(())
    }

    /// Equivalent to `FT_Set_Pixel_Sizes`.
    pub fn set_pixel_sizes(&mut self, pixel_width: u32, pixel_height: u32) {
        let size_metrics = SizeMetrics::from_pixel_size(pixel_width, pixel_height, &self.data);
        self.size_pt = f32::from(size_metrics.y_ppem);
        self.size_metrics = size_metrics;
        self.data.size_pt.set(self.size_pt);
        sync_active_size_metrics(&self.data, self.size_metrics);
        self.face_globals =
            crate::autohint::globals::FaceGlobals::new(self.data.clone(), self.is_italic);
        // C keeps TrueType bytecode execution state on the active size object
        // (`ttobjs.c:tt_size_run_prep`).  A size request invalidates the
        // prepared CVT/prep state; reusing it keeps stale scale values.
        self.bytecode_context = BytecodeContextCache::default();
    }

    /// Equivalent to `FT_Request_Size` for scalable outline faces.
    pub fn request_size(&mut self, request: SizeRequest) -> Result<(), SizeRequestError> {
        self.size_metrics = SizeMetrics::from_size_request(request, &self.data)?;
        self.size_pt = f32::from(self.size_metrics.y_ppem);
        self.data.size_pt.set(self.size_pt);
        sync_active_size_metrics(&self.data, self.size_metrics);
        self.face_globals =
            crate::autohint::globals::FaceGlobals::new(self.data.clone(), self.is_italic);
        // `FT_Request_Size` invalidates the active size's prepared bytecode
        // state just like `FT_Set_Char_Size` and `FT_Set_Pixel_Sizes`.
        self.bytecode_context = BytecodeContextCache::default();
        Ok(())
    }

    pub fn select_size(&mut self, strike_index: usize) -> Result<(), SelectSizeError> {
        let sbit = self
            .data
            .sbit
            .as_ref()
            .filter(|sbit| sbit.strike_count() != 0)
            .ok_or(SelectSizeError::NoFixedSizes)?;
        let metrics = sbit
            .strike_metrics(strike_index)
            .ok_or(SelectSizeError::InvalidArgument)?;
        // FreeType `FT_Select_Size` dispatches TrueType scalable bitmap faces
        // through `tt_size_select` (`src/truetype/ttdriver.c:312-331`), which
        // stores the strike index then calls `FT_Select_Metrics`
        // (`src/base/ftobjs.c:3210-3236`) to rebuild scalable size metrics
        // from the strike ppem values.
        self.size_metrics = SizeMetrics::from_pixel_size(
            u32::from(metrics.x_ppem),
            u32::from(metrics.y_ppem),
            &self.data,
        );
        self.size_pt = f32::from(self.size_metrics.y_ppem);
        self.data.size_pt.set(self.size_pt);
        sync_active_size_metrics(&self.data, self.size_metrics);
        self.face_globals =
            crate::autohint::globals::FaceGlobals::new(self.data.clone(), self.is_italic);
        self.bytecode_context = BytecodeContextCache::default();
        Ok(())
    }

    /// Return all selectable charmaps.
    pub fn charmaps(&self) -> Vec<CharmapInfo> {
        self.data
            .cmap
            .charmaps
            .iter()
            .enumerate()
            .map(|(index, record)| CharmapInfo {
                index,
                platform_id: record.platform_id,
                encoding_id: record.encoding_id,
                format: record.format,
                language_id: record.language_id,
            })
            .collect()
    }

    /// Return the selected charmap.
    pub fn charmap(&self) -> Option<CharmapInfo> {
        self.charmaps().into_iter().nth(self.selected_charmap)
    }

    /// Equivalent to `FT_Get_Charmap_Index` for the active charmap.
    pub fn charmap_index(&self) -> Option<usize> {
        if self.selected_charmap < self.data.cmap.charmaps.len() {
            Some(self.selected_charmap)
        } else {
            None
        }
    }

    /// Equivalent to `FT_Select_Charmap` for platform/encoding pairs.
    pub fn select_charmap(&mut self, platform_id: u16, encoding_id: u16) -> Result<(), FontError> {
        let Some(index) = self.data.cmap.charmaps.iter().position(|record| {
            record.platform_id == platform_id && record.encoding_id == encoding_id
        }) else {
            return Err(FontError::InvalidFont(format!(
                "charmap {platform_id}/{encoding_id} not found"
            )));
        };
        self.selected_charmap = index;
        Ok(())
    }

    /// Equivalent to `FT_Select_Charmap(FT_ENCODING_UNICODE)`.
    pub fn select_unicode_charmap(&mut self) -> Result<(), FontError> {
        // C `find_unicode_charmap` (src/base/ftobjs.c:1372-1453) does not
        // fall back to the first charmap when no FT_ENCODING_UNICODE map exists.
        let Some(index) = default_unicode_charmap_index(&self.data.cmap) else {
            return Err(FontError::InvalidFont(
                "unicode charmap not found".to_string(),
            ));
        };
        self.selected_charmap = index;
        Ok(())
    }

    /// Equivalent to `FT_Set_Charmap` by index.
    pub fn set_charmap(&mut self, index: usize) -> Result<(), FontError> {
        if index >= self.data.cmap.charmaps.len() {
            return Err(FontError::InvalidFont(format!(
                "charmap index {index} out of range"
            )));
        }
        self.selected_charmap = index;
        Ok(())
    }

    /// Equivalent to `FT_Get_Char_Index`.
    pub fn char_index(&self, codepoint: u32) -> u16 {
        self.data
            .cmap
            .char_index_in_charmap(self.selected_charmap, codepoint)
            .unwrap_or(0)
    }

    /// Return the FreeType autofitter glyph style map for the active face.
    ///
    /// This is the face-owned `glyph_styles` payload exposed by
    /// `FT_Prop_GlyphToScriptMap`; values are internal autofitter style indexes
    /// plus flags, not public `FT_AUTOHINTER_SCRIPT_*` constants.
    pub fn autohint_glyph_style_map(&self) -> Vec<u16> {
        crate::autohint::globals::build_public_glyph_style_map(
            &self.data,
            self.data.maxp.num_glyphs,
        )
    }

    /// Equivalent to `FT_Face_GetCharVariantIndex`.
    pub fn char_variant_index(&self, codepoint: u32, variant_selector: u32) -> u16 {
        self.data
            .cmap
            .char_variant_index(self.selected_charmap, codepoint, variant_selector)
    }

    /// Equivalent to `FT_Face_GetCharVariantIsDefault`.
    pub fn char_variant_is_default(&self, codepoint: u32, variant_selector: u32) -> i32 {
        self.data
            .cmap
            .char_variant_is_default(codepoint, variant_selector)
    }

    /// Equivalent to `FT_Face_GetVariantSelectors`.
    pub fn variant_selectors(&self) -> Option<Vec<u32>> {
        self.data.cmap.variant_selectors()
    }

    /// Equivalent to `FT_Face_GetVariantsOfChar`.
    pub fn variants_of_char(&self, codepoint: u32) -> Option<Vec<u32>> {
        self.data.cmap.variants_of_char(codepoint)
    }

    /// Equivalent to `FT_Face_GetCharsOfVariant`.
    pub fn chars_of_variant(&self, variant_selector: u32) -> Option<Vec<u32>> {
        self.data.cmap.chars_of_variant(variant_selector)
    }

    /// Equivalent to `FT_Get_First_Char`.
    pub fn first_char(&self) -> Option<(u32, u16)> {
        self.data.cmap.first_char(self.selected_charmap)
    }

    /// Equivalent to `FT_Get_Next_Char`.
    pub fn next_char(&self, after: u32) -> Option<(u32, u16)> {
        self.data.cmap.next_char(self.selected_charmap, after)
    }

    /// Equivalent to `FT_Sfnt_Table_Info`.
    pub fn sfnt_table_info(&self, index: usize) -> Option<SfntTableInfo> {
        self.data
            .table_directory
            .records
            .get(index)
            .map(|record| SfntTableInfo {
                index,
                tag: record.tag,
                offset: record.offset,
                length: record.length,
            })
    }

    /// Iterate raw SFNT table descriptors.
    pub fn sfnt_tables(&self) -> Vec<SfntTableInfo> {
        self.data
            .table_directory
            .records
            .iter()
            .enumerate()
            .map(|(index, record)| SfntTableInfo {
                index,
                tag: record.tag,
                offset: record.offset,
                length: record.length,
            })
            .collect()
    }

    /// Equivalent to `FT_Load_Sfnt_Table`.
    pub fn load_sfnt_table(
        &self,
        tag: u32,
        offset: i64,
        length: Option<usize>,
    ) -> Result<Vec<u8>, FontError> {
        let (base, table_len) = self.sfnt_table_read_base_and_len(tag)?;
        let start = base
            .checked_add(offset)
            .ok_or_else(|| FontError::InvalidFont("SFNT table read offset overflows".into()))?;
        let start = usize::try_from(start)
            .map_err(|_| FontError::InvalidFont("SFNT table read offset before stream".into()))?;
        let read_len = length.unwrap_or(table_len);
        let end = start
            .checked_add(read_len)
            .ok_or_else(|| FontError::InvalidFont("SFNT table read length overflows".into()))?;
        self.data
            .raw_data
            .get(start..end)
            .map(|bytes| bytes.to_vec())
            .ok_or_else(|| FontError::InvalidFont("SFNT table read exceeds data".into()))
    }

    /// Size reported by `FT_Load_Sfnt_Table` when the caller passes `*length == 0`.
    pub fn sfnt_table_len(&self, tag: u32) -> Result<usize, FontError> {
        self.sfnt_table_read_base_and_len(tag)
            .map(|(_base, table_len)| table_len)
    }

    fn sfnt_table_read_base_and_len(&self, tag: u32) -> Result<(i64, usize), FontError> {
        if !self.is_sfnt() {
            return Err(FontError::InvalidFont("face is not SFNT".into()));
        }
        if tag == 0 {
            return Ok((0, self.data.raw_data.len()));
        }
        if tag == 1 {
            return Ok((
                self.data.face_offset as i64,
                12 + self.data.table_directory.records.len() * 16,
            ));
        }
        let record =
            self.data.table_directory.record(tag).ok_or_else(|| {
                FontError::InvalidFont(format!("SFNT table 0x{tag:08X} not found"))
            })?;
        Ok((i64::from(record.offset), record.length as usize))
    }

    /// `getname()` → `(family, style)`.
    pub fn getname(&self) -> (&str, &str) {
        (&self.family_name, &self.subfamily_name)
    }

    /// Return the face names after applying FreeType open-parameter name flags.
    pub fn getname_with_options(&self) -> (String, String) {
        (self.family_name.clone(), self.subfamily_name.clone())
    }

    /// Apply `FT_Open_Face` typographic-name ignore parameters.
    pub fn set_ignore_typographic_names(
        &mut self,
        ignore_typographic_family: bool,
        ignore_typographic_subfamily: bool,
    ) {
        // `sfnt_init_face` applies these `FT_Open_Face` parameters while
        // choosing `face->family_name` and `face->style_name`
        // (freetype/src/sfnt/sfobjs.c:829-843).  The parsed SFNT name table
        // stays shared and immutable; only this opened face's public names
        // change.
        let is_wws_only = self
            .data
            .os2
            .as_ref()
            .is_some_and(tt::os2::Os2Table::is_wws_only);
        self.family_name =
            tt::name::family_name(&self.data.name, ignore_typographic_family, is_wws_only);
        self.subfamily_name =
            tt::name::subfamily_name(&self.data.name, ignore_typographic_subfamily, is_wws_only);
    }

    /// `getmetrics()` → `(ascent, descent)` in pixels.
    ///
    /// Returns `face->size->metrics.ascender >> 6` and
    /// `-face->size->metrics.descender >> 6`, where the FreeType metrics are
    /// in 26.6 format after `FT_PIX_ROUND`. For the test fonts, this is
    /// equivalent to `ceil(|fu_val| * ppem / upem)`.
    pub fn getmetrics(&self) -> (u32, u32) {
        let data = &self.data;
        let upem = data.head.units_per_em as i32;
        let ppem = i32_from_f32(self.size_pt + 0.5); // FT_PIX_ROUND(size_pt << 6) >> 6

        let (asc_fu, desc_fu) = pick_metrics(data);
        // Match C's FT_PIX_CEIL(FT_MulFix(fu_val, scale)) chain exactly.
        // scale = FT_DivFix(ppem << 6, upem) in 16.16
        // val_26dot6 = FT_MulFix(fu_val, scale)
        // result = FT_PIX_CEIL(val_26dot6)
        let scale: i64 = ((ppem as i64 * 64 * 65536) + (upem as i64 / 2)) / upem as i64;
        let asc_26dot6 = (asc_fu as i64 * scale + 32768) >> 16;
        let desc_26dot6 = (desc_fu as i64 * scale + 32768) >> 16;
        let asc = u32_from_i64((asc_26dot6 + 63) >> 6);
        let desc = u32_from_i64((desc_26dot6 + 63) >> 6);
        (asc, desc)
    }

    /// `getlength(text)` -> total glyph-slot advance in pixels.
    ///
    /// FreeType does not apply pair kerning as part of `FT_Load_Glyph`; callers
    /// that need legacy `kern` table adjustment can use [`Self::getkerning`].
    pub fn getlength(&self, text: &str) -> Result<f32, FontError> {
        Ok(self.layout_advance(text)? as f32 / 64.0)
    }

    /// Return scaled legacy `kern` table adjustment for a Unicode pair in 26.6 pixels.
    pub fn getkerning(&self, left: char, right: char) -> i32 {
        let left = self.char_index(left as u32);
        let right = self.char_index(right as u32);
        self.glyph_kerning(left, right)
    }

    /// Return the scaled horizontal advance for one Unicode codepoint in 26.6 pixels.
    ///
    /// This exposes the fractional pen advance from the font's `hmtx` entry.
    /// It intentionally stays separate from [`Self::getlength`] and
    /// [`Self::glyph_metrics`], which report grid-fit FreeType metric parity.
    /// Kerning is not included; callers that build text runs should add
    /// [`Self::getkerning`] between adjacent glyphs.
    pub fn glyph_hori_advance_26dot6(&self, codepoint: u32) -> i32 {
        let glyph = self.char_index(codepoint);
        self.glyph_index_hori_advance_26dot6(glyph)
    }

    pub(crate) fn glyph_index_hori_advance_26dot6(&self, glyph_index: u16) -> i32 {
        let advance = self.data.load_glyph_outline(glyph_index).map_or_else(
            |_| self.data.hmtx.get(glyph_index).advance_width as i32,
            |outline| {
                self.data
                    .hmtx_hori_advance_with_gvar_delta_or_hmtx(glyph_index, outline.points.len())
            },
        );
        ft_mul_fix(advance, self.size_metrics.x_scale)
    }

    fn glyph_index_vert_advance_26dot6(&self, glyph_index: u16) -> i32 {
        let advance = if let Some(vmtx) = &self.data.vmtx {
            i32::from(vmtx.get(glyph_index).advance_height)
        } else {
            vertical_advance_font_units(&self.data)
        };
        ft_mul_fix(advance, self.size_metrics.y_scale)
    }

    pub(crate) fn glyph_index_hori_advance_16dot16(&self, glyph_index: u16) -> i32 {
        let advance = self.data.load_glyph_outline(glyph_index).map_or_else(
            |_| self.data.hmtx.get(glyph_index).advance_width as i32,
            |outline| {
                self.data
                    .hmtx_hori_advance_with_gvar_delta_or_hmtx(glyph_index, outline.points.len())
            },
        );
        ft_mul_fix(advance * 1024, self.size_metrics.x_scale)
    }

    /// Return `FT_GlyphSlotRec::metrics` for a Unicode codepoint loaded with
    /// FreeType's default TrueType load path.
    ///
    /// This is the scalar metrics path used before rendering: native bytecode
    /// hinting is allowed, autohinting is not forced, and no bitmap render is
    /// requested.
    pub fn glyph_metrics(&self, codepoint: u32) -> Result<GlyphSlotMetrics, FontError> {
        let glyph = self.char_index(codepoint);
        self.glyph_metrics_for_index_default(glyph)
    }

    pub(crate) fn glyph_metrics_for_index_default(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        self.glyph_metrics_for_index_default_with_layout(glyph, false)
    }

    pub(crate) fn glyph_metrics_for_index_default_with_layout(
        &self,
        glyph: u16,
        vertical_layout: bool,
    ) -> Result<GlyphSlotMetrics, FontError> {
        self.glyph_metrics_for_index_default_with_layout_and_mode(
            glyph,
            vertical_layout,
            NativeHintMode::Normal,
        )
    }

    pub(crate) fn glyph_metrics_for_index_default_with_layout_and_mode(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
    ) -> Result<GlyphSlotMetrics, FontError> {
        Ok(self
            .glyph_slot_load_default_with_layout_and_mode(glyph, vertical_layout, native_hint_mode)?
            .metrics)
    }

    pub(crate) fn glyph_slot_load_default_with_layout_and_mode(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
    ) -> Result<GlyphSlotLoad, FontError> {
        self.glyph_slot_load_default_with_layout_and_mode_and_hdmx(
            glyph,
            vertical_layout,
            native_hint_mode,
            true,
        )
    }

    pub(crate) fn glyph_slot_load_default_with_layout_and_mode_and_hdmx(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
        use_hdmx: bool,
    ) -> Result<GlyphSlotLoad, FontError> {
        self.glyph_slot_load_default_with_layout_and_mode_and_hdmx_and_pedantic(
            glyph,
            vertical_layout,
            native_hint_mode,
            use_hdmx,
            false,
        )
    }

    pub(crate) fn glyph_slot_load_default_with_layout_and_mode_and_hdmx_and_pedantic(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
        use_hdmx: bool,
        pedantic_hinting: bool,
    ) -> Result<GlyphSlotLoad, FontError> {
        if self.is_type1_face() {
            return self.glyph_slot_load_type1_scaled(
                glyph,
                vertical_layout,
                MetricsGridFit::Horizontal,
            );
        }
        let scaled = self.scale_glyph_for_metrics_default_with_mode_and_hdmx_and_pedantic(
            glyph,
            native_hint_mode,
            use_hdmx,
            pedantic_hinting,
        )?;
        Ok(self.slot_load_from_scaled(glyph, scaled, grid_fit_for_layout(vertical_layout)))
    }

    pub(crate) fn load_sbit_only_glyph(
        &self,
        glyph_index: u16,
    ) -> Result<tt::sbit::SbitGlyph, FontError> {
        let sbit = self.data.sbit.as_ref().ok_or_else(|| {
            FontError::InvalidArgument("embedded bitmap strike not selected".into())
        })?;
        let metrics = self.size_metrics();
        let mut sbit_glyph = sbit.load_glyph(glyph_index, metrics.x_ppem, metrics.y_ppem, 0)?;
        if sbit.kind() == tt::sbit::SbitTableKind::Eblc {
            // FreeType `truetype/ttgload.c:2401-2469` fills missing scalable
            // EBLC/bloc SBIT advances from the glyph's linear TrueType advances
            // after `load_sbit_image`; CBLC/CBDT color bitmap loads keep the
            // zero advances reported by `sfnt/ttsbit.c`.
            if sbit_glyph.metrics.hori_advance == 0 {
                sbit_glyph.metrics.hori_advance = self.glyph_index_hori_advance_26dot6(glyph_index);
            }
            if sbit_glyph.metrics.vert_advance == 0 {
                sbit_glyph.metrics.vert_advance = self.glyph_index_vert_advance_26dot6(glyph_index);
            }
        }
        Ok(sbit_glyph)
    }

    pub(crate) fn glyph_slot_load_force_autohint_with_layout_and_mode(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
    ) -> Result<GlyphSlotLoad, FontError> {
        let metrics_cache = self.autohint_metrics_for_glyph_checked(glyph)?;
        let scaled = scaler::scale_glyph_for_metrics_with_autohint_and_mode(
            &self.data,
            glyph,
            metrics_cache.as_deref(),
            self.is_italic,
            native_hint_mode,
        )?;
        Ok(self.slot_load_from_scaled(glyph, scaled, grid_fit_for_layout(vertical_layout)))
    }

    pub(crate) fn glyph_slot_load_target_light(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotLoad, FontError> {
        let metrics_cache = self.autohint_metrics_for_glyph_checked(glyph)?;
        let scaled = scaler::scale_glyph_for_metrics_light(
            &self.data,
            glyph,
            metrics_cache.as_deref(),
            self.is_italic,
        )?;
        // C target-light keeps the light horizontal metric box even when
        // FT_LOAD_VERTICAL_LAYOUT is set; only the slot advance vector changes.
        Ok(self.slot_load_from_scaled(glyph, scaled, MetricsGridFit::Horizontal))
    }

    pub(crate) fn glyph_slot_load_no_hinting(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotLoad, FontError> {
        if self.is_type1_face() {
            return self.glyph_slot_load_type1_scaled(glyph, false, MetricsGridFit::None);
        }
        let scaled = scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)?;
        // C: `FT_Load_Glyph` calls `ft_glyphslot_grid_fit_metrics` only when
        // `FT_LOAD_NO_HINTING` is not set (`src/base/ftobjs.c`).  No-hinting
        // slot metrics keep the fractional 26.6 values from `ttgload.c`.
        Ok(self.slot_load_from_scaled(glyph, scaled, MetricsGridFit::None))
    }

    pub(crate) fn glyph_slot_load_no_scale(&self, glyph: u16) -> Result<GlyphSlotLoad, FontError> {
        self.glyph_slot_load_no_scale_with_layout(glyph, false)
    }

    pub(crate) fn glyph_slot_load_no_scale_with_layout(
        &self,
        glyph: u16,
        vertical_layout: bool,
    ) -> Result<GlyphSlotLoad, FontError> {
        if self.is_type1_face() {
            return self.glyph_slot_load_type1_no_scale(glyph, vertical_layout);
        }
        if self.data.cff.is_some() {
            return self.glyph_slot_load_cff_no_scale(glyph, vertical_layout);
        }
        self.glyph_slot_load_truetype_no_scale(glyph)
    }

    fn glyph_slot_load_truetype_no_scale(&self, glyph: u16) -> Result<GlyphSlotLoad, FontError> {
        let outline = tt::glyf::load_glyph(
            &self.data.glyf_data,
            &self.data.loca_data,
            self.data.head.index_to_loc_format,
            glyph,
            &self.data.hmtx,
        )?;
        let subglyphs = subglyphs_from_components(&outline.components);
        let h_metric = self.data.hmtx.get(glyph);
        let hori_advance = h_metric.advance_width as i32;
        if outline.num_contours == 0 || outline.points.is_empty() {
            let mut metrics = GlyphSlotMetrics {
                width: 0,
                height: 0,
                hori_bearing_x: 0,
                hori_bearing_y: 0,
                hori_advance,
                vert_bearing_x: -(hori_advance / 2),
                vert_bearing_y: 0,
                vert_advance: 0,
            };
            self.fill_no_scale_vertical_metrics(glyph, &outline, &mut metrics);
            let outline_cbox = BBox {
                x_min: 0,
                y_min: 0,
                x_max: 0,
                y_max: 0,
            };
            return Ok(GlyphSlotLoad {
                metrics,
                format: GlyphSlotLoadFormat::Outline,
                outline_cbox,
                outline_bbox: outline_cbox,
                subglyphs,
                slot_outline: Some(Outline::default()),
                render_outline: Some(LoadedOutline {
                    outline: Outline::default(),
                    left: 0,
                    bottom: 0,
                    top: 0,
                }),
            });
        }

        // C: normal recursive `FT_LOAD_NO_SCALE` leaves the slot format as an
        // outline, translates it by `-pp1.x`, then `compute_glyph_metrics`
        // calls `FT_Outline_Get_CBox` (`src/truetype/ttgload.c`).  The
        // composite-header bbox is used only for unrecurred composite slots
        // (`FT_LOAD_NO_RECURSE`), which this core path does not model.
        let pp1x = outline.bbox_xmin - h_metric.lsb as i32;
        let mut x_min = outline.points[0].x - pp1x;
        let mut y_min = outline.points[0].y;
        let mut x_max = x_min;
        let mut y_max = y_min;
        for point in &outline.points[1..] {
            let x = point.x - pp1x;
            x_min = x_min.min(x);
            y_min = y_min.min(point.y);
            x_max = x_max.max(x);
            y_max = y_max.max(point.y);
        }

        let mut metrics = GlyphSlotMetrics {
            width: x_max - x_min,
            height: y_max - y_min,
            hori_bearing_x: x_min,
            hori_bearing_y: y_max,
            hori_advance,
            vert_bearing_x: 0,
            vert_bearing_y: 0,
            vert_advance: 0,
        };
        self.fill_no_scale_vertical_metrics(glyph, &outline, &mut metrics);
        let outline_cbox = BBox {
            x_min,
            y_min,
            x_max,
            y_max,
        };
        let slot_outline = no_scale_slot_outline(&outline, pp1x, outline_cbox);
        let render_outline = no_scale_render_outline(&outline, pp1x, outline_cbox);
        Ok(GlyphSlotLoad {
            metrics,
            format: GlyphSlotLoadFormat::Outline,
            outline_cbox,
            outline_bbox: outline_cbox,
            subglyphs,
            slot_outline: Some(slot_outline),
            render_outline: Some(render_outline),
        })
    }

    fn glyph_slot_load_cff_no_scale(
        &self,
        glyph: u16,
        vertical_layout: bool,
    ) -> Result<GlyphSlotLoad, FontError> {
        // C `cff_slot_load` keeps Type2 coordinates and metrics in design
        // units for `FT_LOAD_NO_SCALE`; it uses identity glyph scales rather
        // than the active size metrics (`src/cff/cffgload.c:411-428`).
        let mut outline = self.data.load_glyph_outline(glyph)?.as_ref().clone();
        outline.outline_flags |= crate::outline::OUTLINE_REVERSE_FILL;
        if self.size_metrics.y_ppem < 24 {
            outline.outline_flags |= crate::outline::OUTLINE_HIGH_PRECISION;
        }

        let outline_cbox = BBox {
            x_min: outline.xmin,
            y_min: outline.ymin,
            x_max: outline.xmax,
            y_max: outline.ymax,
        };
        let h_metric = self.data.hmtx.get(glyph);
        let mut metrics = GlyphSlotMetrics {
            width: outline_cbox.x_max - outline_cbox.x_min,
            height: outline_cbox.y_max - outline_cbox.y_min,
            hori_bearing_x: outline_cbox.x_min,
            hori_bearing_y: outline_cbox.y_max,
            hori_advance: i32::from(h_metric.advance_width),
            vert_bearing_x: 0,
            vert_bearing_y: 0,
            vert_advance: 0,
        };

        if let Some(vmtx) = &self.data.vmtx {
            let vertical = vmtx.get(glyph);
            metrics.vert_bearing_x = metrics.hori_bearing_x - metrics.hori_advance / 2;
            metrics.vert_bearing_y = i32::from(vertical.tsb);
            metrics.vert_advance = i32::from(vertical.advance_height);
        } else {
            metrics.vert_advance = vertical_advance_font_units(&self.data);
            if vertical_layout {
                synthesize_vertical_metrics(&mut metrics);
            }
        }

        let slot_outline = no_scale_slot_outline(&outline, 0, outline_cbox);
        let render_outline = no_scale_render_outline(&outline, 0, outline_cbox);
        Ok(GlyphSlotLoad {
            metrics,
            format: GlyphSlotLoadFormat::Outline,
            outline_cbox,
            outline_bbox: outline_cbox,
            subglyphs: Vec::new(),
            slot_outline: Some(slot_outline),
            render_outline: Some(render_outline),
        })
    }

    pub(crate) fn glyph_slot_load_no_recurse(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotLoad, FontError> {
        let mut loaded = self.glyph_slot_load_no_scale(glyph)?;
        if self.glyph_is_composite(glyph)? {
            let outline = tt::glyf::load_glyph(
                &self.data.glyf_data,
                &self.data.loca_data,
                self.data.head.index_to_loc_format,
                glyph,
                &self.data.hmtx,
            )?;
            let x_min = outline.bbox_xmin;
            let y_min = outline.ymin;
            let x_max = outline.xmax;
            let y_max = outline.ymax;
            let h_metric = self.data.hmtx.get(glyph);
            let mut metrics = GlyphSlotMetrics {
                width: x_max - x_min,
                height: y_max - y_min,
                hori_bearing_x: x_min,
                hori_bearing_y: y_max,
                hori_advance: h_metric.advance_width as i32,
                vert_bearing_x: 0,
                vert_bearing_y: 0,
                vert_advance: 0,
            };
            self.fill_no_scale_vertical_metrics(glyph, &outline, &mut metrics);
            let outline_cbox = BBox {
                x_min,
                y_min,
                x_max,
                y_max,
            };
            // C: FT_LOAD_NO_RECURSE leaves composite glyphs in
            // FT_GLYPH_FORMAT_COMPOSITE instead of resolving them to an
            // outline and computes metrics from the composite glyph header
            // bbox (`src/truetype/ttgload.c`).  Renderers then reject the slot
            // with Cannot_Render_Glyph.
            loaded.metrics = metrics;
            loaded.outline_cbox = outline_cbox;
            loaded.outline_bbox = outline_cbox;
            loaded.format = GlyphSlotLoadFormat::Composite;
            loaded.slot_outline = None;
            loaded.render_outline = None;
        }
        Ok(loaded)
    }

    pub(crate) fn glyph_slot_load_no_autohint_with_layout_and_mode_and_pedantic(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
        pedantic_hinting: bool,
    ) -> Result<GlyphSlotLoad, FontError> {
        if self.is_type1_face() {
            return self.glyph_slot_load_type1_scaled(
                glyph,
                vertical_layout,
                MetricsGridFit::Horizontal,
            );
        }
        let scaled = self.scale_glyph_no_autohint_for_metrics_with_mode_and_pedantic(
            glyph,
            native_hint_mode,
            pedantic_hinting,
        )?;
        Ok(self.slot_load_from_scaled(glyph, scaled, grid_fit_for_layout(vertical_layout)))
    }

    /// `getbbox(text)` -> FreeType rendered bitmap bbox for the first glyph.
    ///
    /// Returns the rendered glyph-slot bitmap box for the first glyph.
    pub fn getbbox(&self, text: &str) -> (i32, i32, i32, i32) {
        if text.is_empty() {
            return (0, 0, 0, 0);
        }
        self.getbbox_single_glyph(text)
    }

    /// `getmask(text)` -> FreeType-style 8-bit alpha bitmap for the first glyph,
    /// with no text-run composition or adapter-specific layout.
    ///
    /// # Errors
    ///
    /// Returns [`FontError::InvalidFont`] if the glyph outline cannot be
    /// loaded or scaled, or [`FontError::InvalidOutline`] if the outline
    /// data is malformed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use fontdone::Font;
    /// let font_data = std::fs::read("DejaVuSans.ttf").unwrap();
    /// let font = Font::truetype(&font_data, 10.0).unwrap();
    /// let mask = font.getmask("A").unwrap();
    /// assert!(mask.width > 0);
    /// ```
    pub fn getmask(&self, text: &str) -> Result<GlyphMask, FontError> {
        if text.is_empty() {
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: Vec::new(),
                xmin: 0,
                ymin: 0,
                advance_width: 0,
            });
        }
        self.getmask_single_glyph(text)
    }
}

impl Font {
    fn autohint_metrics_for_glyph_checked(
        &self,
        glyph: u16,
    ) -> Result<Option<Rc<crate::autohint::AfLatinMetrics>>, FontError> {
        let metrics = self.autohint_metrics_for_glyph(glyph);
        if glyph >= self.data.maxp.num_glyphs {
            // C `af_loader_load_glyph` asks `af_face_globals_get_metrics`
            // before its recursive driver load; the globals guard therefore
            // returns Invalid_Argument for explicit autohint loads.
            return Err(FontError::InvalidArgument(
                "autohinter glyph index out of range".into(),
            ));
        }
        Ok(metrics)
    }

    fn autohint_metrics_for_glyph(
        &self,
        glyph: u16,
    ) -> Option<Rc<crate::autohint::AfLatinMetrics>> {
        if glyph == 0 {
            // C: afglobal.c assigns cmap-uncovered glyphs, including `.notdef`,
            // to the module fallback style before afloader.c requests metrics.
            self.face_globals.get_fallback_metrics()
        } else {
            self.face_globals.get_metrics(glyph)
        }
    }

    fn getbbox_single_glyph(&self, text: &str) -> (i32, i32, i32, i32) {
        let ch = text.chars().next().unwrap_or('\0');
        let glyph = self.char_index(ch as u32);
        match self.scale_glyph_for_load_mode(glyph) {
            Ok(g) if g.outline.n_contours > 0 => {
                // Raw FreeType bbox: pixel coords from outline, y-up from baseline.
                (g.bbox_x_min, g.bbox_y_min, g.bbox_x_max, g.bbox_y_max)
            }
            _ => (0, 0, 0, 0),
        }
    }

    fn getmask_single_glyph(&self, text: &str) -> Result<GlyphMask, FontError> {
        let ch = text.chars().next().unwrap_or('\0');
        let glyph = self.char_index(ch as u32);
        let scaled = self.scale_glyph_for_load_mode(glyph)?;

        let outline = scaled.outline;
        if outline.points.is_empty() || outline.n_contours == 0 {
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: Vec::new(),
                xmin: 0,
                ymin: 0,
                advance_width: 0,
            });
        }
        let width = usize_from_i32(outline.cbox_x_max - outline.cbox_x_min);
        let height = usize_from_i32(outline.cbox_y_max - outline.cbox_y_min);
        if width == 0 || height == 0 {
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: Vec::new(),
                xmin: scaled.bbox_x_min,
                ymin: scaled.bbox_y_min,
                advance_width: pixel_round(scaled.advance_width),
            });
        }
        let mut target = vec![0u8; width * height];
        let mut scratch = self.raster_scratch.borrow_mut();
        crate::grays::rasterize_shifted_in_box_to_with_scratch(
            &outline,
            0,
            0,
            width,
            height,
            &mut target,
            width,
            1,
            0,
            outline.cbox_x_min,
            outline.cbox_x_max,
            outline.cbox_y_min,
            outline.cbox_y_max,
            &mut scratch,
        )?;
        drop(scratch);
        Ok(GlyphMask {
            width: u32_from_usize(width),
            height: u32_from_usize(height),
            pixels: target,
            xmin: scaled.bbox_x_min,
            ymin: scaled.bbox_y_min,
            advance_width: pixel_round(scaled.advance_width),
        })
    }

    pub(crate) fn scale_glyph_for_load_mode(
        &self,
        glyph: u16,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        self.scale_glyph_for_load_mode_with_native_mode(glyph, NativeHintMode::Normal)
    }

    pub(crate) fn scale_glyph_for_load_mode_with_native_mode(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        match self.load_mode {
            LoadMode::Default => {
                let bytecode_context = self.native_bytecode_context_for_mode(native_hint_mode)?;
                if native_hint_mode == NativeHintMode::Normal {
                    if bytecode_context.is_none() {
                        scaler::scale_glyph_native_default(&self.data, glyph, None, self.is_italic)
                    } else {
                        scaler::scale_glyph_native_default_with_bytecode_context(
                            &self.data,
                            glyph,
                            None,
                            self.is_italic,
                            bytecode_context,
                        )
                    }
                } else {
                    scaler::scale_glyph_native_default_with_bytecode_context_and_mode(
                        &self.data,
                        glyph,
                        None,
                        self.is_italic,
                        native_hint_mode,
                        bytecode_context,
                    )
                }
            }
            LoadMode::ForceAutoHint => {
                let metrics_cache = self.autohint_metrics_for_glyph(glyph);
                match native_hint_mode {
                    NativeHintMode::Normal => scaler::scale_glyph(
                        &self.data,
                        glyph,
                        metrics_cache.as_deref(),
                        self.is_italic,
                    ),
                    NativeHintMode::Mono => scaler::scale_glyph_mono(
                        &self.data,
                        glyph,
                        metrics_cache.as_deref(),
                        self.is_italic,
                    ),
                    NativeHintMode::Lcd => scaler::scale_glyph_lcd(
                        &self.data,
                        glyph,
                        metrics_cache.as_deref(),
                        self.is_italic,
                    ),
                    NativeHintMode::LcdV => scaler::scale_glyph_lcd_v(
                        &self.data,
                        glyph,
                        metrics_cache.as_deref(),
                        self.is_italic,
                    ),
                }
            }
            LoadMode::TargetLight => {
                let metrics_cache = self.autohint_metrics_for_glyph(glyph);
                scaler::scale_glyph_light(
                    &self.data,
                    glyph,
                    metrics_cache.as_deref(),
                    self.is_italic,
                )
            }
            LoadMode::NoHinting => {
                scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)
            }
            LoadMode::NoAutoHint => {
                self.scale_glyph_no_autohint_for_load_with_mode(glyph, native_hint_mode)
            }
        }
    }

    fn scale_glyph_for_metrics_default_with_mode_and_hdmx_and_pedantic(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
        use_hdmx: bool,
        pedantic_hinting: bool,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if let (Some(fpgm), Some(cvt)) = (&self.data.fpgm, &self.data.cvt) {
            let mut owned_context = None;
            let bytecode_context = self.bytecode_context_for_mode_with_pedantic(
                native_hint_mode,
                pedantic_hinting,
                cvt,
                fpgm,
                &mut owned_context,
            )?;
            let scaled = scaler::scale_glyph_for_metrics_with_bytecode_context_and_mode_and_hdmx(
                &self.data,
                glyph,
                self.is_italic,
                native_hint_mode,
                bytecode_context,
                use_hdmx,
            )?;
            if is_pathological_metrics_cbox(&scaled) || is_pathological_metrics_advance(&scaled) {
                let metrics_cache = self.face_globals.get_metrics(glyph);
                scaler::scale_glyph_for_metrics_with_autohint_preserve_advance(
                    &self.data,
                    glyph,
                    metrics_cache.as_deref(),
                    self.is_italic,
                )
            } else {
                Ok(scaled)
            }
        } else {
            // C `FT_Load_Glyph` only falls back to the auto-hinter for SFNT
            // TrueType faces with no font program and a tiny `prep` program
            // (`src/base/ftobjs.c:966-989`).  Longer prep-only programs still
            // route through the TrueType driver.
            if scaler::should_use_default_autohint(&self.data) {
                let metrics_cache = self.face_globals.get_metrics(glyph);
                // FreeType `FT_Load_Glyph` carries the requested load target
                // into its default auto-hinter fallback (`ftobjs.c`).
                // In particular, `FT_LOAD_TARGET_MONO` snaps advances and
                // stems differently from the normal grayscale target.
                scaler::scale_glyph_for_metrics_with_autohint_and_mode(
                    &self.data,
                    glyph,
                    metrics_cache.as_deref(),
                    self.is_italic,
                    native_hint_mode,
                )
            } else {
                scaler::scale_glyph_for_metrics(&self.data, glyph, self.is_italic)
            }
        }
    }

    fn scale_glyph_no_autohint_for_load_with_mode(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        self.scale_glyph_no_autohint_for_load_with_mode_and_pedantic(glyph, native_hint_mode, false)
    }

    fn scale_glyph_no_autohint_for_load_with_mode_and_pedantic(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
        pedantic_hinting: bool,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if let (Some(fpgm), Some(cvt)) = (&self.data.fpgm, &self.data.cvt) {
            let mut owned_context = None;
            let bytecode_context = self.bytecode_context_for_mode_with_pedantic(
                native_hint_mode,
                pedantic_hinting,
                cvt,
                fpgm,
                &mut owned_context,
            )?;
            scaler::scale_glyph_native_default_with_bytecode_context_and_mode(
                &self.data,
                glyph,
                None,
                self.is_italic,
                native_hint_mode,
                bytecode_context,
            )
        } else {
            scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)
        }
    }

    fn scale_glyph_no_autohint_for_metrics_with_mode_and_pedantic(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
        pedantic_hinting: bool,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if let (Some(fpgm), Some(cvt)) = (&self.data.fpgm, &self.data.cvt) {
            let mut owned_context = None;
            let bytecode_context = self.bytecode_context_for_mode_with_pedantic(
                native_hint_mode,
                pedantic_hinting,
                cvt,
                fpgm,
                &mut owned_context,
            )?;
            if native_hint_mode == NativeHintMode::Normal {
                scaler::scale_glyph_for_metrics_with_bytecode_context(
                    &self.data,
                    glyph,
                    self.is_italic,
                    bytecode_context,
                )
            } else {
                scaler::scale_glyph_for_metrics_with_bytecode_context_and_mode(
                    &self.data,
                    glyph,
                    self.is_italic,
                    native_hint_mode,
                    bytecode_context,
                )
            }
        } else {
            scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)
        }
    }

    fn native_bytecode_context(&self) -> Result<Option<&tt::hinter::exec::ExecContext>, FontError> {
        self.native_bytecode_context_for_mode(NativeHintMode::Normal)
    }

    fn native_bytecode_context_for_mode(
        &self,
        mode: NativeHintMode,
    ) -> Result<Option<&tt::hinter::exec::ExecContext>, FontError> {
        let (Some(fpgm), Some(cvt)) = (&self.data.fpgm, &self.data.cvt) else {
            return Ok(None);
        };
        let slot = self.bytecode_context.slot(mode);
        if slot.get().is_none() {
            let prepared = self.prepare_native_bytecode_context_for_mode(mode, false, cvt, fpgm)?;
            let _ = slot.set(prepared);
        }
        Ok(slot.get())
    }

    fn bytecode_context_for_mode_with_pedantic<'a>(
        &'a self,
        mode: NativeHintMode,
        pedantic_hinting: bool,
        cvt: &[i32],
        fpgm: &[u8],
        owned_context: &'a mut Option<tt::hinter::exec::ExecContext>,
    ) -> Result<Option<&'a tt::hinter::exec::ExecContext>, FontError> {
        if pedantic_hinting {
            *owned_context =
                Some(self.prepare_native_bytecode_context_for_mode(mode, true, cvt, fpgm)?);
            Ok(owned_context.as_ref())
        } else {
            self.native_bytecode_context_for_mode(mode)
        }
    }

    fn prepare_native_bytecode_context_for_mode(
        &self,
        mode: NativeHintMode,
        pedantic_hinting: bool,
        cvt: &[i32],
        fpgm: &[u8],
    ) -> Result<tt::hinter::exec::ExecContext, FontError> {
        let active_scale = scaler::ScaleMetrics::from_font_data(&self.data);
        scaler::prepare_native_bytecode_context(
            &self.data,
            active_scale,
            mode,
            pedantic_hinting,
            cvt,
            fpgm,
        )
    }

    fn layout_advance(&self, text: &str) -> Result<i32, FontError> {
        text.chars().try_fold(0, |total, ch| {
            let glyph = self.char_index(ch as u32);
            if glyph == 0 {
                return Ok(total);
            }
            Ok(total + self.glyph_metrics_for_index_default(glyph)?.hori_advance)
        })
    }

    fn glyph_kerning(&self, left: u16, right: u16) -> i32 {
        self.data.kern.as_ref().map_or(0, |kern| {
            let value = i32::from(kern.get(left, right));
            ft_mul_fix(value, self.size_metrics.x_scale)
        })
    }

    fn slot_load_from_scaled(
        &self,
        glyph_index: u16,
        scaled: scaler::ScaledGlyph,
        grid_fit_metrics: MetricsGridFit,
    ) -> GlyphSlotLoad {
        // Destructure to move `outline` while keeping field access.
        let scaler::ScaledGlyph {
            outline,
            bbox_x_min,
            bbox_y_min,
            bbox_y_max,
            outline_cbox_x_min,
            outline_cbox_y_min,
            outline_cbox_x_max,
            outline_cbox_y_max,
            outline_bbox_x_min,
            outline_bbox_y_min,
            outline_bbox_x_max,
            outline_bbox_y_max,
            cbox_x_min,
            cbox_y_min,
            cbox_x_max,
            cbox_y_max,
            slot_advance_width,
            vertical_bearing_x_advance_width,
            autohint_vertical,
            native_vertical,
            ..
        } = scaled;
        let slot_outline = scaled_slot_outline_from_outline(
            &outline,
            outline_cbox_x_min,
            outline_cbox_y_min,
            outline_cbox_x_max,
            outline_cbox_y_max,
        );
        let mut metrics = GlyphSlotMetrics {
            width: cbox_x_max - cbox_x_min,
            height: cbox_y_max - cbox_y_min,
            hori_bearing_x: cbox_x_min,
            hori_bearing_y: cbox_y_max,
            hori_advance: slot_advance_width,
            vert_bearing_x: 0,
            vert_bearing_y: 0,
            vert_advance: 0,
        };

        if let Some(vertical) = autohint_vertical {
            metrics.vert_bearing_x = vertical.bearing_x;
            metrics.vert_bearing_y = vertical.bearing_y;
            metrics.vert_advance = vertical.advance;
        } else if let Some(vertical) = native_vertical {
            metrics.vert_bearing_y = vertical.bearing_y;
            metrics.vert_advance = vertical.advance;
        } else if let Some(vmtx) = &self.data.vmtx {
            let vertical = vmtx.get(glyph_index);
            metrics.vert_bearing_y = ft_mul_fix(vertical.tsb as i32, self.size_metrics.y_scale);
            metrics.vert_advance =
                ft_mul_fix(vertical.advance_height as i32, self.size_metrics.y_scale);
        } else if self.data.cff.is_some() {
            // CFF keeps made-up vertical metrics mostly inert for horizontal
            // loads without `vmtx`; `cff_slot_load` only synthesizes bearings
            // on `FT_LOAD_VERTICAL_LAYOUT` (`src/cff/cffgload.c:646-742`).
            metrics.vert_advance = ft_mul_fix(
                vertical_advance_font_units(&self.data),
                self.size_metrics.y_scale,
            );
        } else {
            let height_fu = if self.size_metrics.y_scale == 0 {
                0
            } else {
                ft_div_fix(metrics.height, self.size_metrics.y_scale)
            };
            let advance_fu = vertical_advance_font_units(&self.data);
            let top_fu = (advance_fu - height_fu) / 2;
            metrics.vert_bearing_y = ft_mul_fix(top_fu, self.size_metrics.y_scale);
            metrics.vert_advance = ft_mul_fix(advance_fu, self.size_metrics.y_scale);
        }
        if autohint_vertical.is_none()
            && (self.data.cff.is_none()
                || self.data.vmtx.is_some()
                || matches!(grid_fit_metrics, MetricsGridFit::Vertical))
        {
            metrics.vert_bearing_x = metrics.hori_bearing_x - vertical_bearing_x_advance_width / 2;
        }

        match grid_fit_metrics {
            MetricsGridFit::None => {}
            MetricsGridFit::Horizontal => grid_fit_horizontal_metrics(&mut metrics),
            MetricsGridFit::Vertical => grid_fit_vertical_metrics(&mut metrics),
        }
        GlyphSlotLoad {
            metrics,
            format: GlyphSlotLoadFormat::Outline,
            outline_cbox: BBox {
                x_min: outline_cbox_x_min,
                y_min: outline_cbox_y_min,
                x_max: outline_cbox_x_max,
                y_max: outline_cbox_y_max,
            },
            outline_bbox: BBox {
                x_min: outline_bbox_x_min,
                y_min: outline_bbox_y_min,
                x_max: outline_bbox_x_max,
                y_max: outline_bbox_y_max,
            },
            subglyphs: Vec::new(),
            slot_outline: Some(slot_outline),
            render_outline: Some(LoadedOutline {
                outline,
                left: bbox_x_min,
                bottom: bbox_y_min,
                top: bbox_y_max,
            }),
        }
    }

    fn is_type1_face(&self) -> bool {
        matches!(self.face_kind, FaceKind::Type1 { .. })
    }

    fn type1_glyph_program(&self, glyph: u16) -> Result<Type1GlyphProgram, FontError> {
        let charstring = self
            .type1_charstrings
            .get(usize::from(glyph))
            .ok_or_else(|| FontError::InvalidArgument("Type 1 glyph index out of range".into()))?;
        // C Type 1 `T1_Load_Glyph` decodes the selected CharString after
        // `FT_Load_Glyph` has accepted the public glyph index.  Keep this in
        // core so Rust FFI, C ABI, and WASM expose the same parsed outline.
        parse_type1_glyph_program(&charstring.encrypted)
    }

    fn glyph_slot_load_type1_scaled(
        &self,
        glyph: u16,
        _vertical_layout: bool,
        grid_fit_metrics: MetricsGridFit,
    ) -> Result<GlyphSlotLoad, FontError> {
        let program = self.type1_glyph_program(glyph)?;
        let scale = scaler::ScaleMetrics::from_font_data(&self.data);
        let mut scaled_points = program
            .outline
            .points
            .iter()
            .map(|point| OutlinePoint {
                x: type1_scale_font_unit(point.x, scale.x_scale),
                y: type1_scale_font_unit(point.y, scale.y_scale),
                on_curve: point.on_curve,
            })
            .collect::<Vec<_>>();
        let advance_width = type1_scale_font_unit(program.advance_width, scale.x_scale);
        if scaled_points.is_empty() || program.outline.contours.is_empty() {
            let mut metrics = GlyphSlotMetrics {
                width: 0,
                height: 0,
                hori_bearing_x: 0,
                hori_bearing_y: 0,
                hori_advance: advance_width,
                vert_bearing_x: 0,
                vert_bearing_y: 0,
                vert_advance: 0,
            };
            if matches!(
                grid_fit_metrics,
                MetricsGridFit::Horizontal | MetricsGridFit::Vertical
            ) {
                metrics.hori_advance = ft_pix_round(metrics.hori_advance);
            }
            return Ok(GlyphSlotLoad {
                metrics,
                format: GlyphSlotLoadFormat::Outline,
                outline_cbox: BBox {
                    x_min: 0,
                    y_min: 0,
                    x_max: 0,
                    y_max: 0,
                },
                outline_bbox: BBox {
                    x_min: 0,
                    y_min: 0,
                    x_max: 0,
                    y_max: 0,
                },
                subglyphs: Vec::new(),
                slot_outline: Some(Outline::default()),
                render_outline: Some(LoadedOutline {
                    outline: Outline::default(),
                    left: 0,
                    bottom: 0,
                    top: 0,
                }),
            });
        }

        let mut x_min = scaled_points[0].x;
        let mut y_min = scaled_points[0].y;
        let mut x_max = x_min;
        let mut y_max = y_min;
        for point in &scaled_points[1..] {
            x_min = x_min.min(point.x);
            y_min = y_min.min(point.y);
            x_max = x_max.max(point.x);
            y_max = y_max.max(point.y);
        }
        let grid_x_min = ft_pix_floor(x_min);
        let grid_y_min = ft_pix_floor(y_min);
        let grid_x_max = ft_pix_ceil(x_max);
        let grid_y_max = ft_pix_ceil(y_max);
        let px_x_min = grid_x_min >> 6;
        let px_y_min = grid_y_min >> 6;
        let px_x_max = grid_x_max >> 6;
        let px_y_max = grid_y_max >> 6;
        let off_x = grid_x_min;
        let off_y = grid_y_min;
        let slot_points = scaled_points.clone();
        for point in &mut scaled_points {
            point.x -= off_x;
            point.y -= off_y;
        }
        let mut metrics = GlyphSlotMetrics {
            width: grid_x_max - grid_x_min,
            height: grid_y_max - grid_y_min,
            hori_bearing_x: grid_x_min,
            hori_bearing_y: grid_y_max,
            hori_advance: advance_width,
            vert_bearing_x: 0,
            vert_bearing_y: 0,
            vert_advance: 0,
        };
        match grid_fit_metrics {
            MetricsGridFit::None => {
                metrics.width = x_max - x_min;
                metrics.height = y_max - y_min;
                metrics.hori_bearing_x = x_min;
                metrics.hori_bearing_y = y_max;
            }
            MetricsGridFit::Horizontal | MetricsGridFit::Vertical => {
                metrics.hori_advance = ft_pix_round(metrics.hori_advance);
            }
        }
        let cbox = BBox {
            x_min,
            y_min,
            x_max,
            y_max,
        };
        Ok(GlyphSlotLoad {
            metrics,
            format: GlyphSlotLoadFormat::Outline,
            outline_cbox: cbox,
            outline_bbox: cbox,
            subglyphs: Vec::new(),
            slot_outline: Some(Outline {
                n_contours: i32::try_from(program.outline.contours.len()).unwrap_or(i32::MAX),
                contours: program.outline.contours.clone(),
                points: slot_points,
                tags: Vec::new(),
                contour_dropouts: Vec::new(),
                flags: 0,
                cbox_x_min: x_min,
                cbox_y_min: y_min,
                cbox_x_max: x_max,
                cbox_y_max: y_max,
            }),
            render_outline: Some(LoadedOutline {
                outline: Outline {
                    n_contours: i32::try_from(program.outline.contours.len()).unwrap_or(i32::MAX),
                    contours: program.outline.contours,
                    points: scaled_points,
                    tags: Vec::new(),
                    contour_dropouts: Vec::new(),
                    flags: 0,
                    cbox_x_min: 0,
                    cbox_y_min: 0,
                    cbox_x_max: px_x_max - px_x_min,
                    cbox_y_max: px_y_max - px_y_min,
                },
                left: px_x_min,
                bottom: px_y_min,
                top: px_y_max,
            }),
        })
    }

    fn glyph_slot_load_type1_no_scale(
        &self,
        glyph: u16,
        _vertical_layout: bool,
    ) -> Result<GlyphSlotLoad, FontError> {
        let program = self.type1_glyph_program(glyph)?;
        if program.outline.points.is_empty() || program.outline.contours.is_empty() {
            return Ok(GlyphSlotLoad {
                metrics: GlyphSlotMetrics {
                    width: 0,
                    height: 0,
                    hori_bearing_x: 0,
                    hori_bearing_y: 0,
                    hori_advance: program.advance_width,
                    vert_bearing_x: 0,
                    vert_bearing_y: 0,
                    vert_advance: 0,
                },
                format: GlyphSlotLoadFormat::Outline,
                outline_cbox: BBox {
                    x_min: 0,
                    y_min: 0,
                    x_max: 0,
                    y_max: 0,
                },
                outline_bbox: BBox {
                    x_min: 0,
                    y_min: 0,
                    x_max: 0,
                    y_max: 0,
                },
                subglyphs: Vec::new(),
                slot_outline: Some(Outline::default()),
                render_outline: Some(LoadedOutline {
                    outline: Outline::default(),
                    left: 0,
                    bottom: 0,
                    top: 0,
                }),
            });
        }
        let mut x_min = program.outline.points[0].x;
        let mut y_min = program.outline.points[0].y;
        let mut x_max = x_min;
        let mut y_max = y_min;
        for point in &program.outline.points[1..] {
            x_min = x_min.min(point.x);
            y_min = y_min.min(point.y);
            x_max = x_max.max(point.x);
            y_max = y_max.max(point.y);
        }
        let cbox = BBox {
            x_min,
            y_min,
            x_max,
            y_max,
        };
        Ok(GlyphSlotLoad {
            metrics: GlyphSlotMetrics {
                width: x_max - x_min,
                height: y_max - y_min,
                hori_bearing_x: x_min,
                hori_bearing_y: y_max,
                hori_advance: program.advance_width,
                vert_bearing_x: 0,
                vert_bearing_y: 0,
                vert_advance: 0,
            },
            format: GlyphSlotLoadFormat::Outline,
            outline_cbox: cbox,
            outline_bbox: cbox,
            subglyphs: Vec::new(),
            slot_outline: Some(Outline {
                n_contours: i32::try_from(program.outline.contours.len()).unwrap_or(i32::MAX),
                contours: program.outline.contours.clone(),
                points: program.outline.points.clone(),
                tags: Vec::new(),
                contour_dropouts: Vec::new(),
                flags: 0,
                cbox_x_min: x_min,
                cbox_y_min: y_min,
                cbox_x_max: x_max,
                cbox_y_max: y_max,
            }),
            render_outline: Some(LoadedOutline {
                outline: Outline {
                    n_contours: i32::try_from(program.outline.contours.len()).unwrap_or(i32::MAX),
                    contours: program.outline.contours,
                    points: program.outline.points,
                    tags: Vec::new(),
                    contour_dropouts: Vec::new(),
                    flags: 0,
                    cbox_x_min: x_min,
                    cbox_y_min: y_min,
                    cbox_x_max: x_max,
                    cbox_y_max: y_max,
                },
                left: ft_pix_floor(x_min) >> 6,
                bottom: ft_pix_floor(y_min) >> 6,
                top: ft_pix_ceil(y_max) >> 6,
            }),
        })
    }

    fn glyph_is_composite(&self, glyph_index: u16) -> Result<bool, FontError> {
        let loc = tt::loca::get_glyph_location(
            &self.data.loca_data,
            glyph_index,
            self.data.head.index_to_loc_format,
        )
        .ok_or_else(|| FontError::InvalidOutline("loca: glyph index out of range".into()))?;
        if loc.length == 0 {
            return Ok(false);
        }
        let bytes = self
            .data
            .glyf_data
            .get(loc.offset as usize..loc.offset as usize + loc.length as usize)
            .ok_or_else(|| FontError::InvalidOutline("glyf: data out of range".into()))?;
        if bytes.len() < 2 {
            return Err(FontError::InvalidOutline("glyf: glyph too short".into()));
        }
        let num_contours = i16::from_be_bytes([bytes[0], bytes[1]]);
        Ok(num_contours < 0)
    }

    fn fill_no_scale_vertical_metrics(
        &self,
        glyph_index: u16,
        outline: &tt::glyf::GlyphOutline,
        metrics: &mut GlyphSlotMetrics,
    ) {
        if let Some(vmtx) = &self.data.vmtx {
            let vertical = vmtx.get(glyph_index);
            let pp3_y = outline.ymax + vertical.tsb as i32;
            metrics.vert_bearing_y = pp3_y - metrics.hori_bearing_y;
            metrics.vert_advance = vertical.advance_height as i32;
        } else {
            let advance = vertical_advance_font_units(&self.data);
            metrics.vert_bearing_y = (advance - metrics.height) / 2;
            metrics.vert_advance = advance;
        }
        metrics.vert_bearing_x = metrics.hori_bearing_x - metrics.hori_advance / 2;
    }
}

fn scaled_slot_outline_from_outline(
    outline: &Outline,
    ol_cbox_x_min: i32,
    ol_cbox_y_min: i32,
    ol_cbox_x_max: i32,
    ol_cbox_y_max: i32,
) -> Outline {
    let off_x = ft_pix_floor(ol_cbox_x_min);
    let off_y = ft_pix_floor(ol_cbox_y_min);
    // Single-pass: allocate and apply offset in one loop instead of clone+loop.
    let points: Vec<crate::outline::OutlinePoint> = outline
        .points
        .iter()
        .map(|p| crate::outline::OutlinePoint {
            x: p.x + off_x,
            y: p.y + off_y,
            on_curve: p.on_curve,
        })
        .collect();
    Outline {
        n_contours: outline.n_contours,
        contours: outline.contours.clone(),
        points,
        tags: outline.tags.clone(),
        contour_dropouts: outline.contour_dropouts.clone(),
        flags: outline.flags,
        cbox_x_min: ol_cbox_x_min,
        cbox_y_min: ol_cbox_y_min,
        cbox_x_max: ol_cbox_x_max,
        cbox_y_max: ol_cbox_y_max,
    }
}

fn no_scale_slot_outline(outline: &tt::glyf::GlyphOutline, pp1x: i32, cbox: BBox) -> Outline {
    Outline {
        n_contours: i32::from(outline.num_contours),
        contours: outline
            .end_pts_of_contours
            .iter()
            .map(|&e| e as i16)
            .collect(),
        points: outline
            .points
            .iter()
            .map(|point| OutlinePoint {
                x: point.x - pp1x,
                y: point.y,
                on_curve: point.on_curve,
            })
            .collect(),
        tags: no_scale_outline_tags(outline),
        contour_dropouts: Vec::new(),
        flags: outline.outline_flags,
        cbox_x_min: cbox.x_min,
        cbox_y_min: cbox.y_min,
        cbox_x_max: cbox.x_max,
        cbox_y_max: cbox.y_max,
    }
}

fn no_scale_render_outline(
    outline: &tt::glyf::GlyphOutline,
    pp1x: i32,
    cbox: BBox,
) -> LoadedOutline {
    let px_x_min = ft_pix_floor(cbox.x_min) >> 6;
    let px_y_min = ft_pix_floor(cbox.y_min) >> 6;
    let px_x_max = ft_pix_ceil(cbox.x_max) >> 6;
    let px_y_max = ft_pix_ceil(cbox.y_max) >> 6;
    let off_x = ft_pix_floor(cbox.x_min);
    let off_y = ft_pix_floor(cbox.y_min);
    let points = outline
        .points
        .iter()
        .map(|point| OutlinePoint {
            x: point.x - pp1x - off_x,
            y: point.y - off_y,
            on_curve: point.on_curve,
        })
        .collect();

    // C renders the current `FT_GlyphSlot` outline as-is.  For `FT_LOAD_NO_SCALE`
    // those coordinates are font units, still interpreted by the rasterizers as
    // 26.6 values; only the bitmap-origin preset translates the outline.
    LoadedOutline {
        outline: Outline {
            n_contours: i32::from(outline.num_contours),
            contours: outline
                .end_pts_of_contours
                .iter()
                .map(|&e| e as i16)
                .collect(),
            points,
            tags: no_scale_outline_tags(outline),
            contour_dropouts: Vec::new(),
            flags: outline.outline_flags,
            cbox_x_min: 0,
            cbox_y_min: 0,
            cbox_x_max: px_x_max - px_x_min,
            cbox_y_max: px_y_max - px_y_min,
        },
        left: px_x_min,
        bottom: px_y_min,
        top: px_y_max,
    }
}

fn no_scale_outline_tags(outline: &tt::glyf::GlyphOutline) -> Vec<u8> {
    if outline.has_cubic_tags {
        outline.points.iter().map(|point| point.tag & 3).collect()
    } else {
        Vec::new()
    }
}

fn synthesize_vertical_metrics(metrics: &mut GlyphSlotMetrics) {
    // C `ft_synthesize_vertical_metrics` compensates for a bbox that does not
    // straddle the baseline before centering it in the vertical advance
    // (`src/base/ftobjs.c:3145-3166`).
    let mut height = metrics.height;
    if metrics.hori_bearing_y < 0 {
        if height < metrics.hori_bearing_y {
            height = metrics.hori_bearing_y;
        }
    } else if metrics.hori_bearing_y > 0 {
        height -= metrics.hori_bearing_y;
    }
    let mut advance = metrics.vert_advance;
    if advance == 0 {
        advance = height * 12 / 10;
    }
    metrics.vert_bearing_x = metrics.hori_bearing_x - metrics.hori_advance / 2;
    metrics.vert_bearing_y = (advance - height) / 2;
    metrics.vert_advance = advance;
}

fn vertical_advance_font_units(data: &FontData) -> i32 {
    if let Some(os2) = &data.os2 {
        return os2.s_typo_ascender as i32 - os2.s_typo_descender as i32;
    }
    data.hhea.ascent as i32 - data.hhea.descent as i32
}

fn grid_fit_for_layout(vertical_layout: bool) -> MetricsGridFit {
    if vertical_layout {
        MetricsGridFit::Vertical
    } else {
        MetricsGridFit::Horizontal
    }
}

fn is_pathological_metrics_cbox(scaled: &scaler::ScaledGlyph) -> bool {
    let width = scaled.cbox_x_max.saturating_sub(scaled.cbox_x_min);
    let height = scaled.cbox_y_max.saturating_sub(scaled.cbox_y_min);
    width > 16_384
        || height > 16_384
        || scaled.cbox_x_min.abs() > 16_384
        || scaled.cbox_x_max.abs() > 16_384
        || scaled.cbox_y_min.abs() > 16_384
        || scaled.cbox_y_max.abs() > 16_384
}

fn is_pathological_metrics_advance(scaled: &scaler::ScaledGlyph) -> bool {
    scaled.slot_advance_width.abs() > 16_384
        || scaled
            .slot_advance_width
            .saturating_sub(scaled.advance_width)
            .abs()
            > 16_384
}

fn default_unicode_charmap_index(cmap: &tt::cmap::CmapTable) -> Option<usize> {
    // C `find_unicode_charmap` scans in reverse directory order, first for
    // UCS-4 platform/encoding pairs and then for any Unicode charmap
    // (`src/base/ftobjs.c:1371-1448`).
    cmap.charmaps
        .iter()
        .rposition(|record| {
            record.is_unicode()
                && ((record.platform_id == 3 && record.encoding_id == 10)
                    || (record.platform_id == 0 && record.encoding_id == 4)
                    || (record.platform_id == 0 && record.encoding_id == 6 && record.format == 13))
        })
        .or_else(|| {
            cmap.charmaps
                .iter()
                .rposition(tt::cmap::CharmapRecord::is_unicode)
        })
}

fn grid_fit_horizontal_metrics(metrics: &mut GlyphSlotMetrics) {
    metrics.vert_bearing_x = ft_pix_floor(metrics.vert_bearing_x);
    metrics.vert_bearing_y = ft_pix_floor(metrics.vert_bearing_y);

    let right = ft_pix_ceil(metrics.hori_bearing_x + metrics.width);
    let bottom = ft_pix_floor(metrics.hori_bearing_y - metrics.height);
    metrics.hori_bearing_x = ft_pix_floor(metrics.hori_bearing_x);
    metrics.hori_bearing_y = ft_pix_ceil(metrics.hori_bearing_y);
    metrics.width = right - metrics.hori_bearing_x;
    metrics.height = metrics.hori_bearing_y - bottom;
    metrics.hori_advance = ft_pix_round(metrics.hori_advance);
    metrics.vert_advance = ft_pix_round(metrics.vert_advance);
}

fn grid_fit_vertical_metrics(metrics: &mut GlyphSlotMetrics) {
    metrics.hori_bearing_x = ft_pix_floor(metrics.hori_bearing_x);
    metrics.hori_bearing_y = ft_pix_ceil(metrics.hori_bearing_y);

    let right = ft_pix_ceil(metrics.vert_bearing_x + metrics.width);
    let bottom = ft_pix_ceil(metrics.vert_bearing_y + metrics.height);
    metrics.vert_bearing_x = ft_pix_floor(metrics.vert_bearing_x);
    metrics.vert_bearing_y = ft_pix_floor(metrics.vert_bearing_y);
    metrics.width = right - metrics.vert_bearing_x;
    metrics.height = bottom - metrics.vert_bearing_y;
    metrics.hori_advance = ft_pix_round(metrics.hori_advance);
    metrics.vert_advance = ft_pix_round(metrics.vert_advance);
}

impl SizeMetrics {
    fn tt_scale(&self) -> i32 {
        if self.x_ppem >= self.y_ppem {
            self.x_scale
        } else {
            self.y_scale
        }
    }

    fn tt_ppem(&self) -> i32 {
        i32::from(if self.x_ppem >= self.y_ppem {
            self.x_ppem
        } else {
            self.y_ppem
        })
    }

    fn tt_ratios(&self) -> (i32, i32) {
        // C `tt_size_reset` rejects a zero ppem before ratio setup.  Our
        // shared undefined-size sync still runs, so equal axes (including
        // 0/0) must be the identity instead of calling `FT_DivFix(0, 0)`.
        if self.x_ppem == self.y_ppem {
            (0x1_0000, 0x1_0000)
        } else if self.x_ppem > self.y_ppem {
            (
                0x1_0000,
                ft_div_fix(i32::from(self.y_ppem), i32::from(self.x_ppem)),
            )
        } else {
            (
                ft_div_fix(i32::from(self.x_ppem), i32::from(self.y_ppem)),
                0x1_0000,
            )
        }
    }

    fn tt_point_size(&self) -> i32 {
        if self.char_height != 0 {
            self.char_height
        } else {
            self.char_width
        }
    }

    fn from_char_size(
        char_width: i32,
        char_height: i32,
        x_dpi: u32,
        y_dpi: u32,
        data: &FontData,
    ) -> Self {
        Self::try_from_char_size(char_width, char_height, x_dpi, y_dpi, data)
            .unwrap_or_else(|_| Self::from_pixel_size(1, 1, data))
    }

    fn try_from_char_size(
        char_width: i32,
        char_height: i32,
        x_dpi: u32,
        y_dpi: u32,
        data: &FontData,
    ) -> Result<Self, SizeRequestError> {
        let (char_width, char_height) = normalize_char_size_dimensions(char_width, char_height);
        let (x_dpi, y_dpi) = normalize_size_resolutions(x_dpi, y_dpi);
        let scaled_width = scaled_char_size_26dot6(char_width, x_dpi);
        let scaled_height = scaled_char_size_26dot6(char_height, y_dpi);
        let x_ppem = ppem_from_scaled_char_size(scaled_width)?;
        let y_ppem = ppem_from_scaled_char_size(scaled_height)?;
        let units_per_em = i32::from(data.head.units_per_em);
        let x_scale = ft_div_fix(scaled_width.max(64), units_per_em);
        let y_scale = ft_div_fix(scaled_height.max(64), units_per_em);
        Ok(SizeMetrics {
            x_ppem,
            y_ppem,
            x_scale,
            y_scale,
            ascender: 0,
            descender: 0,
            height: 0,
            max_advance: 0,
            x_dpi,
            y_dpi,
            char_width,
            char_height,
        }
        .with_face_metrics(data))
    }

    fn from_pixel_size(pixel_width: u32, pixel_height: u32, data: &FontData) -> Self {
        // C normalizes a missing dimension from the other one, then clamps
        // both dimensions to 1..=0xFFFF (ftobjs.c:3574-3588).
        let mut width = if pixel_width == 0 {
            pixel_height
        } else {
            pixel_width
        };
        let mut height = if pixel_height == 0 {
            pixel_width
        } else {
            pixel_height
        };
        width = width.clamp(1, 0xFFFF);
        height = height.clamp(1, 0xFFFF);
        let units_per_em = i32::from(data.head.units_per_em);
        let x_scale = ft_div_fix((width as i32) << 6, units_per_em);
        let y_scale = ft_div_fix((height as i32) << 6, units_per_em);
        SizeMetrics {
            x_ppem: width as u16,
            y_ppem: height as u16,
            x_scale,
            y_scale,
            ascender: 0,
            descender: 0,
            height: 0,
            max_advance: 0,
            x_dpi: 72,
            y_dpi: 72,
            char_width: (width as i32) << 6,
            char_height: (height as i32) << 6,
        }
        .with_face_metrics(data)
    }

    fn from_size_request(request: SizeRequest, data: &FontData) -> Result<Self, SizeRequestError> {
        let units_per_em = i64::from(data.head.units_per_em);
        let (x_scale, y_scale, mut scaled_w, mut scaled_h) = match request.request_type {
            SizeRequestType::Scales => {
                let mut x_scale = request.width;
                let mut y_scale = request.height;
                if x_scale == 0 {
                    x_scale = y_scale;
                } else if y_scale == 0 {
                    y_scale = x_scale;
                }
                (x_scale, y_scale, 0, 0)
            }
            _ => {
                let (mut w, mut h) = match request.request_type {
                    SizeRequestType::Nominal => (units_per_em, units_per_em),
                    SizeRequestType::RealDim => {
                        let real_dim = i64::from(data.hhea.ascent) - i64::from(data.hhea.descent);
                        (real_dim, real_dim)
                    }
                    SizeRequestType::BBox => (
                        i64::from(data.head.x_max) - i64::from(data.head.x_min),
                        i64::from(data.head.y_max) - i64::from(data.head.y_min),
                    ),
                    SizeRequestType::Cell => (
                        i64::from(data.hhea.advance_width_max),
                        i64::from(data.hhea.ascent) - i64::from(data.hhea.descent),
                    ),
                    SizeRequestType::Scales => unreachable!(),
                };
                w = w.abs();
                h = h.abs();

                let mut scaled_w =
                    request_dimension(request.width, request.hori_resolution, "width")?;
                let mut scaled_h =
                    request_dimension(request.height, request.vert_resolution, "height")?;

                let mut y_scale = 0;
                if request.height != 0 || request.width == 0 {
                    if h == 0 {
                        return Err(SizeRequestError::DivideByZero);
                    }
                    y_scale = ft_div_fix_i64(scaled_h, h)?;
                }

                let x_scale = if request.width != 0 {
                    if w == 0 {
                        return Err(SizeRequestError::DivideByZero);
                    }
                    ft_div_fix_i64(scaled_w, w)?
                } else {
                    scaled_w = ft_mul_div_i64(scaled_h, w, h)?;
                    y_scale
                };

                let mut x_scale = x_scale;
                if request.height == 0 {
                    y_scale = x_scale;
                    scaled_h = ft_mul_div_i64(scaled_w, h, w)?;
                }

                if request.request_type == SizeRequestType::Cell {
                    if y_scale > x_scale {
                        y_scale = x_scale;
                    } else {
                        x_scale = y_scale;
                    }
                }

                (x_scale, y_scale, scaled_w, scaled_h)
            }
        };

        if request.request_type != SizeRequestType::Nominal {
            scaled_w = ft_mul_fix_i64(units_per_em, x_scale)?;
            scaled_h = ft_mul_fix_i64(units_per_em, y_scale)?;
        }

        let x_ppem = ppem_from_scaled_26dot6(scaled_w)?;
        let y_ppem = ppem_from_scaled_26dot6(scaled_h)?;
        if data.cff.is_none() && (x_ppem == 0 || y_ppem == 0) {
            // FreeType 2.14.3 TrueType driver `tt_size_request` calls
            // `FT_Request_Metrics`, then rejects zero ppem in `tt_size_reset`
            // (`src/truetype/ttdriver.c:349-410`, `ttobjs.c:1247-1248`).
            return Err(SizeRequestError::InvalidPpem);
        }
        if data.sbit.as_ref().is_some_and(|sbit| {
            sbit.kind() == tt::sbit::SbitTableKind::Cblc
                && sbit.strike_count() != 0
                && !sbit.has_strike(x_ppem, y_ppem)
        }) {
            // Pillow's `_imagingft.c` creates the face through FreeType, and the
            // TrueType/SFNT bitmap driver rejects CBLC/CBDT color bitmap size
            // requests that do not match an available strike before rendering.
            return Err(SizeRequestError::InvalidPixelSize);
        }
        Ok(SizeMetrics {
            x_ppem,
            y_ppem,
            x_scale: i32_from_i64(x_scale)?,
            y_scale: i32_from_i64(y_scale)?,
            ascender: 0,
            descender: 0,
            height: 0,
            max_advance: 0,
            x_dpi: normalize_dpi(request.hori_resolution),
            y_dpi: normalize_dpi(request.vert_resolution),
            char_width: i32_from_i64(request.width).unwrap_or_default(),
            char_height: i32_from_i64(request.height).unwrap_or_default(),
        }
        .with_face_metrics(data))
    }

    fn with_face_metrics(mut self, data: &FontData) -> Self {
        let (ascender, descender, height) = face_metric_values(data);
        let max_advance = i32::from(data.hhea.advance_width_max);

        self.ascender = ft_pix_ceil(ft_mul_fix(ascender, self.y_scale));
        self.descender = ft_pix_floor(ft_mul_fix(descender, self.y_scale));
        self.height = ft_pix_round(ft_mul_fix(height, self.y_scale));
        self.max_advance = ft_pix_round(ft_mul_fix(max_advance, self.x_scale));
        self
    }
}

fn true_type_hint_scales(data: &FontData, metrics: SizeMetrics) -> (i32, i32) {
    if data.cff.is_none() && data.head.flags & 8 != 0 {
        // C `tt_size_reset` recomputes hinted TrueType scales from rounded
        // integer ppems for the driver when `head.Flags & 8` is set
        // (`ttobjs.c:1255-1262`). `ScaleMetrics::new` owns that exact
        // integer-ppem `FT_DivFix` construction; build each axis independently
        // so non-square public size requests retain distinct hinted scales.
        let x = scaler::ScaleMetrics::new(f32::from(metrics.x_ppem), data.head.units_per_em);
        let y = scaler::ScaleMetrics::new(f32::from(metrics.y_ppem), data.head.units_per_em);
        (x.x_scale, y.y_scale)
    } else {
        (metrics.x_scale, metrics.y_scale)
    }
}

fn sync_active_size_metrics(data: &FontData, metrics: SizeMetrics) {
    let (x_scale, y_scale) = true_type_hint_scales(data, metrics);
    data.size_public_x_scale.set(metrics.x_scale);
    data.size_public_y_scale.set(metrics.y_scale);
    data.size_x_scale.set(x_scale);
    data.size_y_scale.set(y_scale);
    data.size_tt_scale.set(if metrics.x_ppem >= metrics.y_ppem {
        x_scale
    } else {
        y_scale
    });
    data.size_tt_ppem.set(metrics.tt_ppem());
    let (x_ratio, y_ratio) = metrics.tt_ratios();
    data.size_tt_x_ratio.set(x_ratio);
    data.size_tt_y_ratio.set(y_ratio);
    data.size_tt_point_size.set(metrics.tt_point_size());
}

fn request_dimension(value: i64, resolution: u32, _axis: &str) -> Result<i64, SizeRequestError> {
    if resolution == 0 {
        return Ok(value);
    }
    value
        .checked_mul(i64::from(resolution))
        .and_then(|value| value.checked_add(36))
        .map(|value| value / 72)
        .ok_or(SizeRequestError::InvalidPixelSize)
}

fn ppem_from_scaled_26dot6(value: i64) -> Result<u16, SizeRequestError> {
    let ppem = (value + 32) >> 6;
    if !(0..=i64::from(u16::MAX)).contains(&ppem) {
        return Err(SizeRequestError::InvalidPixelSize);
    }
    Ok(ppem as u16)
}

fn i32_from_i64(value: i64) -> Result<i32, SizeRequestError> {
    i32::try_from(value).map_err(|_| SizeRequestError::InvalidPixelSize)
}

fn ft_div_fix_i64(a: i64, b: i64) -> Result<i64, SizeRequestError> {
    let a = i32_from_i64(a)?;
    let b = i32_from_i64(b)?;
    if b == 0 {
        return Err(SizeRequestError::DivideByZero);
    }
    Ok(i64::from(ft_div_fix(a, b)))
}

fn ft_mul_fix_i64(a: i64, b: i64) -> Result<i64, SizeRequestError> {
    Ok(i64::from(ft_mul_fix(i32_from_i64(a)?, i32_from_i64(b)?)))
}

fn ft_mul_div_i64(a: i64, b: i64, c: i64) -> Result<i64, SizeRequestError> {
    if c == 0 {
        return Err(SizeRequestError::DivideByZero);
    }
    Ok(i64::from(ft_mul_div(
        i32_from_i64(a)?,
        i32_from_i64(b)?,
        i32_from_i64(c)?,
    )))
}

fn normalize_dpi(dpi: u32) -> u32 {
    if dpi == 0 { 72 } else { dpi }
}

fn normalize_size_resolutions(mut x_dpi: u32, mut y_dpi: u32) -> (u32, u32) {
    if x_dpi == 0 {
        x_dpi = y_dpi;
    } else if y_dpi == 0 {
        y_dpi = x_dpi;
    }
    if x_dpi == 0 { (72, 72) } else { (x_dpi, y_dpi) }
}

fn normalize_char_size_dimensions(mut char_width: i32, mut char_height: i32) -> (i32, i32) {
    if char_width == 0 {
        char_width = char_height;
    } else if char_height == 0 {
        char_height = char_width;
    }
    (char_width, char_height)
}

fn scaled_char_size_26dot6(char_size_26dot6: i32, dpi: u32) -> i32 {
    ft_mul_div(char_size_26dot6, dpi as i32, 72)
}

fn ppem_from_scaled_char_size(scaled_26dot6: i32) -> Result<u16, SizeRequestError> {
    let rounded = (i64::from(scaled_26dot6) + 32) & !63;
    let ppem = (rounded >> 6).max(1);
    u16::try_from(ppem).map_err(|_| SizeRequestError::InvalidPixelSize)
}

fn named_instance_postscript_name(
    name: &tt::name::NameTable,
    fvar: &Option<tt::fvar::FvarTable>,
    named_instance: usize,
) -> Option<String> {
    let fvar = fvar.as_ref()?;
    let instance = fvar.instances.get(named_instance.checked_sub(1)?)?;
    // `sfnt_get_var_ps_name` resolves and caps the prefix before consulting a
    // named instance's explicit PostScript name.  The 91-byte cap reserves
    // room for `-`, a 128-bit checksum, and `...` within MAX_PS_NAME_LEN.
    let mut prefix = tt::name::variations_postscript_prefix(name)?;
    prefix.truncate(VARIATION_PS_PREFIX_MAX_LEN);
    if let Some(name_id) = instance.postscript_name_id
        && let Some(name) = tt::name::name_string(name, name_id)
    {
        return Some(limit_variation_postscript_name(&prefix, name));
    }
    let Some(subfamily) = tt::name::name_string(name, instance.subfamily_name_id) else {
        // FreeType `sfnt_get_var_ps_name` in `src/sfnt/sfdriver.c` falls through
        // to `construct_instance_name` when a named instance lacks a usable
        // subfamily name, using non-default fvar coordinates plus axis tags.
        return Some(limit_variation_postscript_name(
            &prefix,
            synthesize_instance_postscript_name(&prefix, &fvar.axes, &instance.coords),
        ));
    };
    let mut result = String::with_capacity(prefix.len() + 1 + subfamily.len());
    result.push_str(&prefix);
    result.push('-');
    result.extend(subfamily.chars().filter(|ch| ch.is_ascii_alphanumeric()));
    Some(limit_variation_postscript_name(&prefix, result))
}

fn normalized_variation_coords_for_named_instance(
    fvar: &Option<tt::fvar::FvarTable>,
    named_instance: usize,
) -> Vec<i16> {
    let Some(fvar) = fvar else {
        return Vec::new();
    };
    let Some(instance) = named_instance
        .checked_sub(1)
        .and_then(|index| fvar.instances.get(index))
    else {
        return Vec::new();
    };
    fvar.axes
        .iter()
        .zip(&instance.coords)
        .map(|(axis, coord)| {
            tt::gvar::normalize_axis_coord(
                *coord,
                axis.min_value,
                axis.default_value,
                axis.max_value,
            )
        })
        .collect()
}

fn design_variation_coords_for_named_instance(
    fvar: &Option<tt::fvar::FvarTable>,
    named_instance: usize,
) -> Vec<i32> {
    let Some(fvar) = fvar else {
        return Vec::new();
    };
    let Some(instance) = named_instance
        .checked_sub(1)
        .and_then(|index| fvar.instances.get(index))
    else {
        return fvar.axes.iter().map(|axis| axis.default_value).collect();
    };
    fvar.axes
        .iter()
        .enumerate()
        .map(|(index, axis)| {
            instance
                .coords
                .get(index)
                .copied()
                .unwrap_or(axis.default_value)
                .clamp(axis.min_value, axis.max_value)
        })
        .collect()
}

fn design_variation_coords_for_design_coords(
    fvar: &Option<tt::fvar::FvarTable>,
    design_coords: &[i32],
) -> Vec<i32> {
    let Some(fvar) = fvar else {
        return Vec::new();
    };
    // C parity: FT_Set_Var_Design_Coordinates stores the caller's design
    // values for FT_Get_Var_Design_Coordinates while filling omitted axes
    // from fvar defaults; normalization for internal deltas clamps separately.
    // See freetype/src/base/ftmm.c:281-388 and truetype/ttgxvar.c.
    fvar.axes
        .iter()
        .enumerate()
        .map(|(index, axis)| {
            design_coords
                .get(index)
                .copied()
                .unwrap_or(axis.default_value)
        })
        .collect()
}

fn normalized_variation_coords_for_design_coords(
    fvar: &Option<tt::fvar::FvarTable>,
    design_coords: &[i32],
) -> Vec<i16> {
    let Some(fvar) = fvar else {
        return Vec::new();
    };
    fvar.axes
        .iter()
        .enumerate()
        .map(|(index, axis)| {
            let coord = design_coords
                .get(index)
                .copied()
                .unwrap_or(axis.default_value);
            tt::gvar::normalize_axis_coord(
                coord,
                axis.min_value,
                axis.default_value,
                axis.max_value,
            )
        })
        .collect()
}

fn design_coord_for_normalized_blend_16_16(blend_16_16: i32, axis: &tt::fvar::FvarAxis) -> i32 {
    let blend = blend_16_16.clamp(-65_536, 65_536);
    let extent = if blend < 0 {
        axis.default_value - axis.min_value
    } else {
        axis.max_value - axis.default_value
    };
    let delta = ((i64::from(extent) * i64::from(blend)) / 65_536) as i32;
    axis.default_value.saturating_add(delta)
}

fn blend_variation_coords_for_blend_coords_16_16(
    fvar: &Option<tt::fvar::FvarTable>,
    blend_coords_16_16: &[i32],
) -> Vec<i32> {
    let Some(fvar) = fvar else {
        return Vec::new();
    };
    // C parity: the public blend setter copies caller-provided normalized
    // 16.16 coordinates up to the axis count, fills omitted axes with default
    // blend zero, and ignores excess values.  This public getter state is
    // separate from the internal F2Dot14 coordinates used for variation math.
    // See freetype/src/base/ftmm.c:465-600 and truetype/ttgxvar.c.
    (0..fvar.axes.len())
        .map(|index| blend_coords_16_16.get(index).copied().unwrap_or(0))
        .collect()
}

const VARIATION_PS_NAME_MAX_LEN: usize = 127;
const VARIATION_PS_PREFIX_MAX_LEN: usize = VARIATION_PS_NAME_MAX_LEN - (1 + 32 + 3);

/// Apply FreeType's `sfnt_get_var_ps_name` length fallback.
///
/// Pinned `sfdriver.c:1017-1061` hashes the constructed C string including its
/// terminating NUL, then keeps the already-capped prefix and replaces the
/// remainder with `-<MurmurHash3-x86-128>...`.
fn limit_variation_postscript_name(prefix: &str, result: String) -> String {
    if result.len() < VARIATION_PS_NAME_MAX_LEN {
        return result;
    }

    let mut hash_input = Vec::with_capacity(result.len() + 1);
    hash_input.extend_from_slice(result.as_bytes());
    hash_input.push(0);
    let hash = murmur_hash_3_x86_128(&hash_input, 123_456_789);

    let mut limited = String::with_capacity(prefix.len() + 36);
    limited.push_str(prefix);
    limited.push('-');
    for word in hash {
        use std::fmt::Write as _;
        let _ = write!(limited, "{word:08X}");
    }
    limited.push_str("...");
    limited
}

fn murmur_hash_3_x86_128(bytes: &[u8], seed: u32) -> [u32; 4] {
    const C1: u32 = 0x239B_961B;
    const C2: u32 = 0xAB0E_9789;
    const C3: u32 = 0x38B3_4AE5;
    const C4: u32 = 0xA1E3_8B93;

    let mut h1 = seed;
    let mut h2 = seed;
    let mut h3 = seed;
    let mut h4 = seed;

    let mut blocks = bytes.chunks_exact(16);
    for block in &mut blocks {
        let mut k1 = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        let mut k2 = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        let mut k3 = u32::from_le_bytes([block[8], block[9], block[10], block[11]]);
        let mut k4 = u32::from_le_bytes([block[12], block[13], block[14], block[15]]);

        k1 = k1.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h1 ^= k1;
        h1 = h1.rotate_left(19).wrapping_add(h2);
        h1 = h1.wrapping_mul(5).wrapping_add(0x561C_CD1B);

        k2 = k2.wrapping_mul(C2).rotate_left(16).wrapping_mul(C3);
        h2 ^= k2;
        h2 = h2.rotate_left(17).wrapping_add(h3);
        h2 = h2.wrapping_mul(5).wrapping_add(0x0BCA_A747);

        k3 = k3.wrapping_mul(C3).rotate_left(17).wrapping_mul(C4);
        h3 ^= k3;
        h3 = h3.rotate_left(15).wrapping_add(h4);
        h3 = h3.wrapping_mul(5).wrapping_add(0x96CD_1C35);

        k4 = k4.wrapping_mul(C4).rotate_left(18).wrapping_mul(C1);
        h4 ^= k4;
        h4 = h4.rotate_left(13).wrapping_add(h1);
        h4 = h4.wrapping_mul(5).wrapping_add(0x32AC_3B17);
    }

    let tail = blocks.remainder();
    let mut k1 = 0u32;
    let mut k2 = 0u32;
    let mut k3 = 0u32;
    let mut k4 = 0u32;
    for (index, byte) in tail.iter().copied().enumerate() {
        let shift = (index % 4) * 8;
        match index / 4 {
            0 => k1 |= u32::from(byte) << shift,
            1 => k2 |= u32::from(byte) << shift,
            2 => k3 |= u32::from(byte) << shift,
            3 => k4 |= u32::from(byte) << shift,
            _ => unreachable!(),
        }
    }
    if tail.len() > 12 {
        h4 ^= k4.wrapping_mul(C4).rotate_left(18).wrapping_mul(C1);
    }
    if tail.len() > 8 {
        h3 ^= k3.wrapping_mul(C3).rotate_left(17).wrapping_mul(C4);
    }
    if tail.len() > 4 {
        h2 ^= k2.wrapping_mul(C2).rotate_left(16).wrapping_mul(C3);
    }
    if !tail.is_empty() {
        h1 ^= k1.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
    }

    let len = bytes.len() as u32;
    h1 ^= len;
    h2 ^= len;
    h3 ^= len;
    h4 ^= len;

    h1 = h1.wrapping_add(h2).wrapping_add(h3).wrapping_add(h4);
    h2 = h2.wrapping_add(h1);
    h3 = h3.wrapping_add(h1);
    h4 = h4.wrapping_add(h1);

    h1 = murmur_fmix32(h1);
    h2 = murmur_fmix32(h2);
    h3 = murmur_fmix32(h3);
    h4 = murmur_fmix32(h4);

    h1 = h1.wrapping_add(h2).wrapping_add(h3).wrapping_add(h4);
    h2 = h2.wrapping_add(h1);
    h3 = h3.wrapping_add(h1);
    h4 = h4.wrapping_add(h1);
    [h1, h2, h3, h4]
}

fn murmur_fmix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x85EB_CA6B);
    value ^= value >> 13;
    value = value.wrapping_mul(0xC2B2_AE35);
    value ^ (value >> 16)
}

fn synthesize_instance_postscript_name(
    prefix: &str,
    axes: &[tt::fvar::FvarAxis],
    coords: &[i32],
) -> String {
    let mut result = String::with_capacity(prefix.len() + axes.len() * 16);
    result.push_str(prefix);
    for (axis, coord) in axes.iter().zip(coords) {
        if *coord == axis.default_value {
            continue;
        }
        result.push('_');
        result.push_str(&fixed_16_16_to_short_decimal(*coord));
        push_variation_axis_tag(&mut result, axis.tag);
    }
    result
}

fn fixed_16_16_to_short_decimal(value: i32) -> String {
    if value == 0 {
        return "0".into();
    }

    let mut fixed = i64::from(value);
    let mut result = String::new();
    if fixed < 0 {
        result.push('-');
        fixed = -fixed;
    }

    let int_part = (fixed >> 16) & 0xFFFF;
    if int_part != 0 {
        result.push_str(&int_part.to_string());
    }

    let mut frac_part = fixed & 0xFFFF;
    if frac_part == 0 {
        return result;
    }

    result.push('.');
    let point_index = result.len() - 1;
    frac_part = frac_part * 10 + 5;
    for _ in 0..5 {
        let digit = frac_part / 0x10000;
        result.push(char::from(b'0' + digit as u8));
        frac_part %= 0x10000;
        if frac_part == 0 {
            break;
        }
        frac_part *= 10;
    }

    if result.len() - point_index - 1 == 5 {
        let mut last = result.pop().unwrap_or('0') as u8;
        if frac_part < 34480 * 10 && last == b'1' {
            last = b'0';
        } else if frac_part == 17232 * 10 && (last - b'0') % 2 == 1 {
            last -= 1;
        } else if frac_part < 17232 * 10 && last != b'0' {
            last -= 1;
        }
        result.push(char::from(last));
    }

    while result.ends_with('0') {
        result.pop();
    }
    if result.ends_with('.') {
        result.pop();
    }
    result
}

fn push_variation_axis_tag(result: &mut String, tag: u32) {
    for shift in [24, 16, 8, 0] {
        let byte = (tag >> shift) as u8;
        if byte != b' ' && byte.is_ascii_alphanumeric() {
            result.push(char::from(byte));
        }
    }
}

/// Pick (ascender, descender) as positive font-unit magnitudes.
///
/// FreeType's `sfnt_init_face` uses OS/2 usWinAscent/usWinDescent for the
/// face-level ascender/descender. The descender is converted to a positive
/// value for the public `(ascent, descent)` pair.
fn pick_metrics(data: &FontData) -> (i32, i32) {
    let (ascender, descender, _) = face_metric_values(data);
    (ascender, -descender)
}

/// Select the face-level ascender, descender, and height from sfobjs.c.
fn face_metric_values(data: &FontData) -> (i32, i32, i32) {
    if let Some(os2) = data.os2.as_ref().filter(|os2| os2.use_typo_metrics()) {
        let ascender = i32::from(os2.s_typo_ascender);
        let descender = i32::from(os2.s_typo_descender);
        return (
            ascender,
            descender,
            ascender - descender + i32::from(os2.s_typo_line_gap),
        );
    }

    let ascender = i32::from(data.hhea.ascent);
    let descender = i32::from(data.hhea.descent);
    if ascender != 0 || descender != 0 {
        return (
            ascender,
            descender,
            ascender - descender + i32::from(data.hhea.line_gap),
        );
    }

    let Some(os2) = data.os2.as_ref() else {
        return (ascender, descender, ascender - descender);
    };
    let typo_ascender = i32::from(os2.s_typo_ascender);
    let typo_descender = i32::from(os2.s_typo_descender);
    if typo_ascender != 0 || typo_descender != 0 {
        return (
            typo_ascender,
            typo_descender,
            typo_ascender - typo_descender + i32::from(os2.s_typo_line_gap),
        );
    }

    let win_ascender = i32::from(i16::from_be_bytes(os2.us_win_ascent.to_be_bytes()));
    let win_descender = -i32::from(i16::from_be_bytes(os2.us_win_descent.to_be_bytes()));
    (win_ascender, win_descender, win_ascender - win_descender)
}

#[cfg(test)]
mod tests {
    use super::BdfPropertyValue;
    use super::Font;

    const DEJAVU_SANS: &[u8] = include_bytes!("../tests/fixtures/input/fonts/DejaVuSans.ttf");

    fn test_font() -> Font {
        match Font::truetype(DEJAVU_SANS, 20.0) {
            Ok(font) => font,
            Err(err) => panic!("test font should load: {err}"),
        }
    }

    fn bdf_property_test_font() -> Font {
        let data = include_bytes!(
            "../tests/fixtures/input/fonts/bdf/properties-atoms-integers-cardinals.bdf"
        );
        match Font::memory_face(data, 0, 12.0) {
            Ok(font) => font,
            Err(err) => panic!("BDF fixture should load: {err}"),
        }
    }

    #[test]
    fn getbbox_uses_freetype_glyph_slot_contract() {
        let font = test_font();
        let single = font.getbbox("A");
        let text = font.getbbox("AA");

        assert_eq!(text, single);
    }

    #[test]
    fn getmask_uses_freetype_glyph_slot_contract() {
        let font = test_font();
        let single = match font.getmask("A") {
            Ok(mask) => mask,
            Err(err) => panic!("single glyph should render: {err}"),
        };
        let text = match font.getmask("AA") {
            Ok(mask) => mask,
            Err(err) => panic!("text should render: {err}"),
        };

        assert_eq!(text.width, single.width);
        assert_eq!(text.height, single.height);
        assert_eq!(text.xmin, single.xmin);
        assert_eq!(text.ymin, single.ymin);
        assert_eq!(text.advance_width, single.advance_width);
        assert_eq!(
            text.pixels.len(),
            text.width as usize * text.height as usize
        );
        assert_eq!(text.pixels, single.pixels);
    }

    #[test]
    fn getlength_reports_glyph_slot_advance_without_implicit_kerning() {
        let font = test_font();
        let single = match font.getlength("A") {
            Ok(value) => value,
            Err(error) => panic!("getlength('A') failed: {error}"),
        };
        let text = match font.getlength("AA") {
            Ok(value) => value,
            Err(error) => panic!("getlength('AA') failed: {error}"),
        };

        assert!(text > single);
        assert_eq!(text, single * 2.0);
    }

    #[test]
    fn bdf_property_returns_atom_property_from_startproperties_block() {
        let font = bdf_property_test_font();

        assert_eq!(
            font.bdf_property("FOUNDRY"),
            Some(&BdfPropertyValue::Atom("PillowRs".to_string()))
        );
    }

    #[test]
    fn bdf_property_uses_freetype_builtin_integer_format_for_pixel_size() {
        let font = bdf_property_test_font();

        assert_eq!(
            font.bdf_property("PIXEL_SIZE"),
            Some(&BdfPropertyValue::Integer(12))
        );
    }

    #[test]
    fn bdf_property_does_not_synthesize_missing_family_name() {
        let font = bdf_property_test_font();

        // Pinned FreeType 2.14.3 resolves BDF properties through
        // `src/base/ftbdf.c:FT_Get_BDF_Property` and
        // `src/bdf/bdfdrivr.c:bdf_get_bdf_property`; the BDF `FONT` name is
        // not synthesized into a `FAMILY_NAME` property.
        assert_eq!(font.bdf_property("FAMILY_NAME"), None);
    }
}

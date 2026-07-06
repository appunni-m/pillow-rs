//! FreeType-compatible font face API implemented in pure Rust.
//!
//! Runtime code follows FreeType glyph-slot behavior. Higher-level adapters,
//! including text layout or framework-specific packaging, live outside this
//! crate.

use crate::casts::{i32_from_f32, u32_from_i64, u32_from_usize};

use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_div, ft_mul_fix};
use crate::grays::{self, RasterResult};
use crate::scaler::{self, ft_pix_ceil, ft_pix_floor, ft_pix_round, pixel_round};
use crate::tables::FontData;
use crate::tt::{self, tag};
use std::sync::{Arc, OnceLock};

/// FreeType glyph load behavior used by high-level render helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadMode {
    /// `FT_LOAD_RENDER`: use the font's native TrueType program when present.
    #[default]
    Default,
    /// `FT_LOAD_RENDER | FT_LOAD_FORCE_AUTOHINT`: force the auto-hinter.
    ForceAutoHint,
    /// `FT_LOAD_NO_HINTING`: scale outlines without native or automatic hinting.
    NoHinting,
    /// `FT_LOAD_NO_AUTOHINT`: prefer native hints, but do not fall back to autohinting.
    NoAutoHint,
}

/// A loaded TrueType font at a given point size.
#[derive(Clone)]
pub struct Font {
    pub data: Arc<FontData>,
    pub size_pt: f32,
    pub load_mode: LoadMode,
    /// Face-level global hinting data: per-glyph script assignment,
    /// lazy-computed per-style metrics (Latin, Greek, etc.).
    /// Matches FreeType's AF_FaceGlobals.
    pub face_globals: crate::autohint::globals::FaceGlobals,
    /// Whether the font is italic/oblique (from head.mac_style bit 1).
    pub is_italic: bool,
    size_metrics: SizeMetrics,
    selected_charmap: usize,
    bytecode_context: Arc<OnceLock<tt::hinter::exec::ExecContext>>,
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

struct PositionedGlyph {
    x_position: i32,
    advance_width: i32,
    bbox_x_min: i32,
    bbox_x_max: i32,
    bbox_y_min: i32,
    bbox_y_max: i32,
    raster: Option<RasterResult>,
}

impl Font {
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

    /// Load a specific face from raw SFNT/TTC bytes with an explicit load mode.
    pub fn truetype_face_with_load_mode(
        data: &[u8],
        face_index: usize,
        size_pt: f32,
        load_mode: LoadMode,
    ) -> Result<Self, FontError> {
        let (num_faces, face_offset) = tt::resolve_face_index(data, face_index)?;
        let dir = tt::parse_table_directory_at(data, face_offset)?;

        let head_bytes = dir
            .find(data, tag(b"head"))
            .ok_or_else(|| FontError::InvalidFont("missing 'head' table".into()))?;
        let head = tt::head::parse_head(head_bytes)?;

        let maxp_bytes = dir
            .find(data, tag(b"maxp"))
            .ok_or_else(|| FontError::InvalidFont("missing 'maxp' table".into()))?;
        let maxp = tt::maxp::parse_maxp(maxp_bytes)?;

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
        let hmtx = tt::hmtx::parse_hmtx(hmtx_bytes, hhea.num_hmetrics, maxp.num_glyphs)?;

        let name = match dir.find(data, tag(b"name")) {
            Some(d) => tt::name::parse_name(d)?,
            None => crate::tt::name::NameTable {
                family: "Unknown".into(),
                subfamily: "Regular".into(),
                postscript_name: None,
            },
        };

        let os2 = dir.find(data, tag(b"OS/2")).and_then(tt::os2::parse_os2);
        let post = dir.find(data, tag(b"post")).and_then(tt::post::parse_post);
        let vhea = dir
            .find(data, tag(b"vhea"))
            .and_then(|d| tt::vhea::parse_vhea(d).ok());
        let vmtx = vhea.as_ref().and_then(|vhea| {
            dir.find(data, tag(b"vmtx"))
                .and_then(|d| tt::vmtx::parse_vmtx(d, vhea.num_vmetrics, maxp.num_glyphs).ok())
        });
        let hdmx = dir
            .find(data, tag(b"hdmx"))
            .and_then(|d| tt::hdmx::parse_hdmx(d, maxp.num_glyphs).ok());
        let kern = dir
            .find(data, tag(b"kern"))
            .and_then(|d| tt::kern::parse_kern(d).ok());

        let loca_data = dir
            .find(data, tag(b"loca"))
            .ok_or_else(|| FontError::InvalidFont("missing 'loca' table".into()))?
            .to_vec();
        let glyf_data = dir
            .find(data, tag(b"glyf"))
            .ok_or_else(|| FontError::InvalidFont("missing 'glyf' table".into()))?
            .to_vec();

        // Bytecode tables are optional. When present they are used by the
        // native TrueType path to match FreeType's default load behavior.
        // Missing tables fall back to unhinted scaling (same behavior as FreeType
        // without TT_USE_BYTECODE_INTERPRETER).
        let fpgm = dir.find(data, tag(b"fpgm")).map(|d| d.to_vec());
        let prep = dir.find(data, tag(b"prep")).map(|d| d.to_vec());
        let cvt = dir
            .find(data, tag(b"cvt "))
            .and_then(|d| crate::tt::hinter::tables::parse_cvt(d).ok());

        // Build FontData first, then compute Latin autohinter metrics.
        let font_data = Arc::new(FontData {
            raw_data: data.to_vec(),
            face_offset,
            face_index,
            num_faces,
            table_directory: dir,
            cmap,
            head,
            hhea,
            hmtx,
            maxp,
            name,
            os2,
            post,
            vhea,
            vmtx,
            hdmx,
            kern,
            loca_data,
            glyf_data,
            size_pt,
            fpgm,
            prep,
            cvt,
        });

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

        let selected_charmap = default_unicode_charmap_index(&font_data.cmap);

        Ok(Font {
            data: font_data,
            size_pt,
            load_mode,
            face_globals,
            is_italic,
            size_metrics,
            selected_charmap,
            bytecode_context: Arc::new(OnceLock::new()),
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

    /// Return the number of faces in the original font resource.
    pub fn num_faces(&self) -> usize {
        self.data.num_faces
    }

    /// Return scalar face metadata.
    pub fn face_info(&self) -> FaceInfo {
        FaceInfo {
            num_faces: self.data.num_faces,
            face_index: self.data.face_index,
            family_name: self.data.name.family.clone(),
            style_name: self.data.name.subfamily.clone(),
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
            ascender: self.data.hhea.ascent,
            descender: self.data.hhea.descent,
            height: self.data.hhea.ascent - self.data.hhea.descent + self.data.hhea.line_gap,
            max_advance_width: i32::from(self.data.hhea.advance_width_max),
            max_advance_height: self
                .data
                .vhea
                .as_ref()
                .map_or(0, |vhea| i32::from(vhea.advance_height_max)),
            underline_position: self
                .data
                .post
                .as_ref()
                .map_or(0, |post| post.underline_position),
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

    /// Equivalent to `FT_Get_FSType_Flags`.
    pub fn get_fstype_flags(&self) -> u16 {
        self.data.os2.as_ref().map_or(0, |os2| os2.fs_type)
    }

    /// Approximate `FT_FaceRec::face_flags` for supported SFNT outline faces.
    pub fn face_flags(&self) -> u32 {
        const FT_FACE_FLAG_SCALABLE: u32 = 1 << 0;
        const FT_FACE_FLAG_SFNT: u32 = 1 << 3;
        const FT_FACE_FLAG_HORIZONTAL: u32 = 1 << 4;
        const FT_FACE_FLAG_GLYPH_NAMES: u32 = 1 << 9;

        let mut flags = FT_FACE_FLAG_SCALABLE | FT_FACE_FLAG_SFNT | FT_FACE_FLAG_HORIZONTAL;
        if self.data.table_directory.record(tag(b"post")).is_some() {
            flags |= FT_FACE_FLAG_GLYPH_NAMES;
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

    pub(crate) fn clone_with_load_mode(&self, load_mode: LoadMode) -> Self {
        let mut font = self.clone();
        font.load_mode = load_mode;
        font
    }

    /// Equivalent to `FT_Set_Char_Size`.
    pub fn set_char_size(&mut self, char_width: i32, char_height: i32, x_dpi: u32, y_dpi: u32) {
        let height = if char_height == 0 {
            char_width
        } else {
            char_height
        };
        let width = if char_width == 0 { height } else { char_width };
        self.size_pt = height as f32 / 64.0;
        self.size_metrics = SizeMetrics::from_char_size(
            width,
            height,
            normalize_dpi(x_dpi),
            normalize_dpi(y_dpi),
            &self.data,
        );
        Arc::make_mut(&mut self.data).size_pt = self.size_pt;
        self.face_globals =
            crate::autohint::globals::FaceGlobals::new(self.data.clone(), self.is_italic);
    }

    /// Equivalent to `FT_Set_Pixel_Sizes`.
    pub fn set_pixel_sizes(&mut self, pixel_width: u32, pixel_height: u32) {
        let height = if pixel_height == 0 {
            pixel_width
        } else {
            pixel_height
        };
        self.size_pt = height as f32;
        self.size_metrics = SizeMetrics::from_pixel_size(pixel_width, height, &self.data);
        Arc::make_mut(&mut self.data).size_pt = self.size_pt;
        self.face_globals =
            crate::autohint::globals::FaceGlobals::new(self.data.clone(), self.is_italic);
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
        offset: usize,
        length: Option<usize>,
    ) -> Result<Vec<u8>, FontError> {
        let record =
            self.data.table_directory.record(tag).ok_or_else(|| {
                FontError::InvalidFont(format!("SFNT table 0x{tag:08X} not found"))
            })?;
        let start = record.offset as usize + offset;
        let table_end = record.offset as usize + record.length as usize;
        if start > table_end {
            return Err(FontError::InvalidFont(format!(
                "SFNT table offset {offset} exceeds table length {}",
                record.length
            )));
        }
        let end = match length {
            Some(length) => start + length,
            None => table_end,
        };
        self.data
            .raw_data
            .get(start..end)
            .map(|bytes| bytes.to_vec())
            .ok_or_else(|| FontError::InvalidFont("SFNT table read exceeds data".into()))
    }

    /// `getname()` → `(family, style)`.
    pub fn getname(&self) -> (&str, &str) {
        (&self.data.name.family, &self.data.name.subfamily)
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
    pub fn getlength(&self, text: &str) -> f32 {
        self.layout_advance(text) as f32 / 64.0
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
        let advance = self.data.hmtx.get(glyph).advance_width as i32;
        ft_mul_fix(advance, self.size_metrics.x_scale)
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
        let scaled = self.scale_glyph_for_metrics_default(glyph)?;
        Ok(self.slot_metrics_from_scaled(glyph, &scaled))
    }

    pub(crate) fn glyph_metrics_for_index_force_autohint(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        let metrics_cache = self.face_globals.get_metrics(glyph);
        let scaled = scaler::scale_glyph_for_metrics_with_autohint(
            &self.data,
            glyph,
            metrics_cache.as_deref(),
            self.is_italic,
        )?;
        Ok(self.slot_metrics_from_scaled(glyph, &scaled))
    }

    pub(crate) fn glyph_metrics_for_index_no_hinting(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        let scaled = scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)?;
        Ok(self.slot_metrics_from_scaled(glyph, &scaled))
    }

    pub(crate) fn glyph_metrics_for_index_no_scale(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        let outline = tt::glyf::load_glyph(
            &self.data.glyf_data,
            &self.data.loca_data,
            self.data.head.index_to_loc_format,
            glyph,
            &self.data.hmtx,
        )?;
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
            return Ok(metrics);
        }

        // C: `FT_LOAD_NO_SCALE` reaches `compute_glyph_metrics` in
        // `src/truetype/ttgload.c` with x/y scale set to 1.0.  The outline is
        // still translated by `-pp1.x` first (`ttgload.c:2582`), so metrics
        // are raw font units after that origin shift, not raw glyf coordinates.
        let pp1x = if outline.is_composite {
            outline.xmin - outline.sub_lsb
        } else {
            outline.xmin - h_metric.lsb as i32
        };
        let (x_min, y_min, x_max, y_max) = if outline.is_composite {
            // C: `compute_glyph_metrics` in `ttgload.c` uses the loader bbox
            // for composite glyphs instead of walking flattened component
            // points.  The saved header xMin preserves that public slot metric.
            (outline.bbox_xmin, outline.ymin, outline.xmax, outline.ymax)
        } else {
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
            (x_min, y_min, x_max, y_max)
        };

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
        Ok(metrics)
    }

    pub(crate) fn glyph_metrics_for_index_no_autohint(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        let scaled = self.scale_glyph_no_autohint_for_metrics(glyph)?;
        Ok(self.slot_metrics_from_scaled(glyph, &scaled))
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

        if scaled.outline.n_contours == 0 {
            return Ok(GlyphMask {
                width: 0,
                height: 0,
                pixels: Vec::new(),
                xmin: 0,
                ymin: 0,
                advance_width: 0,
            });
        }

        let raster = grays::rasterize(scaled.outline)?;
        Ok(GlyphMask {
            width: u32_from_usize(raster.width),
            height: u32_from_usize(raster.height),
            pixels: raster.pixels,
            xmin: scaled.bbox_x_min,
            ymin: scaled.bbox_y_min,
            advance_width: pixel_round(scaled.advance_width),
        })
    }

    pub(crate) fn scale_glyph_for_load_mode(
        &self,
        glyph: u16,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        match self.load_mode {
            LoadMode::Default => {
                let bytecode_context = self.native_bytecode_context()?;
                scaler::scale_glyph_native_default_with_bytecode_context(
                    &self.data,
                    glyph,
                    None,
                    self.is_italic,
                    bytecode_context,
                )
            }
            LoadMode::ForceAutoHint => {
                let metrics_cache = self.face_globals.get_metrics(glyph);
                scaler::scale_glyph(&self.data, glyph, metrics_cache.as_deref(), self.is_italic)
            }
            LoadMode::NoHinting => {
                scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)
            }
            LoadMode::NoAutoHint => self.scale_glyph_no_autohint_for_load(glyph),
        }
    }

    pub(crate) fn scale_glyph_for_metrics_default(
        &self,
        glyph: u16,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if self.data.fpgm.is_some() && self.data.cvt.is_some() {
            let bytecode_context = self.native_bytecode_context()?;
            let scaled = scaler::scale_glyph_for_metrics_with_bytecode_context(
                &self.data,
                glyph,
                self.is_italic,
                bytecode_context,
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
            let metrics_cache = self.face_globals.get_metrics(glyph);
            scaler::scale_glyph_for_metrics_with_autohint(
                &self.data,
                glyph,
                metrics_cache.as_deref(),
                self.is_italic,
            )
        }
    }

    fn scale_glyph_no_autohint_for_load(
        &self,
        glyph: u16,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if self.data.fpgm.is_some() && self.data.cvt.is_some() {
            let bytecode_context = self.native_bytecode_context()?;
            scaler::scale_glyph_native_default_with_bytecode_context(
                &self.data,
                glyph,
                None,
                self.is_italic,
                bytecode_context,
            )
        } else {
            scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)
        }
    }

    fn scale_glyph_no_autohint_for_metrics(
        &self,
        glyph: u16,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if self.data.fpgm.is_some() && self.data.cvt.is_some() {
            let bytecode_context = self.native_bytecode_context()?;
            scaler::scale_glyph_for_metrics_with_bytecode_context(
                &self.data,
                glyph,
                self.is_italic,
                bytecode_context,
            )
        } else {
            scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)
        }
    }

    fn native_bytecode_context(&self) -> Result<Option<&tt::hinter::exec::ExecContext>, FontError> {
        let (Some(fpgm), Some(cvt)) = (&self.data.fpgm, &self.data.cvt) else {
            return Ok(None);
        };
        if self.bytecode_context.get().is_none() {
            let scale = tt::hinter::HintScale {
                x_scale: self.size_metrics.x_scale,
                y_scale: self.size_metrics.y_scale,
                ppem: i32::from(self.size_metrics.x_ppem),
                storage_size: self.data.maxp.max_storage as usize,
                twilight_points: self.data.maxp.max_twilight_points as usize,
                is_composite: false,
                reset_vectors_at_glyph_entry: false,
                metrics_legacy_phantoms: false,
            };
            let prep = self.data.prep.as_deref().unwrap_or(&[]);
            let prepared = tt::hinter::prepare_context(cvt, fpgm, prep, &scale)?;
            let _ = self.bytecode_context.set(prepared);
        }
        Ok(self.bytecode_context.get())
    }

    fn layout_glyphs(&self, text: &str) -> Result<Vec<PositionedGlyph>, FontError> {
        let mut glyphs = Vec::new();
        let mut x_position = 0;
        let mut previous = None;
        for ch in text.chars() {
            let glyph_index = self.char_index(ch as u32);
            if let Some(previous) = previous {
                x_position += self.glyph_kerning(previous, glyph_index);
            }
            let scaled = self.scale_glyph_for_load_mode(glyph_index)?;
            let raster = if scaled.outline.n_contours == 0 {
                None
            } else {
                Some(grays::rasterize(scaled.outline)?)
            };
            glyphs.push(PositionedGlyph {
                x_position,
                advance_width: scaled.advance_width,
                bbox_x_min: scaled.bbox_x_min,
                bbox_x_max: scaled.bbox_x_max,
                bbox_y_min: scaled.bbox_y_min,
                bbox_y_max: scaled.bbox_y_max,
                raster,
            });
            x_position += scaled.advance_width;
            previous = Some(glyph_index);
        }
        Ok(glyphs)
    }

    fn layout_advance(&self, text: &str) -> i32 {
        text.chars().fold(0, |total, ch| {
            let glyph = self.char_index(ch as u32);
            if glyph == 0 {
                return total;
            }
            match self.glyph_metrics_for_index_default(glyph) {
                Ok(metrics) => total + metrics.hori_advance,
                Err(_) => total,
            }
        })
    }

    fn glyph_kerning(&self, left: u16, right: u16) -> i32 {
        self.data.kern.as_ref().map_or(0, |kern| {
            let value = i32::from(kern.get(left, right));
            ft_mul_fix(value, self.size_metrics.x_scale)
        })
    }

    fn layout_bounds(&self, text: &str) -> Result<(i32, i32, i32, i32), FontError> {
        let glyphs = self.layout_glyphs(text)?;
        Ok(layout_bounds_from_glyphs(&glyphs))
    }

    fn slot_metrics_from_scaled(
        &self,
        glyph_index: u16,
        scaled: &scaler::ScaledGlyph,
    ) -> GlyphSlotMetrics {
        let mut metrics = GlyphSlotMetrics {
            width: scaled.cbox_x_max - scaled.cbox_x_min,
            height: scaled.cbox_y_max - scaled.cbox_y_min,
            hori_bearing_x: scaled.cbox_x_min,
            hori_bearing_y: scaled.cbox_y_max,
            hori_advance: scaled.slot_advance_width,
            vert_bearing_x: 0,
            vert_bearing_y: 0,
            vert_advance: 0,
        };

        if let Some(vertical) = scaled.autohint_vertical {
            metrics.vert_bearing_x = vertical.bearing_x;
            metrics.vert_bearing_y = vertical.bearing_y;
            metrics.vert_advance = vertical.advance;
        } else if let Some(vmtx) = &self.data.vmtx {
            let vertical = vmtx.get(glyph_index);
            metrics.vert_bearing_y = ft_mul_fix(vertical.tsb as i32, self.size_metrics.y_scale);
            metrics.vert_advance =
                ft_mul_fix(vertical.advance_height as i32, self.size_metrics.y_scale);
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
        if scaled.autohint_vertical.is_none() {
            metrics.vert_bearing_x = metrics.hori_bearing_x - metrics.hori_advance / 2;
        }

        grid_fit_horizontal_metrics(&mut metrics);
        metrics
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

fn layout_bounds_from_glyphs(glyphs: &[PositionedGlyph]) -> (i32, i32, i32, i32) {
    let mut x_min = 0;
    let mut x_max = 0;
    let mut y_min = 0;
    let mut y_max = 0;
    let mut position = 0;
    for glyph in glyphs {
        let px = pixel_round(position);
        position += glyph.advance_width;
        x_max = x_max.max(pixel_round(position));
        if glyph.raster.is_some() {
            x_min = x_min.min(px + glyph.bbox_x_min);
            x_max = x_max.max(px + glyph.bbox_x_max);
            y_min = y_min.min(glyph.bbox_y_min);
            y_max = y_max.max(glyph.bbox_y_max);
        }
    }
    (x_min, x_max, y_min, y_max)
}

fn vertical_advance_font_units(data: &FontData) -> i32 {
    if let Some(os2) = &data.os2 {
        return os2.s_typo_ascender as i32 - os2.s_typo_descender as i32;
    }
    data.hhea.ascent as i32 - data.hhea.descent as i32
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

fn default_unicode_charmap_index(cmap: &tt::cmap::CmapTable) -> usize {
    cmap.charmaps
        .iter()
        .position(|record| {
            record.format == 12 && record.platform_id == 3 && record.encoding_id == 10
        })
        .or_else(|| {
            cmap.charmaps
                .iter()
                .position(|record| record.format == 12 && record.platform_id == 0)
        })
        .or_else(|| {
            cmap.charmaps
                .iter()
                .position(|record| record.format == 4 && record.platform_id == 3)
        })
        .or_else(|| {
            cmap.charmaps
                .iter()
                .position(|record| record.format == 4 && record.platform_id == 0)
        })
        .unwrap_or(0)
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

impl SizeMetrics {
    fn from_char_size(
        char_width: i32,
        char_height: i32,
        x_dpi: u32,
        y_dpi: u32,
        data: &FontData,
    ) -> Self {
        let x_dpi = normalize_dpi(x_dpi);
        let y_dpi = normalize_dpi(y_dpi);
        let x_ppem = ppem_from_char_size(char_width, x_dpi);
        let y_ppem = ppem_from_char_size(char_height, y_dpi);
        let units_per_em = i32::from(data.head.units_per_em);
        let x_scale = ft_div_fix((x_ppem as i32) << 6, units_per_em);
        let y_scale = ft_div_fix((y_ppem as i32) << 6, units_per_em);
        SizeMetrics {
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
        .with_face_metrics(data)
    }

    fn from_pixel_size(pixel_width: u32, pixel_height: u32, data: &FontData) -> Self {
        let width = if pixel_width == 0 {
            pixel_height
        } else {
            pixel_width
        };
        let height = if pixel_height == 0 {
            pixel_width
        } else {
            pixel_height
        };
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

    fn with_face_metrics(mut self, data: &FontData) -> Self {
        let ascender = i32::from(data.hhea.ascent);
        let descender = i32::from(data.hhea.descent);
        let height = i32::from(data.hhea.ascent) - i32::from(data.hhea.descent)
            + i32::from(data.hhea.line_gap);
        let max_advance = i32::from(data.hhea.advance_width_max);

        self.ascender = ft_pix_ceil(ft_mul_fix(ascender, self.y_scale));
        self.descender = ft_pix_floor(ft_mul_fix(descender, self.y_scale));
        self.height = ft_pix_round(ft_mul_fix(height, self.y_scale));
        self.max_advance = ft_pix_round(ft_mul_fix(max_advance, self.x_scale));
        self
    }
}

fn normalize_dpi(dpi: u32) -> u32 {
    if dpi == 0 { 72 } else { dpi }
}

fn ppem_from_char_size(char_size_26dot6: i32, dpi: u32) -> u16 {
    let ppem_26dot6 = ft_mul_div(char_size_26dot6, dpi as i32, 72);
    (((ppem_26dot6 + 32) & !63) >> 6).max(1) as u16
}

/// Pick (ascender, descender) as positive font-unit magnitudes.
///
/// FreeType's `sfnt_init_face` uses OS/2 usWinAscent/usWinDescent for the
/// face-level ascender/descender. The descender is converted to a positive
/// value for the public `(ascent, descent)` pair.
fn pick_metrics(data: &FontData) -> (i32, i32) {
    if let Some(pair) = pick_typo_metrics(data) {
        return pair;
    }
    let asc = data.hhea.ascent as i32;
    let desc = -data.hhea.descent as i32;
    if asc != 0 || desc != 0 {
        return (asc, desc);
    }
    pick_os2_metrics(data).unwrap_or((asc, desc))
}

/// Priority 1: OS/2 sTypoAscender / sTypoDescender when USE_TYPO_METRICS is set.
fn pick_typo_metrics(data: &FontData) -> Option<(i32, i32)> {
    let os2 = data.os2.as_ref()?;
    if os2.use_typo_metrics() {
        Some((os2.s_typo_ascender as i32, (-os2.s_typo_descender) as i32))
    } else {
        None
    }
}

/// Priority 2-3: Try OS/2 typo, then usWin fallback (sfobjs.c:1395-1413).
fn pick_os2_metrics(data: &FontData) -> Option<(i32, i32)> {
    let os2 = data.os2.as_ref()?;
    let ta = os2.s_typo_ascender as i32;
    let td = -os2.s_typo_descender as i32;
    if ta != 0 || td != 0 {
        return Some((ta, td));
    }
    Some((os2.us_win_ascent as i32, os2.us_win_descent as i32))
}

#[cfg(test)]
mod tests {
    use super::Font;

    const DEJAVU_SANS: &[u8] = include_bytes!("../tests/fixtures/input/fonts/DejaVuSans.ttf");

    fn test_font() -> Font {
        match Font::truetype(DEJAVU_SANS, 20.0) {
            Ok(font) => font,
            Err(err) => panic!("test font should load: {err}"),
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
        let single = font.getlength("A");
        let text = font.getlength("AA");

        assert!(text > single);
        assert_eq!(text, single * 2.0);
    }
}

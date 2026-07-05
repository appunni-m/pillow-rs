//! PIL `ImageFont` compatibility layer on top of the FreeType port.
//!
//! Mirrors the subset of PIL's `FreeTypeFont` API used by the coverage matrix:
//! `truetype`, `getmask`, `getbbox`, `getmetrics`, `getname`, `getlength`.

use crate::casts::{i32_from_f32, u32_from_i32, u32_from_i64, u32_from_usize, usize_from_i32};

use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_div, ft_mul_fix};
use crate::grays::{self, RasterResult};
use crate::scaler::{
    self, ft_pix_ceil, ft_pix_floor, ft_pix_round, pixel_ceil, pixel_round, ScaleMetrics,
};
use crate::tables::FontData;
use crate::tt::{self, tag};
use std::sync::Arc;

/// Selects the rendering pipeline for `getmask` / `getbbox`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitmapBackend {
    /// PIL-style mask: baseline-padded with advance-aware width.
    /// bbox uses ascender-relative screen coords.
    #[default]
    PIL,
    /// Raw FreeType output: bitmap as-is, no padding, FreeType bbox coords.
    FreeType,
}

/// A loaded TrueType font at a given point size.
#[derive(Clone)]
pub struct Font {
    pub data: Arc<FontData>,
    pub size_pt: f32,
    /// Selected rendering backend.
    pub backend: BitmapBackend,
    /// Face-level global hinting data: per-glyph script assignment,
    /// lazy-computed per-style metrics (Latin, Greek, etc.).
    /// Matches FreeType's AF_FaceGlobals.
    pub face_globals: crate::autohint::globals::FaceGlobals,
    /// Whether the font is italic/oblique (from head.mac_style bit 1).
    pub is_italic: bool,
    size_metrics: SizeMetrics,
    selected_charmap: usize,
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
    pub ascender: i16,
    pub descender: i16,
    pub height: i16,
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
    /// Top bearing offset in pixels (bbox ymin — used for vertical placement).
    /// PIL convention: positive = above baseline.
    pub ymin: i32,
    /// Advance width in 26.6 fixed-point format.
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
    /// `FT_Set_Char_Size` for the table subset PIL touches.
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
    /// use pillow_rs_freetype::{Font, BitmapBackend};
    /// let font_data = std::fs::read("DejaVuSans.ttf").unwrap();
    /// let font = Font::truetype(&font_data, 10.0, BitmapBackend::PIL).unwrap();
    /// assert_eq!(font.getname(), ("DejaVu Sans", "Book"));
    /// ```
    pub fn truetype(data: &[u8], size_pt: f32, backend: BitmapBackend) -> Result<Self, FontError> {
        Self::truetype_face(data, 0, size_pt, backend)
    }

    /// Load a specific face from raw SFNT/TTC bytes.
    ///
    /// `face_index` follows FreeType's zero-based face selection semantics.
    pub fn truetype_face(
        data: &[u8],
        face_index: usize,
        size_pt: f32,
        backend: BitmapBackend,
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

        let loca_data = dir
            .find(data, tag(b"loca"))
            .ok_or_else(|| FontError::InvalidFont("missing 'loca' table".into()))?
            .to_vec();
        let glyf_data = dir
            .find(data, tag(b"glyf"))
            .ok_or_else(|| FontError::InvalidFont("missing 'glyf' table".into()))?
            .to_vec();

        // Bytecode tables: optional, required for PIL backend pixel parity.
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
            vhea,
            vmtx,
            hdmx,
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
            font_data.head.units_per_em,
        );

        let selected_charmap = default_unicode_charmap_index(&font_data.cmap);

        Ok(Font {
            data: font_data,
            size_pt,
            backend,
            face_globals,
            is_italic,
            size_metrics,
            selected_charmap,
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
            ascender: self.data.hhea.ascent,
            descender: self.data.hhea.descent,
            height: self.data.hhea.ascent - self.data.hhea.descent + self.data.hhea.line_gap,
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
            self.data.head.units_per_em,
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
        self.size_metrics =
            SizeMetrics::from_pixel_size(pixel_width, height, self.data.head.units_per_em);
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
    /// PIL returns `face->size->metrics.ascender >> 6` and
    /// `-face->size->metrics.descender >> 6`, where the FreeType metrics are
    /// in 26.6 format after FT_PIX_ROUND. For the test fonts, this is
    /// equivalent to ceil(|fu_val| * ppem / upem).
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

    /// `getlength(text)` → total advance width in pixels (float).
    pub fn getlength(&self, text: &str) -> f32 {
        self.layout_advance(text) as f32 / 64.0
    }

    /// Return `FT_GlyphSlotRec::metrics` for a Unicode codepoint loaded with
    /// FreeType's default TrueType load path.
    ///
    /// This is the scalar metrics path used before rendering: native bytecode
    /// hinting is allowed, autohinting is not forced, and no bitmap render is
    /// requested.
    pub fn glyph_metrics(&self, codepoint: u32) -> Result<GlyphSlotMetrics, FontError> {
        let glyph = self.char_index(codepoint);
        let scaled = if self.data.fpgm.is_some() && self.data.cvt.is_some() {
            let scaled = scaler::scale_glyph_for_metrics(&self.data, glyph, self.is_italic)?;
            if is_pathological_metrics_cbox(&scaled) || is_pathological_metrics_advance(&scaled) {
                let metrics_cache = self.face_globals.get_metrics(glyph);
                scaler::scale_glyph_for_metrics_with_autohint_preserve_advance(
                    &self.data,
                    glyph,
                    metrics_cache.as_ref(),
                    self.is_italic,
                )?
            } else {
                scaled
            }
        } else {
            let metrics_cache = self.face_globals.get_metrics(glyph);
            scaler::scale_glyph_for_metrics_with_autohint(
                &self.data,
                glyph,
                metrics_cache.as_ref(),
                self.is_italic,
            )?
        };
        Ok(self.slot_metrics_from_scaled(glyph, &scaled))
    }

    /// `getbbox(text)` → `(left, top, right, bottom)` in pixels.
    ///
    /// `BitmapBackend::PIL`: PIL coords, y-down from ascender with baseline padding.
    /// `BitmapBackend::FreeType`: raw FreeType bbox, y-up from baseline.
    pub fn getbbox(&self, text: &str) -> (i32, i32, i32, i32) {
        if text.is_empty() {
            return (0, 0, 0, 0);
        }
        if self.backend == BitmapBackend::FreeType {
            return self.getbbox_single_glyph(text);
        }
        match self.layout_bounds(text) {
            Ok((x_min, x_max, y_min, y_max)) => match self.backend {
                BitmapBackend::PIL => {
                    let scale = ScaleMetrics::new(self.data.size_pt, self.data.head.units_per_em);
                    let asc_26 = ft_mul_fix(pick_metrics(&self.data).0, scale.y_scale);
                    let asc_px = pixel_ceil(asc_26);
                    (x_min, asc_px - y_max, x_max, asc_px - y_min)
                }
                BitmapBackend::FreeType => (x_min, y_min, x_max, y_max),
            },
            Err(_) => (0, 0, 0, 0),
        }
    }

    /// `getmask(text)` → 8-bit alpha bitmap sized to PIL's glyph mask box,
    /// matching PIL's `getmask` on an `L` image.
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
    /// use pillow_rs_freetype::{Font, BitmapBackend};
    /// let font_data = std::fs::read("DejaVuSans.ttf").unwrap();
    /// let font = Font::truetype(&font_data, 10.0, BitmapBackend::PIL).unwrap();
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
        if self.backend == BitmapBackend::FreeType {
            return self.getmask_single_glyph(text);
        }

        let glyphs = self.layout_glyphs(text)?;
        let (x_min, x_max, y_min, y_max) = layout_bounds_from_glyphs(&glyphs);
        let width = u32_from_i32(x_max - x_min);
        let height = u32_from_i32(y_max - y_min);
        let width_usize = width as usize;
        let height_usize = height as usize;
        let len = width_usize.checked_mul(height_usize).ok_or_else(|| {
            FontError::InvalidOutline("rendered text mask dimensions overflow".into())
        })?;
        let mut pixels = vec![0u8; len];

        if width_usize != 0 && height_usize != 0 {
            for glyph in &glyphs {
                if let Some(raster) = &glyph.raster {
                    let dst_x =
                        usize_from_i32(pixel_round(glyph.x_position) + glyph.bbox_x_min - x_min);
                    let dst_y = usize_from_i32(y_max - glyph.bbox_y_max);
                    for y in 0..raster.height {
                        let src = y * raster.width;
                        let dst = (dst_y + y) * width_usize + dst_x;
                        if dst + raster.width <= pixels.len()
                            && dst_x + raster.width <= width_usize
                            && src + raster.width <= raster.pixels.len()
                        {
                            for x in 0..raster.width {
                                let value = raster.pixels[src + x];
                                let target = &mut pixels[dst + x];
                                *target = (*target).max(value);
                            }
                        }
                    }
                }
            }
        }

        Ok(GlyphMask {
            width,
            height,
            pixels,
            xmin: x_min,
            ymin: y_min,
            advance_width: pixel_round(self.layout_advance(text)),
        })
    }
}

impl Font {
    fn getbbox_single_glyph(&self, text: &str) -> (i32, i32, i32, i32) {
        let data = &self.data;
        let ch = text.chars().next().unwrap_or('\0');
        let glyph = self.char_index(ch as u32);
        let metrics_cache = self.face_globals.get_metrics(glyph);
        match scaler::scale_glyph(data, glyph, metrics_cache.as_ref(), self.is_italic) {
            Ok(g) if g.outline.n_contours > 0 => {
                // Raw FreeType bbox: pixel coords from outline, y-up from baseline.
                (g.bbox_x_min, g.bbox_y_min, g.bbox_x_max, g.bbox_y_max)
            }
            _ => (0, 0, 0, 0),
        }
    }

    fn getmask_single_glyph(&self, text: &str) -> Result<GlyphMask, FontError> {
        let data = &self.data;
        let ch = text.chars().next().unwrap_or('\0');
        let glyph = self.char_index(ch as u32);
        let metrics_cache = self.face_globals.get_metrics(glyph);
        let scaled = scaler::scale_glyph(data, glyph, metrics_cache.as_ref(), self.is_italic)?;

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

    fn layout_glyphs(&self, text: &str) -> Result<Vec<PositionedGlyph>, FontError> {
        let mut glyphs = Vec::new();
        let mut x_position = 0;
        for ch in text.chars() {
            let glyph_index = self.char_index(ch as u32);
            let metrics_cache = self.face_globals.get_metrics(glyph_index);
            // Pillow's `_imagingft.c` fallback layout uses FT_LOAD_DEFAULT for
            // L-mode rendering, then positions each glyph with PIXEL(position).
            let metrics_for_scale = match self.backend {
                BitmapBackend::PIL => None,
                BitmapBackend::FreeType => metrics_cache.as_ref(),
            };
            let scaled = match self.backend {
                BitmapBackend::PIL => scaler::scale_glyph_native_default(
                    &self.data,
                    glyph_index,
                    metrics_for_scale,
                    self.is_italic,
                )?,
                BitmapBackend::FreeType => {
                    scaler::scale_glyph(&self.data, glyph_index, metrics_for_scale, self.is_italic)?
                }
            };
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
        }
        Ok(glyphs)
    }

    fn layout_advance(&self, text: &str) -> i32 {
        let data = &self.data;
        let scale = ScaleMetrics::new(data.size_pt, data.head.units_per_em);
        text.chars().fold(0, |total, ch| {
            let glyph = self.char_index(ch as u32);
            let m = data.hmtx.get(glyph);
            total + ft_mul_fix(m.advance_width as i32, scale.x_scale)
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
        units_per_em: u16,
    ) -> Self {
        let x_dpi = normalize_dpi(x_dpi);
        let y_dpi = normalize_dpi(y_dpi);
        let x_ppem = ppem_from_char_size(char_width, x_dpi);
        let y_ppem = ppem_from_char_size(char_height, y_dpi);
        let x_scale = ft_div_fix((x_ppem as i32) << 6, units_per_em as i32);
        let y_scale = ft_div_fix((y_ppem as i32) << 6, units_per_em as i32);
        SizeMetrics {
            x_ppem,
            y_ppem,
            x_scale,
            y_scale,
            x_dpi,
            y_dpi,
            char_width,
            char_height,
        }
    }

    fn from_pixel_size(pixel_width: u32, pixel_height: u32, units_per_em: u16) -> Self {
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
        let x_scale = ft_div_fix((width as i32) << 6, units_per_em as i32);
        let y_scale = ft_div_fix((height as i32) << 6, units_per_em as i32);
        SizeMetrics {
            x_ppem: width as u16,
            y_ppem: height as u16,
            x_scale,
            y_scale,
            x_dpi: 72,
            y_dpi: 72,
            char_width: (width as i32) << 6,
            char_height: (height as i32) << 6,
        }
    }
}

fn normalize_dpi(dpi: u32) -> u32 {
    if dpi == 0 {
        72
    } else {
        dpi
    }
}

fn ppem_from_char_size(char_size_26dot6: i32, dpi: u32) -> u16 {
    let ppem_26dot6 = ft_mul_div(char_size_26dot6, dpi as i32, 72);
    (((ppem_26dot6 + 32) & !63) >> 6).max(1) as u16
}

/// Pick (ascender, descender) as positive font-unit magnitudes.
///
/// FreeType's `sfnt_init_face` uses OS/2 usWinAscent/usWinDescent for the
/// face-level ascender/descender. The descender is converted to a positive
/// value matching PIL's convention.
// ✅ VERIFIED: OS/2 priority lookup matches C (sfobjs.c).
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
    use super::{BitmapBackend, Font};

    const DEJAVU_SANS: &[u8] = include_bytes!("../tests/fixtures/input/fonts/DejaVuSans.ttf");

    fn test_font() -> Font {
        match Font::truetype(DEJAVU_SANS, 20.0, BitmapBackend::PIL) {
            Ok(font) => font,
            Err(err) => panic!("test font should load: {err}"),
        }
    }

    #[test]
    fn getbbox_uses_the_whole_string() {
        let font = test_font();
        let single = font.getbbox("A");
        let text = font.getbbox("AA");

        assert!(text.2 - text.0 > single.2 - single.0);
        assert!(text.3 - text.1 >= single.3 - single.1);
    }

    #[test]
    fn getmask_uses_the_whole_string() {
        let font = test_font();
        let single = match font.getmask("A") {
            Ok(mask) => mask,
            Err(err) => panic!("single glyph should render: {err}"),
        };
        let text = match font.getmask("AA") {
            Ok(mask) => mask,
            Err(err) => panic!("text should render: {err}"),
        };

        assert!(text.width > single.width);
        assert_eq!(
            text.pixels.len(),
            text.width as usize * text.height as usize
        );
        assert!(text.pixels.iter().any(|&pixel| pixel != 0));
    }

    #[test]
    fn getlength_reports_run_advance() {
        let font = test_font();
        let single = font.getlength("A");
        let text = font.getlength("AA");

        assert!(text > single);
        assert_eq!(text, single * 2.0);
    }
}

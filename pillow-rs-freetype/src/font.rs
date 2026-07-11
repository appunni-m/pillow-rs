//! FreeType-compatible font face API implemented in pure Rust.
//!
//! Runtime code follows FreeType glyph-slot behavior. Higher-level adapters,
//! including text layout or framework-specific packaging, live outside this
//! crate.

use crate::casts::{i16_from_i32, i32_from_f32, u32_from_i64, u32_from_usize, usize_from_i32};

use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_div, ft_mul_fix};
use crate::grays::{self, RasterResult};
use crate::outline::{Outline, OutlinePoint};
use crate::scaler::{self, ft_pix_ceil, ft_pix_floor, ft_pix_round, pixel_round};
use crate::tables::FontData;
use crate::tt::hinter::NativeHintMode;
use crate::tt::{self, tag};
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
    /// Face-level global hinting data: per-glyph script assignment,
    /// lazy-computed per-style metrics (Latin, Greek, etc.).
    /// Matches FreeType's AF_FaceGlobals.
    pub face_globals: crate::autohint::globals::FaceGlobals,
    /// Whether the font is italic/oblique (from head.mac_style bit 1).
    pub is_italic: bool,
    size_metrics: SizeMetrics,
    selected_charmap: usize,
    bytecode_context: BytecodeContextCache,
    /// Reusable raster scratch space for gray rasterizer passes.
    /// Avoids allocating scanline cell vectors on every glyph render.
    pub(crate) raster_scratch: std::cell::RefCell<crate::grays::RasterScratch>,
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

struct PositionedGlyph {
    x_position: i32,
    advance_width: i32,
    bbox_x_min: i32,
    bbox_x_max: i32,
    bbox_y_min: i32,
    bbox_y_max: i32,
    raster: Option<RasterResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricsGridFit {
    None,
    Horizontal,
    Vertical,
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
                family: "Unknown".into(),
                subfamily: "Regular".into(),
                postscript_name: None,
                records: Vec::new(),
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
            head,
            hhea,
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
            loca_data,
            glyf_data,
            size_pt: std::cell::Cell::new(size_pt),
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

        let selected_charmap = default_unicode_charmap_index(&font_data.cmap);

        Ok(Font {
            data: font_data,
            size_pt,
            load_mode,
            face_globals,
            is_italic,
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

    /// Return the number of faces in the original font resource.
    pub fn num_faces(&self) -> usize {
        self.data.num_faces
    }

    /// Return scalar face metadata.
    pub fn face_info(&self) -> FaceInfo {
        let (ascender, descender, height) = face_metric_values(&self.data);
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
            ascender: i16_from_i32(ascender),
            descender: i16_from_i32(descender),
            height: i16_from_i32(height),
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

    /// Equivalent to `FT_Get_Glyph_Name` for supported SFNT `post` names.
    pub fn glyph_name(&self, glyph_index: u32) -> Option<&str> {
        let glyph_index = usize::try_from(glyph_index).ok()?;
        self.data
            .post
            .as_ref()?
            .glyph_name(glyph_index, self.data.maxp.num_glyphs)
    }

    /// Equivalent to `FT_Get_Name_Index` for supported SFNT `post` names.
    pub fn name_index(&self, glyph_name: &str) -> u32 {
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

    /// Number of raw SFNT name records exposed by `FT_Get_Sfnt_Name_Count`.
    pub fn sfnt_name_count(&self) -> usize {
        self.data.name.records.len()
    }

    /// Return one raw SFNT name record by index.
    pub fn sfnt_name(&self, index: usize) -> Option<&tt::name::SfntNameRecord> {
        self.data.name.records.get(index)
    }

    /// Approximate `FT_FaceRec::face_flags` for supported SFNT outline faces.
    pub fn face_flags(&self) -> u32 {
        const FT_FACE_FLAG_SCALABLE: u32 = 1 << 0;
        const FT_FACE_FLAG_FIXED_WIDTH: u32 = 1 << 2;
        const FT_FACE_FLAG_SFNT: u32 = 1 << 3;
        const FT_FACE_FLAG_HORIZONTAL: u32 = 1 << 4;
        const FT_FACE_FLAG_VERTICAL: u32 = 1 << 5;
        const FT_FACE_FLAG_KERNING: u32 = 1 << 6;
        const FT_FACE_FLAG_MULTIPLE_MASTERS: u32 = 1 << 8;
        const FT_FACE_FLAG_GLYPH_NAMES: u32 = 1 << 9;
        const FT_FACE_FLAG_HINTER: u32 = 1 << 11;

        let mut flags = FT_FACE_FLAG_SCALABLE | FT_FACE_FLAG_SFNT | FT_FACE_FLAG_HORIZONTAL;
        if self
            .data
            .post
            .as_ref()
            .is_some_and(|post| post.is_fixed_pitch != 0)
        {
            flags |= FT_FACE_FLAG_FIXED_WIDTH;
        }
        // sfobjs.c:1118-1121 exposes glyph names only if `tt_face_load_post`
        // accepted the `post` format, and format 3 intentionally has no names.
        if self
            .data
            .post
            .as_ref()
            .is_some_and(|post| matches!(post.format_type, 0x0001_0000 | 0x0002_0000 | 0x0002_5000))
        {
            flags |= FT_FACE_FLAG_GLYPH_NAMES;
        }
        if self.data.vhea.is_some() && self.data.vmtx.is_some() {
            flags |= FT_FACE_FLAG_VERTICAL;
        }
        if self.data.kern.as_ref().is_some_and(|kern| !kern.is_empty()) {
            flags |= FT_FACE_FLAG_KERNING;
        }
        if self.data.fvar.is_some() {
            flags |= FT_FACE_FLAG_MULTIPLE_MASTERS;
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
        // C normalizes a missing dimension from the other one, then clamps
        // both dimensions to 1..=0xFFFF (ftobjs.c:3574-3588).
        let mut width = pixel_width;
        let mut height = pixel_height;
        if width == 0 {
            width = height;
        } else if height == 0 {
            height = width;
        }
        width = width.clamp(1, 0xFFFF);
        height = height.clamp(1, 0xFFFF);
        self.size_pt = height as f32;
        self.size_metrics = SizeMetrics::from_pixel_size(width, height, &self.data);
        self.data.size_pt.set(self.size_pt);
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
        self.face_globals =
            crate::autohint::globals::FaceGlobals::new(self.data.clone(), self.is_italic);
        // `FT_Request_Size` invalidates the active size's prepared bytecode
        // state just like `FT_Set_Char_Size` and `FT_Set_Pixel_Sizes`.
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
        let index = default_unicode_charmap_index(&self.data.cmap);
        if index >= self.data.cmap.charmaps.len() {
            return Err(FontError::InvalidFont(
                "unicode charmap not found".to_string(),
            ));
        }
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
        self.glyph_index_hori_advance_26dot6(glyph)
    }

    pub(crate) fn glyph_index_hori_advance_26dot6(&self, glyph_index: u16) -> i32 {
        let advance = self.data.hmtx.get(glyph_index).advance_width as i32;
        ft_mul_fix(advance, self.size_metrics.x_scale)
    }

    pub(crate) fn glyph_index_hori_advance_16dot16(&self, glyph_index: u16) -> i32 {
        let advance = self.data.hmtx.get(glyph_index).advance_width as i32;
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
        let scaled = self.scale_glyph_for_metrics_default_with_mode_and_hdmx(
            glyph,
            native_hint_mode,
            use_hdmx,
        )?;
        Ok(self.slot_load_from_scaled(glyph, scaled, grid_fit_for_layout(vertical_layout)))
    }

    pub(crate) fn glyph_metrics_for_index_force_autohint(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        self.glyph_metrics_for_index_force_autohint_with_layout(glyph, false)
    }

    pub(crate) fn glyph_metrics_for_index_force_autohint_with_layout(
        &self,
        glyph: u16,
        vertical_layout: bool,
    ) -> Result<GlyphSlotMetrics, FontError> {
        self.glyph_metrics_for_index_force_autohint_with_layout_and_mode(
            glyph,
            vertical_layout,
            NativeHintMode::Normal,
        )
    }

    pub(crate) fn glyph_metrics_for_index_force_autohint_with_layout_and_mode(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
    ) -> Result<GlyphSlotMetrics, FontError> {
        Ok(self
            .glyph_slot_load_force_autohint_with_layout_and_mode(
                glyph,
                vertical_layout,
                native_hint_mode,
            )?
            .metrics)
    }

    pub(crate) fn glyph_slot_load_force_autohint_with_layout_and_mode(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
    ) -> Result<GlyphSlotLoad, FontError> {
        let metrics_cache = self.autohint_metrics_for_glyph(glyph);
        let scaled = scaler::scale_glyph_for_metrics_with_autohint_and_mode(
            &self.data,
            glyph,
            metrics_cache.as_deref(),
            self.is_italic,
            native_hint_mode,
        )?;
        Ok(self.slot_load_from_scaled(glyph, scaled, grid_fit_for_layout(vertical_layout)))
    }

    pub(crate) fn glyph_metrics_for_index_target_light_with_layout(
        &self,
        glyph: u16,
        _vertical_layout: bool,
    ) -> Result<GlyphSlotMetrics, FontError> {
        Ok(self.glyph_slot_load_target_light(glyph)?.metrics)
    }

    pub(crate) fn glyph_slot_load_target_light(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotLoad, FontError> {
        let metrics_cache = self.autohint_metrics_for_glyph(glyph);
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

    pub(crate) fn glyph_metrics_for_index_no_hinting(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        Ok(self.glyph_slot_load_no_hinting(glyph)?.metrics)
    }

    pub(crate) fn glyph_slot_load_no_hinting(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotLoad, FontError> {
        let scaled = scaler::scale_glyph_no_hinting(&self.data, glyph, self.is_italic)?;
        // C: `FT_Load_Glyph` calls `ft_glyphslot_grid_fit_metrics` only when
        // `FT_LOAD_NO_HINTING` is not set (`src/base/ftobjs.c`).  No-hinting
        // slot metrics keep the fractional 26.6 values from `ttgload.c`.
        Ok(self.slot_load_from_scaled(glyph, scaled, MetricsGridFit::None))
    }

    pub(crate) fn glyph_metrics_for_index_no_scale(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        Ok(self.glyph_slot_load_no_scale(glyph)?.metrics)
    }

    pub(crate) fn glyph_slot_load_no_scale(&self, glyph: u16) -> Result<GlyphSlotLoad, FontError> {
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

    pub(crate) fn glyph_slot_load_no_recurse(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotLoad, FontError> {
        let mut loaded = self.glyph_slot_load_no_scale(glyph)?;
        if self.glyph_is_composite(glyph)? {
            // C: FT_LOAD_NO_RECURSE leaves composite glyphs in
            // FT_GLYPH_FORMAT_COMPOSITE instead of resolving them to an
            // outline (`src/truetype/ttgload.c`).  Renderers then reject the
            // slot with Cannot_Render_Glyph.
            loaded.format = GlyphSlotLoadFormat::Composite;
            loaded.slot_outline = None;
            loaded.render_outline = None;
        }
        Ok(loaded)
    }

    pub(crate) fn glyph_metrics_for_index_no_autohint(
        &self,
        glyph: u16,
    ) -> Result<GlyphSlotMetrics, FontError> {
        self.glyph_metrics_for_index_no_autohint_with_layout(glyph, false)
    }

    pub(crate) fn glyph_metrics_for_index_no_autohint_with_layout(
        &self,
        glyph: u16,
        vertical_layout: bool,
    ) -> Result<GlyphSlotMetrics, FontError> {
        self.glyph_metrics_for_index_no_autohint_with_layout_and_mode(
            glyph,
            vertical_layout,
            NativeHintMode::Normal,
        )
    }

    pub(crate) fn glyph_metrics_for_index_no_autohint_with_layout_and_mode(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
    ) -> Result<GlyphSlotMetrics, FontError> {
        Ok(self
            .glyph_slot_load_no_autohint_with_layout_and_mode(
                glyph,
                vertical_layout,
                native_hint_mode,
            )?
            .metrics)
    }

    pub(crate) fn glyph_slot_load_no_autohint_with_layout_and_mode(
        &self,
        glyph: u16,
        vertical_layout: bool,
        native_hint_mode: NativeHintMode,
    ) -> Result<GlyphSlotLoad, FontError> {
        let scaled = self.scale_glyph_no_autohint_for_metrics_with_mode(glyph, native_hint_mode)?;
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
                scaler::scale_glyph_native_default_with_bytecode_context_and_mode(
                    &self.data,
                    glyph,
                    None,
                    self.is_italic,
                    native_hint_mode,
                    bytecode_context,
                )
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

    pub(crate) fn scale_glyph_for_metrics_default(
        &self,
        glyph: u16,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        self.scale_glyph_for_metrics_default_with_mode(glyph, NativeHintMode::Normal)
    }

    fn scale_glyph_for_metrics_default_with_mode(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        self.scale_glyph_for_metrics_default_with_mode_and_hdmx(glyph, native_hint_mode, true)
    }

    fn scale_glyph_for_metrics_default_with_mode_and_hdmx(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
        use_hdmx: bool,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if self.data.fpgm.is_some() && self.data.cvt.is_some() {
            let bytecode_context = self.native_bytecode_context_for_mode(native_hint_mode)?;
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
        self.scale_glyph_no_autohint_for_load_with_mode(glyph, NativeHintMode::Normal)
    }

    fn scale_glyph_no_autohint_for_load_with_mode(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if self.data.fpgm.is_some() && self.data.cvt.is_some() {
            let bytecode_context = self.native_bytecode_context_for_mode(native_hint_mode)?;
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

    fn scale_glyph_no_autohint_for_metrics(
        &self,
        glyph: u16,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        self.scale_glyph_no_autohint_for_metrics_with_mode(glyph, NativeHintMode::Normal)
    }

    fn scale_glyph_no_autohint_for_metrics_with_mode(
        &self,
        glyph: u16,
        native_hint_mode: NativeHintMode,
    ) -> Result<scaler::ScaledGlyph, FontError> {
        if self.data.fpgm.is_some() && self.data.cvt.is_some() {
            let bytecode_context = self.native_bytecode_context_for_mode(native_hint_mode)?;
            scaler::scale_glyph_for_metrics_with_bytecode_context_and_mode(
                &self.data,
                glyph,
                self.is_italic,
                native_hint_mode,
                bytecode_context,
            )
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
            let scale = tt::hinter::HintScale {
                x_scale: self.size_metrics.x_scale,
                y_scale: self.size_metrics.y_scale,
                tt_scale: self.size_metrics.tt_scale(),
                ppem: self.size_metrics.tt_ppem(),
                point_size: self.size_metrics.tt_point_size(),
                storage_size: self.data.maxp.max_storage as usize,
                twilight_points: self.data.maxp.max_twilight_points as usize,
                is_composite: false,
                reset_vectors_at_glyph_entry: false,
                metrics_legacy_phantoms: false,
                native_hint_mode: mode,
                phantom_x_override: None,
            };
            let prep = self.data.prep.as_deref().unwrap_or(&[]);
            let prepared = tt::hinter::prepare_context(cvt, fpgm, prep, &scale)?;
            let _ = slot.set(prepared);
        }
        Ok(slot.get())
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
        scaled: scaler::ScaledGlyph,
        grid_fit_metrics: MetricsGridFit,
    ) -> GlyphSlotMetrics {
        self.slot_load_from_scaled(glyph_index, scaled, grid_fit_metrics)
            .metrics
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
        if autohint_vertical.is_none() {
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

fn scaled_slot_outline(scaled: &scaler::ScaledGlyph) -> Outline {
    scaled_slot_outline_from_outline(
        &scaled.outline,
        scaled.outline_cbox_x_min,
        scaled.outline_cbox_y_min,
        scaled.outline_cbox_x_max,
        scaled.outline_cbox_y_max,
    )
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
        tags: Vec::new(),
        contour_dropouts: Vec::new(),
        flags: 0,
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
    let instance = fvar
        .as_ref()?
        .instances
        .get(named_instance.checked_sub(1)?)?;
    if let Some(name_id) = instance.postscript_name_id
        && let Some(name) = tt::name::name_string(name, name_id)
    {
        return Some(name);
    }
    let prefix = tt::name::variations_postscript_prefix(name)?;
    let subfamily = tt::name::name_string(name, instance.subfamily_name_id)?;
    let mut result = String::with_capacity(prefix.len() + 1 + subfamily.len());
    result.push_str(&prefix);
    result.push('-');
    result.extend(subfamily.chars().filter(|ch| ch.is_ascii_alphanumeric()));
    Some(result)
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

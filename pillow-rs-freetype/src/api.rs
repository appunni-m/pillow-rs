//! Safe Rust façade shaped after FreeType's common face/glyph-slot API.
//!
//! This module intentionally mirrors the concepts exposed by binding crates
//! such as Servo's `rust-freetype` without mirroring raw pointers or runtime
//! FFI.  A [`Face`] owns the parsed pure-Rust font, and [`GlyphSlot`] is an
//! immutable snapshot of the last loaded glyph data a C caller would inspect
//! through `FT_FaceRec::glyph`.

use crate::error::FontError;
use crate::font::{
    BBox, FaceInfo, Font, GlyphSlotLoad, GlyphSlotLoadFormat, GlyphSlotMetrics, KerningMode,
    LoadMode, LoadedOutline, SizeMetrics, SizeRequest, SizeRequestError,
};
use crate::render::{PixelMode, RenderMode, RenderedBitmap, render_loaded_outline};
use crate::tt::hinter::NativeHintMode;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// FreeType-style 2D vector in 26.6 pixel units unless documented otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vector {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}

/// Glyph image format loaded into a [`GlyphSlot`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphFormat {
    /// No outline or bitmap is present.
    None,
    /// Scalable outline data is present.
    Outline,
    /// A composite glyph slot is present but not recursively loaded.
    Composite,
    /// The glyph was rendered to a bitmap.
    Bitmap,
}

/// Bitflag-style glyph loading options aligned with common `FT_LOAD_*` flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadFlags(u32);

impl LoadFlags {
    /// Default native load behavior.
    pub const DEFAULT: Self = Self(0);
    /// Render the loaded glyph into [`GlyphSlot::bitmap`].
    pub const RENDER: Self = Self(1 << 0);
    /// Force the auto-hinter instead of native TrueType bytecode.
    pub const FORCE_AUTOHINT: Self = Self(1 << 1);
    /// Disable hinting for metrics and outline placement.
    pub const NO_HINTING: Self = Self(1 << 2);
    /// Render with monochrome coverage when [`Self::RENDER`] is also set.
    pub const TARGET_MONO: Self = Self(1 << 3);
    /// Render with horizontal LCD coverage when [`Self::RENDER`] is also set.
    pub const TARGET_LCD: Self = Self(1 << 4);
    /// Render with vertical LCD coverage when [`Self::RENDER`] is also set.
    pub const TARGET_LCD_V: Self = Self(1 << 5);
    /// Disable FreeType's auto-hinter while still allowing native hints.
    pub const NO_AUTOHINT: Self = Self(1 << 6);
    /// Load scalable glyphs in font units without scaling or rendering.
    pub const NO_SCALE: Self = Self(1 << 7);
    /// Load a composite glyph without resolving component subglyphs.
    pub const NO_RECURSE: Self = Self(1 << 11);
    /// Use vertical layout advances for the loaded glyph slot.
    pub const VERTICAL_LAYOUT: Self = Self(1 << 8);
    /// Use FreeType's light auto-hint target: vertical hinting only, gray render.
    pub const TARGET_LIGHT: Self = Self(1 << 9);
    /// Render as monochrome when the load target is normal.
    pub const MONOCHROME_RENDER: Self = Self(1 << 10);

    /// Return true if all bits in `other` are set.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    fn render_mode(self) -> RenderMode {
        if self.contains(Self::TARGET_MONO)
            || (self.contains(Self::MONOCHROME_RENDER)
                && !self.contains(Self::TARGET_LCD)
                && !self.contains(Self::TARGET_LCD_V)
                && !self.contains(Self::TARGET_LIGHT))
        {
            RenderMode::Mono
        } else if self.contains(Self::TARGET_LCD) {
            RenderMode::Lcd
        } else if self.contains(Self::TARGET_LCD_V) {
            RenderMode::LcdV
        } else {
            RenderMode::Normal
        }
    }

    fn native_hint_mode(self) -> NativeHintMode {
        if self.contains(Self::TARGET_MONO) {
            NativeHintMode::Mono
        } else if self.contains(Self::TARGET_LCD) {
            NativeHintMode::Lcd
        } else if self.contains(Self::TARGET_LCD_V) {
            NativeHintMode::LcdV
        } else {
            NativeHintMode::Normal
        }
    }

    pub(crate) fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl std::ops::BitOr for LoadFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for LoadFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A FreeType-style library handle.
///
/// The pure Rust implementation has no dynamic module registry, so this is a
/// zero-sized value used to align construction flow with `FT_Init_FreeType`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Library;

impl Library {
    /// Create a new pure-Rust library handle.
    pub fn init() -> Self {
        Self
    }

    /// Open a font face from memory, equivalent to `FT_New_Memory_Face`.
    pub fn new_memory_face(
        self,
        data: &[u8],
        face_index: usize,
        size_pt: f32,
    ) -> Result<Face, FontError> {
        let font = Font::truetype_face(data, face_index, size_pt)?;
        Ok(Face {
            font,
            render_fonts: RenderFontCache::default(),
        })
    }
}

/// A loaded font face.
#[derive(Clone)]
pub struct Face {
    font: Font,
    render_fonts: RenderFontCache,
}

#[derive(Clone, Default)]
struct RenderFontCache {
    fonts: Rc<RefCell<BTreeMap<RenderFontKey, Font>>>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RenderFontKey {
    load_mode: RenderLoadModeKey,
    size_pt_bits: u32,
    x_ppem: u16,
    y_ppem: u16,
    x_scale: i32,
    y_scale: i32,
    ascender: i32,
    descender: i32,
    height: i32,
    max_advance: i32,
    x_dpi: u32,
    y_dpi: u32,
    char_width: i32,
    char_height: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RenderLoadModeKey {
    Default,
    ForceAutoHint,
    TargetLight,
    NoHinting,
    NoAutoHint,
}

impl RenderFontCache {
    fn get_or_insert_with(&self, key: RenderFontKey, build: impl FnOnce() -> Font) -> Font {
        if let Some(font) = self.fonts.borrow().get(&key).cloned() {
            return font;
        }
        let font = build();
        self.fonts
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| font.clone())
            .clone()
    }
}

impl RenderFontKey {
    fn new(font: &Font, load_mode: LoadMode) -> Self {
        let metrics = font.size_metrics();
        Self {
            load_mode: RenderLoadModeKey::from(load_mode),
            size_pt_bits: font.size_pt.to_bits(),
            x_ppem: metrics.x_ppem,
            y_ppem: metrics.y_ppem,
            x_scale: metrics.x_scale,
            y_scale: metrics.y_scale,
            ascender: metrics.ascender,
            descender: metrics.descender,
            height: metrics.height,
            max_advance: metrics.max_advance,
            x_dpi: metrics.x_dpi,
            y_dpi: metrics.y_dpi,
            char_width: metrics.char_width,
            char_height: metrics.char_height,
        }
    }
}

impl From<LoadMode> for RenderLoadModeKey {
    fn from(load_mode: LoadMode) -> Self {
        match load_mode {
            LoadMode::Default => Self::Default,
            LoadMode::ForceAutoHint => Self::ForceAutoHint,
            LoadMode::TargetLight => Self::TargetLight,
            LoadMode::NoHinting => Self::NoHinting,
            LoadMode::NoAutoHint => Self::NoAutoHint,
        }
    }
}

impl Face {
    /// Open a font face directly from bytes.
    pub fn from_memory(data: &[u8], face_index: usize, size_pt: f32) -> Result<Self, FontError> {
        Library::init().new_memory_face(data, face_index, size_pt)
    }

    /// Return the underlying high-level font object.
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// Return the active charmap index when the face has selectable charmaps.
    pub fn charmap_index(&self) -> Option<usize> {
        self.font.charmap_index()
    }

    /// Return scalar face metadata.
    pub fn info(&self) -> FaceInfo {
        self.font.face_info()
    }

    /// Return active size metrics.
    pub fn size_metrics(&self) -> SizeMetrics {
        self.font.size_metrics()
    }

    /// Set the active character size, equivalent to `FT_Set_Char_Size`.
    pub fn set_char_size(&mut self, char_width: i32, char_height: i32, x_dpi: u32, y_dpi: u32) {
        self.font
            .set_char_size(char_width, char_height, x_dpi, y_dpi);
    }

    pub(crate) fn try_set_char_size(
        &mut self,
        char_width: i32,
        char_height: i32,
        x_dpi: u32,
        y_dpi: u32,
    ) -> Result<(), SizeRequestError> {
        self.font
            .try_set_char_size(char_width, char_height, x_dpi, y_dpi)
    }

    /// Set the active pixel size, equivalent to `FT_Set_Pixel_Sizes`.
    pub fn set_pixel_sizes(&mut self, pixel_width: u32, pixel_height: u32) {
        self.font.set_pixel_sizes(pixel_width, pixel_height);
    }

    /// Request the active size, equivalent to `FT_Request_Size`.
    pub fn request_size(&mut self, request: SizeRequest) -> Result<(), SizeRequestError> {
        self.font.request_size(request)
    }

    /// Return the glyph index for a Unicode scalar value.
    pub fn get_char_index(&self, char_code: u32) -> u16 {
        self.font.char_index(char_code)
    }

    /// Select the best Unicode charmap, equivalent to `FT_Select_Charmap`.
    pub fn select_unicode_charmap(&mut self) -> Result<(), FontError> {
        self.font.select_unicode_charmap()
    }

    /// Set the active charmap by face-owned charmap index, equivalent to `FT_Set_Charmap`.
    pub fn set_charmap(&mut self, index: usize) -> Result<(), FontError> {
        self.font.set_charmap(index)
    }

    /// Return OS/2 embedding permission flags, equivalent to `FT_Get_FSType_Flags`.
    pub fn get_fstype_flags(&self) -> u16 {
        self.font.get_fstype_flags()
    }

    /// Return kerning vector for two glyph indexes, equivalent to `FT_Get_Kerning`.
    pub fn kerning_by_glyphs(&self, left: u32, right: u32, mode: KerningMode) -> Vector {
        let (x, y) = self.font.kerning_by_glyphs(left, right, mode);
        Vector { x, y }
    }

    /// Return the number of raw SFNT name records.
    pub fn sfnt_name_count(&self) -> usize {
        self.font.sfnt_name_count()
    }

    /// Return one raw SFNT name record by index.
    pub fn sfnt_name(&self, index: usize) -> Option<&crate::tt::name::SfntNameRecord> {
        self.font.sfnt_name(index)
    }

    /// Return the first mapped character and glyph index for the active charmap.
    pub fn first_char(&self) -> Option<(u32, u16)> {
        self.font.first_char()
    }

    /// Return the next mapped character and glyph index after `char_code`.
    pub fn next_char(&self, char_code: u32) -> Option<(u32, u16)> {
        self.font.next_char(char_code)
    }

    pub(crate) fn glyph_hori_advance_16dot16(&self, glyph_index: u16) -> i32 {
        self.font.glyph_index_hori_advance_16dot16(glyph_index)
    }

    /// Load a Unicode scalar value, equivalent to `FT_Load_Char`.
    pub fn load_char(&self, char_code: u32, flags: LoadFlags) -> Result<GlyphSlot, FontError> {
        self.load_glyph(self.get_char_index(char_code), flags)
    }

    /// Load a glyph index, equivalent to `FT_Load_Glyph`.
    pub fn load_glyph(&self, glyph_index: u16, flags: LoadFlags) -> Result<GlyphSlot, FontError> {
        let vertical_layout = flags.contains(LoadFlags::VERTICAL_LAYOUT);
        let native_hint_mode = flags.native_hint_mode();
        let loaded = if flags.contains(LoadFlags::NO_RECURSE) {
            self.font.glyph_slot_load_no_recurse(glyph_index)?
        } else if flags.contains(LoadFlags::NO_SCALE) {
            self.font.glyph_slot_load_no_scale(glyph_index)?
        } else if flags.contains(LoadFlags::NO_HINTING) {
            self.font.glyph_slot_load_no_hinting(glyph_index)?
        } else if flags.contains(LoadFlags::TARGET_LIGHT) && !flags.contains(LoadFlags::NO_AUTOHINT)
        {
            self.font.glyph_slot_load_target_light(glyph_index)?
        } else if flags.contains(LoadFlags::FORCE_AUTOHINT)
            && !flags.contains(LoadFlags::NO_AUTOHINT)
        {
            self.font
                .glyph_slot_load_force_autohint_with_layout_and_mode(
                    glyph_index,
                    vertical_layout,
                    native_hint_mode,
                )?
        } else if flags.contains(LoadFlags::NO_AUTOHINT) {
            self.font.glyph_slot_load_no_autohint_with_layout_and_mode(
                glyph_index,
                vertical_layout,
                native_hint_mode,
            )?
        } else {
            self.font.glyph_slot_load_default_with_layout_and_mode(
                glyph_index,
                vertical_layout,
                native_hint_mode,
            )?
        };

        let render_requested = flags.contains(LoadFlags::RENDER);
        let bitmap = if render_requested {
            let render_font = self.render_font(flags)?;
            let bitmap = render_font.render_char_mode_for_index_with_native_hint_mode(
                glyph_index,
                flags.render_mode(),
                native_hint_mode,
            )?;
            if bitmap.buffer.is_empty() {
                None
            } else {
                Some(bitmap)
            }
        } else {
            None
        };

        Ok(GlyphSlot::new(
            glyph_index,
            loaded,
            bitmap,
            vertical_layout,
            render_requested,
        ))
    }

    pub fn render_loaded_glyph(
        &self,
        glyph_index: u16,
        load_flags: LoadFlags,
        mode: RenderMode,
    ) -> Result<GlyphSlot, FontError> {
        let load_only_flags = load_flags.without(LoadFlags::RENDER);
        self.load_glyph(glyph_index, load_only_flags)?.render(mode)
    }

    fn render_font(&self, flags: LoadFlags) -> Result<Font, FontError> {
        let load_mode = if flags.contains(LoadFlags::NO_HINTING) {
            LoadMode::NoHinting
        } else if flags.contains(LoadFlags::TARGET_LIGHT) && !flags.contains(LoadFlags::NO_AUTOHINT)
        {
            LoadMode::TargetLight
        } else if flags.contains(LoadFlags::FORCE_AUTOHINT) {
            if flags.contains(LoadFlags::NO_AUTOHINT) {
                LoadMode::NoAutoHint
            } else {
                LoadMode::ForceAutoHint
            }
        } else if flags.contains(LoadFlags::NO_AUTOHINT) {
            LoadMode::NoAutoHint
        } else {
            LoadMode::Default
        };
        let key = RenderFontKey::new(&self.font, load_mode);
        Ok(self
            .render_fonts
            .get_or_insert_with(key, || self.font.clone_with_load_mode(load_mode)))
    }
}

/// Snapshot of the loaded glyph-slot fields callers normally read from
/// `FT_GlyphSlotRec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphSlot {
    /// Glyph index loaded into the slot.
    pub glyph_index: u16,
    /// Grid-fit glyph metrics in 26.6 pixels.
    pub metrics: GlyphSlotMetrics,
    /// Slot advance vector in 26.6 pixels.
    pub advance: Vector,
    /// Glyph image format currently present in the slot.
    pub format: GlyphFormat,
    /// Rendered bitmap if `LoadFlags::RENDER` was requested.
    pub bitmap: Option<RenderedBitmap>,
    /// Bitmap left bearing in pixels.
    pub bitmap_left: i32,
    /// Bitmap top bearing in pixels.
    pub bitmap_top: i32,
    /// Exact `FT_Outline_Get_CBox` result for the loaded outline.
    pub outline_cbox: BBox,
    /// Exact `FT_Outline_Get_BBox` result for the loaded outline.
    pub outline_bbox: BBox,
    slot_outline: Option<crate::outline::Outline>,
    loaded_outline: Option<LoadedOutline>,
}

impl GlyphSlot {
    fn new(
        glyph_index: u16,
        loaded: GlyphSlotLoad,
        bitmap: Option<RenderedBitmap>,
        vertical_layout: bool,
        rendered: bool,
    ) -> Self {
        let metrics = loaded.metrics;
        let slot_outline = loaded.slot_outline;
        let loaded_outline = loaded.render_outline;
        let format = if rendered {
            GlyphFormat::Bitmap
        } else {
            match loaded.format {
                GlyphSlotLoadFormat::Outline => GlyphFormat::Outline,
                GlyphSlotLoadFormat::Composite => GlyphFormat::Composite,
            }
        };
        let (bitmap_left, bitmap_top) = bitmap
            .as_ref()
            .map_or((0, 0), |bitmap| (bitmap.left, bitmap.top));

        Self {
            glyph_index,
            metrics,
            advance: if vertical_layout {
                Vector {
                    x: 0,
                    y: metrics.vert_advance,
                }
            } else {
                Vector {
                    x: metrics.hori_advance,
                    y: 0,
                }
            },
            format,
            bitmap,
            bitmap_left,
            bitmap_top,
            outline_cbox: loaded.outline_cbox,
            outline_bbox: loaded.outline_bbox,
            slot_outline,
            loaded_outline,
        }
    }

    /// Return the rendered bitmap's pixel mode, if a bitmap is present.
    pub fn pixel_mode(&self) -> Option<PixelMode> {
        self.bitmap.as_ref().map(|bitmap| bitmap.pixel_mode)
    }

    fn set_rendered_bitmap(&mut self, bitmap: RenderedBitmap) {
        if bitmap.buffer.is_empty() {
            self.bitmap_left = 0;
            self.bitmap_top = 0;
            self.bitmap = None;
        } else {
            self.bitmap_left = bitmap.left;
            self.bitmap_top = bitmap.top;
            self.bitmap = Some(bitmap);
        }
        self.format = GlyphFormat::Bitmap;
    }

    pub(crate) fn render(mut self, mode: RenderMode) -> Result<Self, FontError> {
        if self.format == GlyphFormat::Bitmap {
            return Ok(self);
        }
        if self.format == GlyphFormat::Composite {
            return Err(FontError::CannotRenderGlyph(
                "composite glyph slot cannot be rendered".to_string(),
            ));
        }
        let Some(loaded) = self.loaded_outline.clone() else {
            return Err(FontError::InvalidOutline(
                "loaded glyph slot has no outline snapshot".to_string(),
            ));
        };
        let bitmap =
            render_loaded_outline(loaded.outline, loaded.left, loaded.bottom, loaded.top, mode)?;
        self.set_rendered_bitmap(bitmap);
        Ok(self)
    }

    pub(crate) fn slot_outline(&self) -> Option<&crate::outline::Outline> {
        self.slot_outline.as_ref()
    }
}

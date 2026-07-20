//! Safe Rust façade shaped after FreeType's common face/glyph-slot API.
//!
//! This module intentionally mirrors the concepts exposed by binding crates
//! such as Servo's `rust-freetype` without mirroring raw pointers or runtime
//! FFI.  A [`Face`] owns the parsed pure-Rust font, and [`GlyphSlot`] is an
//! immutable snapshot of the last loaded glyph data a C caller would inspect
//! through `FT_FaceRec::glyph`.

use crate::error::FontError;
use crate::font::{
    ActiveSizeState, BBox, FaceInfo, Font, GlyphSlotLoad, GlyphSlotLoadFormat, GlyphSlotMetrics,
    KerningMode, LoadMode, LoadedOutline, SelectSizeError, SizeMetrics, SizeRequest,
    SizeRequestError, SubGlyphInfo,
};
use crate::render::{PixelMode, RenderMode, RenderedBitmap, render_loaded_outline};
use crate::tt::hinter::NativeHintMode;
use crate::tt::sbit::SbitPixelMode;
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
    /// Recompute scalable glyph metrics without device-width tables.
    pub const COMPUTE_METRICS: Self = Self(1 << 12);
    /// Load only embedded bitmap strikes, equivalent to `FT_LOAD_SBITS_ONLY`.
    pub const SBITS_ONLY: Self = Self(1 << 13);
    /// Disable embedded bitmap strikes, equivalent to `FT_LOAD_NO_BITMAP`.
    pub const NO_BITMAP: Self = Self(1 << 14);
    /// Return TrueType glyph-program errors instead of silently ignoring them.
    pub const PEDANTIC: Self = Self(1 << 15);
    /// Load embedded bitmap metrics without rendering bitmap bytes.
    pub const BITMAP_METRICS_ONLY: Self = Self(1 << 16);

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
        let font = Font::memory_face(data, face_index, size_pt)?;
        Ok(Face {
            font,
            render_fonts: RenderFontCache::default(),
        })
    }

    /// Open a font face with the name-selection options normally carried by
    /// `FT_Open_Face` parameters.
    pub fn new_memory_face_with_name_options(
        self,
        data: &[u8],
        face_index: usize,
        size_pt: f32,
        ignore_typographic_family: bool,
        ignore_typographic_subfamily: bool,
    ) -> Result<Face, FontError> {
        let mut face = self.new_memory_face(data, face_index, size_pt)?;
        face.font
            .set_ignore_typographic_names(ignore_typographic_family, ignore_typographic_subfamily);
        Ok(face)
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

    fn clear(&self) {
        self.fonts.borrow_mut().clear();
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

    pub(crate) fn font_mut(&mut self) -> &mut Font {
        &mut self.font
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

    pub(crate) fn active_size_state(&self) -> ActiveSizeState {
        self.font.active_size_state()
    }

    pub(crate) fn activate_size_state(&mut self, state: &ActiveSizeState) {
        self.font.activate_size_state(state);
        self.render_fonts.clear();
    }

    pub(crate) fn reset_size_to_undefined(&mut self) {
        self.font.reset_size_to_undefined();
        self.render_fonts.clear();
    }

    pub(crate) fn reset_probe_size_request_metrics(&mut self) {
        self.font.reset_probe_size_request_metrics();
        self.render_fonts.clear();
    }

    /// Set the active character size, equivalent to `FT_Set_Char_Size`.
    pub fn set_char_size(&mut self, char_width: i32, char_height: i32, x_dpi: u32, y_dpi: u32) {
        self.font
            .set_char_size(char_width, char_height, x_dpi, y_dpi);
        self.render_fonts.clear();
    }

    pub(crate) fn try_set_char_size(
        &mut self,
        char_width: i32,
        char_height: i32,
        x_dpi: u32,
        y_dpi: u32,
    ) -> Result<(), SizeRequestError> {
        self.font
            .try_set_char_size(char_width, char_height, x_dpi, y_dpi)?;
        self.render_fonts.clear();
        Ok(())
    }

    /// Set the active pixel size, equivalent to `FT_Set_Pixel_Sizes`.
    pub fn set_pixel_sizes(&mut self, pixel_width: u32, pixel_height: u32) {
        self.font.set_pixel_sizes(pixel_width, pixel_height);
        self.render_fonts.clear();
    }

    /// Request the active size, equivalent to `FT_Request_Size`.
    pub fn request_size(&mut self, request: SizeRequest) -> Result<(), SizeRequestError> {
        self.font.request_size(request)?;
        self.render_fonts.clear();
        Ok(())
    }

    /// Select an embedded bitmap strike, equivalent to `FT_Select_Size`.
    pub fn select_size(&mut self, strike_index: usize) -> Result<(), SelectSizeError> {
        self.font.select_size(strike_index)?;
        self.render_fonts.clear();
        Ok(())
    }

    /// Return the glyph index for a Unicode scalar value.
    pub fn get_char_index(&self, char_code: u32) -> u16 {
        self.font.char_index(char_code)
    }

    /// Return the glyph index for a Unicode variation-selector pair.
    pub fn get_char_variant_index(&self, char_code: u32, variant_selector: u32) -> u16 {
        self.font.char_variant_index(char_code, variant_selector)
    }

    /// Return whether a Unicode variation-selector pair uses the default glyph.
    pub fn get_char_variant_is_default(&self, char_code: u32, variant_selector: u32) -> i32 {
        self.font
            .char_variant_is_default(char_code, variant_selector)
    }

    /// Return Unicode variation selectors found in the face.
    pub fn get_variant_selectors(&self) -> Option<Vec<u32>> {
        self.font.variant_selectors()
    }

    /// Return variation selectors active for a Unicode scalar value.
    pub fn get_variants_of_char(&self, char_code: u32) -> Option<Vec<u32>> {
        self.font.variants_of_char(char_code)
    }

    /// Return Unicode scalar values covered by a variation selector.
    pub fn get_chars_of_variant(&self, variant_selector: u32) -> Option<Vec<u32>> {
        self.font.chars_of_variant(variant_selector)
    }

    /// Select the best Unicode charmap, equivalent to `FT_Select_Charmap`.
    pub fn select_unicode_charmap(&mut self) -> Result<(), FontError> {
        self.font.select_unicode_charmap()?;
        self.render_fonts.clear();
        Ok(())
    }

    /// Set the active charmap by face-owned charmap index, equivalent to `FT_Set_Charmap`.
    pub fn set_charmap(&mut self, index: usize) -> Result<(), FontError> {
        self.font.set_charmap(index)?;
        self.render_fonts.clear();
        Ok(())
    }

    /// Return OS/2 embedding permission flags, equivalent to `FT_Get_FSType_Flags`.
    pub fn get_fstype_flags(&self) -> u16 {
        self.font.get_fstype_flags()
    }

    /// Return `gasp` table flags for a ppem, equivalent to `FT_Get_Gasp`.
    pub fn get_gasp(&self, ppem: u32) -> i32 {
        self.font.get_gasp(ppem)
    }

    /// Return the parsed Windows FNT header, equivalent to `FT_Get_WinFNT_Header`.
    pub fn winfnt_header(&self) -> Option<&crate::font::WinFntHeader> {
        self.font.winfnt_header()
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

    /// Return the raw SFNT name table format field.
    pub fn sfnt_name_format(&self) -> u16 {
        self.font.sfnt_name_format()
    }

    /// Return one raw SFNT language-tag record by index.
    pub fn sfnt_lang_tag(&self, index: usize) -> Option<&crate::tt::name::SfntLangTagRecord> {
        self.font.sfnt_lang_tag(index)
    }

    /// Return the face PostScript name, equivalent to `FT_Get_Postscript_Name`.
    pub fn postscript_name(&self) -> Option<&str> {
        self.font.postscript_name()
    }

    /// Set or clear the current named instance, equivalent to `FT_Set_Named_Instance`.
    pub fn set_named_instance(&mut self, instance_index: usize) -> Result<(), FontError> {
        self.font.set_named_instance(instance_index)?;
        self.render_fonts.clear();
        Ok(())
    }

    /// Set explicit OpenType design coordinates, equivalent to
    /// `FT_Set_Var_Design_Coordinates`.
    pub(crate) fn set_var_design_coordinates(&mut self, coords: &[i32]) -> Result<(), FontError> {
        self.font.set_var_design_coordinates(coords)?;
        self.render_fonts.clear();
        Ok(())
    }

    /// Return active OpenType design coordinates, equivalent to
    /// `FT_Get_Var_Design_Coordinates`.
    pub(crate) fn var_design_coordinates(&self) -> Result<&[i32], FontError> {
        self.font.var_design_coordinates()
    }

    /// Return active normalized blend coordinates in FreeType's 16.16 public
    /// representation.
    pub(crate) fn var_blend_coordinates_16_16(&self) -> Result<Vec<i32>, FontError> {
        self.font.var_blend_coordinates_16_16()
    }

    /// Set normalized blend coordinates, equivalent to
    /// `FT_Set_MM_Blend_Coordinates` / `FT_Set_Var_Blend_Coordinates`.
    pub(crate) fn set_var_blend_coordinates(
        &mut self,
        coords_16_16: &[i32],
    ) -> Result<(), FontError> {
        self.font.set_var_blend_coordinates(coords_16_16)?;
        self.render_fonts.clear();
        Ok(())
    }

    pub(crate) fn set_type1_mm_blend_coordinates(
        &mut self,
        coords_16_16: &[i32],
        variation_active: bool,
    ) -> Result<(), FontError> {
        self.font
            .set_type1_mm_blend_coordinates(coords_16_16, variation_active)?;
        self.render_fonts.clear();
        Ok(())
    }

    /// Return a glyph's PostScript name when the face exposes glyph names.
    pub fn glyph_name(&self, glyph_index: u32) -> Option<&str> {
        self.font.glyph_name(glyph_index)
    }

    /// Return the first glyph index with the given PostScript name.
    pub fn name_index(&self, glyph_name: &str) -> u32 {
        self.font.name_index(glyph_name)
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
    /// Optionally apply a 2×2 transform matrix after loading.
    pub fn load_glyph(&self, glyph_index: u16, flags: LoadFlags) -> Result<GlyphSlot, FontError> {
        self.load_glyph_with_transform(glyph_index, flags, None)
    }

    /// Load a glyph index with an optional transform, equivalent to
    /// `FT_Load_Glyph` + `FT_Set_Transform`.
    pub fn load_glyph_with_transform(
        &self,
        glyph_index: u16,
        flags: LoadFlags,
        transform: Option<(i32, i32, i32, i32, i32, i32)>,
    ) -> Result<GlyphSlot, FontError> {
        Self::load_glyph_from_font(&self.font, glyph_index, flags, transform)
    }

    fn normalize_load_flags(font: &Font, mut flags: LoadFlags) -> LoadFlags {
        let metrics = font.size_metrics();
        // FreeType resolves these dependencies in `FT_Load_Glyph` before driver
        // load (`src/base/ftobjs.c:932-952`).  Keep this in core so Rust, C ABI,
        // and WASM public entry points all exercise the same policy.
        if metrics.x_ppem == 0 || metrics.y_ppem == 0 {
            flags |= LoadFlags::NO_SCALE;
        }
        if flags.contains(LoadFlags::NO_RECURSE) {
            flags |= LoadFlags::NO_SCALE;
        }
        if flags.contains(LoadFlags::NO_SCALE) {
            flags |= LoadFlags::NO_HINTING | LoadFlags::NO_BITMAP;
            flags = flags.without(LoadFlags::RENDER);
        }
        if flags.contains(LoadFlags::BITMAP_METRICS_ONLY) {
            flags = flags.without(LoadFlags::RENDER);
        }
        flags
    }

    fn load_glyph_from_font(
        font: &Font,
        glyph_index: u16,
        mut flags: LoadFlags,
        transform: Option<(i32, i32, i32, i32, i32, i32)>,
    ) -> Result<GlyphSlot, FontError> {
        flags = Self::normalize_load_flags(font, flags);
        let transform = if flags.contains(LoadFlags::NO_RECURSE) {
            None
        } else {
            transform
        };
        let vertical_layout = flags.contains(LoadFlags::VERTICAL_LAYOUT);
        let native_hint_mode = flags.native_hint_mode();
        let pedantic_hinting = flags.contains(LoadFlags::PEDANTIC);
        let sbits_only = flags.contains(LoadFlags::SBITS_ONLY);
        let sbit_allowed =
            !flags.contains(LoadFlags::NO_SCALE) && !flags.contains(LoadFlags::NO_BITMAP);
        if sbits_only && !sbit_allowed {
            return Err(FontError::InvalidArgument(
                "embedded bitmap strike not selected".into(),
            ));
        }
        if sbit_allowed {
            // C `FT_Load_Glyph` first tries SVG, then embedded bitmaps before
            // outline loading (`base/ftobjs.c:1028-1050`). The TrueType driver
            // repeats that SBIT attempt in `truetype/ttgload.c:2401-2474`.
            match font.load_sbit_only_glyph(glyph_index) {
                Ok(sbit) => return Ok(sbit_glyph_slot(glyph_index, sbit, vertical_layout)),
                Err(_) if sbits_only => {
                    // For scalable TrueType faces with `FT_LOAD_SBITS_ONLY`,
                    // failed SBIT loading is replaced with Invalid_Argument
                    // (`truetype/ttgload.c:2467-2474`).
                    return Err(FontError::InvalidArgument(
                        "embedded bitmap image not available".into(),
                    ));
                }
                Err(_) => {}
            }
        }
        let mut loaded = if flags.contains(LoadFlags::NO_RECURSE) {
            font.glyph_slot_load_no_recurse(glyph_index)?
        } else if flags.contains(LoadFlags::NO_SCALE) {
            font.glyph_slot_load_no_scale_with_layout(glyph_index, vertical_layout)?
        } else if flags.contains(LoadFlags::NO_HINTING) {
            font.glyph_slot_load_no_hinting(glyph_index)?
        } else if flags.contains(LoadFlags::TARGET_LIGHT) && !flags.contains(LoadFlags::NO_AUTOHINT)
        {
            font.glyph_slot_load_target_light(glyph_index)?
        } else if flags.contains(LoadFlags::FORCE_AUTOHINT)
            && !flags.contains(LoadFlags::NO_AUTOHINT)
        {
            font.glyph_slot_load_force_autohint_with_layout_and_mode(
                glyph_index,
                vertical_layout,
                native_hint_mode,
            )?
        } else if flags.contains(LoadFlags::NO_AUTOHINT) {
            font.glyph_slot_load_no_autohint_with_layout_and_mode_and_pedantic(
                glyph_index,
                vertical_layout,
                native_hint_mode,
                pedantic_hinting,
            )?
        } else {
            // C `tt_loader_init` suppresses `size->widthp` when
            // `FT_LOAD_COMPUTE_METRICS` is set (ttgload.c:2299-2305).
            font.glyph_slot_load_default_with_layout_and_mode_and_hdmx_and_pedantic(
                glyph_index,
                vertical_layout,
                native_hint_mode,
                !flags.contains(LoadFlags::COMPUTE_METRICS),
                pedantic_hinting,
            )?
        };

        let render_requested = flags.contains(LoadFlags::RENDER);
        // Only extract render outline when we're going to render.
        // Otherwise leave it in `loaded` for potential later FT_Render_Glyph calls.
        let mut render_lo = if render_requested {
            loaded.render_outline.take()
        } else {
            None
        };

        // Build the slot (without render_outline — we handle rendering below).
        let mut slot = GlyphSlot::new(glyph_index, loaded, None, vertical_layout, false);

        // Apply transform to slot AND extracted outline before rendering.
        if let Some((xx, xy, yx, yy, dx, dy)) = transform {
            slot.apply_transform(xx, xy, yx, yy, dx, dy);
            if let Some(lo) = render_lo.as_mut() {
                transform_loaded_outline_for_render(lo, xx, xy, yx, yy, dx, dy);
            }
        }

        // Render without cloning the outline — takes ownership.
        if render_requested {
            if let Some(lo) = render_lo {
                let mode = flags.render_mode();
                let mut scratch = font.raster_scratch.borrow_mut();
                let bmp = crate::render::render_loaded_outline(
                    lo.outline,
                    lo.left,
                    lo.bottom,
                    lo.top,
                    mode,
                    &mut scratch,
                )?;
                drop(scratch);
                slot.set_rendered_bitmap(bmp);
            }
        }

        Ok(slot)
    }

    pub fn render_loaded_glyph(
        &self,
        glyph_index: u16,
        load_flags: LoadFlags,
        mode: RenderMode,
    ) -> Result<GlyphSlot, FontError> {
        let load_only_flags = load_flags.without(LoadFlags::RENDER);
        let font = self.render_font(load_only_flags)?;
        Self::load_glyph_from_font(&font, glyph_index, load_only_flags, None)?.render(mode)
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

fn sbit_glyph_slot(
    glyph_index: u16,
    sbit: crate::tt::sbit::SbitGlyph,
    vertical_layout: bool,
) -> GlyphSlot {
    let metrics = sbit.metrics;
    let (bitmap_left, bitmap_top) = if vertical_layout {
        (metrics.vert_bearing_x / 64, metrics.vert_bearing_y / 64)
    } else {
        (metrics.hori_bearing_x / 64, metrics.hori_bearing_y / 64)
    };
    let bitmap = RenderedBitmap {
        width: sbit.bitmap.width,
        rows: sbit.bitmap.rows,
        pitch: sbit.bitmap.pitch,
        pixel_mode: sbit_pixel_mode_to_render(sbit.bitmap.pixel_mode),
        num_grays: sbit.bitmap.num_grays,
        left: bitmap_left,
        top: bitmap_top,
        buffer: sbit.bitmap.buffer,
    };
    let zero_bbox = BBox {
        x_min: 0,
        y_min: 0,
        x_max: 0,
        y_max: 0,
    };
    let loaded = GlyphSlotLoad {
        metrics: GlyphSlotMetrics {
            width: metrics.width,
            height: metrics.height,
            hori_bearing_x: metrics.hori_bearing_x,
            hori_bearing_y: metrics.hori_bearing_y,
            hori_advance: metrics.hori_advance,
            vert_bearing_x: metrics.vert_bearing_x,
            vert_bearing_y: metrics.vert_bearing_y,
            vert_advance: metrics.vert_advance,
        },
        format: GlyphSlotLoadFormat::Outline,
        outline_cbox: zero_bbox,
        outline_bbox: zero_bbox,
        subglyphs: Vec::new(),
        slot_outline: None,
        render_outline: None,
    };
    GlyphSlot::new(glyph_index, loaded, Some(bitmap), vertical_layout, true)
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
    /// Composite subglyph rows when the slot format is [`GlyphFormat::Composite`].
    pub subglyphs: Vec<SubGlyphInfo>,
    slot_outline: Option<crate::outline::Outline>,
    loaded_outline: Option<LoadedOutline>,
}

impl GlyphSlot {
    pub(crate) fn empty() -> Self {
        // C FreeType initializes `face->glyph` during face creation
        // (`src/base/ftobjs.c`, glyph-slot allocation path).  Before any
        // successful load, public callers observe format NONE and zeroed slot
        // fields; failed loads preserve that empty slot.
        Self {
            glyph_index: 0,
            metrics: GlyphSlotMetrics {
                width: 0,
                height: 0,
                hori_bearing_x: 0,
                hori_bearing_y: 0,
                hori_advance: 0,
                vert_bearing_x: 0,
                vert_bearing_y: 0,
                vert_advance: 0,
            },
            advance: Vector { x: 0, y: 0 },
            format: GlyphFormat::None,
            bitmap: None,
            bitmap_left: 0,
            bitmap_top: 0,
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
            slot_outline: None,
            loaded_outline: None,
        }
    }

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
        let subglyphs = loaded.subglyphs;
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
            subglyphs,
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
            if mode == RenderMode::Sdf {
                let Some(bitmap) = self.bitmap.take() else {
                    return Ok(self);
                };
                self.set_rendered_bitmap(crate::render::render_bitmap_sdf(bitmap)?);
            }
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
        let mut scratch = crate::grays::RasterScratch::new();
        let bitmap = render_loaded_outline(
            loaded.outline,
            loaded.left,
            loaded.bottom,
            loaded.top,
            mode,
            &mut scratch,
        )?;
        self.set_rendered_bitmap(bitmap);
        Ok(self)
    }

    /// Apply a 2×2 transform matrix to advance, outline, and bbox.
    /// Mirrors C's `ft_glyphslot_grid_fit_metrics` in `src/base/ftobjs.c`.
    /// NOTE: C does NOT transform metrics width/height/bearings/advance
    /// for FT_LOAD_DEFAULT with a user-space transform.
    pub fn apply_transform(&mut self, xx: i32, xy: i32, yx: i32, yy: i32, dx: i32, dy: i32) {
        // Transform the advance vector (matching C).
        {
            let ft_mul = crate::fixed::ft_mul_fix;
            let a = &mut self.advance;
            let (ax, ay) = (
                ft_mul(a.x, xx) + ft_mul(a.y, xy),
                ft_mul(a.x, yx) + ft_mul(a.y, yy),
            );
            a.x = ax;
            a.y = ay;
        }
        self.apply_outline_transform(xx, xy, yx, yy, dx, dy);
    }

    /// Apply a 2x2 transform to outline snapshots without changing metrics or advance.
    pub(crate) fn apply_outline_transform(
        &mut self,
        xx: i32,
        xy: i32,
        yx: i32,
        yy: i32,
        dx: i32,
        dy: i32,
    ) {
        if let Some(ref mut outline) = self.slot_outline {
            transform_outline_points(&mut outline.points, xx, xy, yx, yy, dx, dy);
        }
        if let Some(ref mut lo) = self.loaded_outline {
            transform_loaded_outline_for_render(lo, xx, xy, yx, yy, dx, dy);
        }
        self.recompute_outline_boxes();
    }

    /// Apply FreeType's synthetic outline emboldening and slot metric side effects.
    pub(crate) fn adjust_outline_weight(&mut self, xstrength: i32, ystrength: i32) {
        if let Some(ref mut outline) = self.slot_outline {
            embolden_outline(outline, xstrength, ystrength);
        }
        if let Some(ref mut loaded) = self.loaded_outline {
            embolden_loaded_outline_for_render(loaded, xstrength, ystrength);
        }

        self.apply_synthetic_weight_metrics(xstrength, ystrength);
        self.recompute_outline_boxes();
    }

    /// Apply FreeType's synthetic bitmap-slot emboldening and slot metric side effects.
    pub(crate) fn adjust_bitmap_weight(&mut self, mut xstrength: i64, mut ystrength: i64) {
        let Some(ref mut bitmap) = self.bitmap else {
            return;
        };

        // FreeType `src/base/ftsynth.c` rounds bitmap slot strengths down to
        // full pixels before calling FT_Bitmap_Embolden, and forces a minimum
        // one-pixel horizontal embolden for zero or subpixel x strength.
        xstrength &= !63;
        if xstrength == 0 {
            xstrength = 1 << 6;
        }
        ystrength &= !63;

        let x_pixels = xstrength >> 6;
        let y_pixels = ystrength >> 6;
        // C `FT_GlyphSlot_AdjustWeight` checks vertical `FT_Int` range before
        // ownership, then `FT_Bitmap_Embolden` rejects either positive pixel
        // count above `FT_INT_MAX` and all negative strengths
        // (`src/base/ftsynth.c:137-160`, `src/base/ftbitmap.c:302-317`).
        if y_pixels < i64::from(i32::MIN)
            || x_pixels > i64::from(i32::MAX)
            || y_pixels > i64::from(i32::MAX)
            || x_pixels < 0
            || y_pixels < 0
        {
            return;
        }
        let (x_pixels, y_pixels) = (x_pixels as usize, y_pixels as usize);
        if !embolden_rendered_bitmap(bitmap, x_pixels, y_pixels) {
            return;
        }

        self.bitmap_top = self.bitmap_top.wrapping_add(y_pixels as i32);
        bitmap.left = self.bitmap_left;
        bitmap.top = self.bitmap_top;
        self.apply_synthetic_weight_metrics(xstrength as i32, ystrength as i32);
    }

    fn apply_synthetic_weight_metrics(&mut self, xstrength: i32, ystrength: i32) {
        if self.advance.x != 0 {
            self.advance.x = self.advance.x.wrapping_add(xstrength);
        }
        if self.advance.y != 0 {
            self.advance.y = self.advance.y.wrapping_add(ystrength);
        }

        // FreeType ftsynth.c updates these slot metrics even though
        // FT_Outline_EmboldenXY reports errors only to its ignored return value.
        self.metrics.width = self.metrics.width.wrapping_add(xstrength);
        self.metrics.height = self.metrics.height.wrapping_add(ystrength);
        self.metrics.hori_advance = self.metrics.hori_advance.wrapping_add(xstrength);
        self.metrics.vert_advance = self.metrics.vert_advance.wrapping_add(ystrength);
        self.metrics.hori_bearing_y = self.metrics.hori_bearing_y.wrapping_add(ystrength);
    }

    fn recompute_outline_boxes(&mut self) {
        let mut new_cbox = crate::font::BBox {
            x_min: 0,
            y_min: 0,
            x_max: 0,
            y_max: 0,
        };
        let mut new_bbox = crate::font::BBox {
            x_min: 0,
            y_min: 0,
            x_max: 0,
            y_max: 0,
        };
        if let Some(ref outline) = self.slot_outline {
            if let Some(cbox) = outline_point_cbox(&outline.points) {
                new_cbox = cbox;
                new_bbox = cbox;
            }
        }
        self.outline_cbox = new_cbox;
        self.outline_bbox = new_bbox;
    }

    pub(crate) fn slot_outline(&self) -> Option<&crate::outline::Outline> {
        self.slot_outline.as_ref()
    }
}

fn sbit_pixel_mode_to_render(mode: SbitPixelMode) -> PixelMode {
    match mode {
        SbitPixelMode::Mono => PixelMode::Mono,
        SbitPixelMode::Gray2 => PixelMode::Gray2,
        SbitPixelMode::Gray4 => PixelMode::Gray4,
        SbitPixelMode::Gray => PixelMode::Gray,
        SbitPixelMode::Bgra => PixelMode::Bgra,
    }
}

fn embolden_rendered_bitmap(bitmap: &mut RenderedBitmap, x_pixels: usize, y_pixels: usize) -> bool {
    // `adjust_bitmap_weight` applies FreeType's mandatory one-pixel
    // horizontal minimum and validates nonnegative `FT_Int` pixel counts
    // before calling this private helper.
    match bitmap.pixel_mode {
        PixelMode::Gray => embolden_8bit_positive_pitch_bitmap(bitmap, x_pixels, y_pixels),
        PixelMode::Mono => embolden_mono_positive_pitch_bitmap(bitmap, x_pixels.min(8), y_pixels),
        // FreeType `src/base/ftbitmap.c:313-333` converts packed 2/4-bit
        // bitmaps to 8-bit gray before applying the same embolden loop.
        PixelMode::Gray2 => {
            convert_packed_gray_bitmap(bitmap, 2, 4)
                && embolden_8bit_positive_pitch_bitmap(bitmap, x_pixels, y_pixels)
        }
        PixelMode::Gray4 => {
            convert_packed_gray_bitmap(bitmap, 4, 16)
                && embolden_8bit_positive_pitch_bitmap(bitmap, x_pixels, y_pixels)
        }
        // FreeType returns success for color glyphs without mutating bitmap
        // bytes, then ftsynth still applies slot metric/top side effects.
        PixelMode::Bgra => true,
        // FreeType `src/base/ftbitmap.c:330-336` treats LCD bitmaps as
        // 8-bit buffers and scales only the bitmap embolden footprint by the
        // subpixel axis.  The ftsynth slot metrics still use the original
        // rounded 26.6 strengths.
        PixelMode::Lcd => x_pixels.checked_mul(3).is_some_and(|x_pixels| {
            embolden_8bit_positive_pitch_bitmap(bitmap, x_pixels, y_pixels)
        }),
        PixelMode::LcdV => y_pixels.checked_mul(3).is_some_and(|y_pixels| {
            embolden_8bit_positive_pitch_bitmap(bitmap, x_pixels, y_pixels)
        }),
    }
}

fn embolden_8bit_positive_pitch_bitmap(
    bitmap: &mut RenderedBitmap,
    x_pixels: usize,
    y_pixels: usize,
) -> bool {
    let (Ok(width), Ok(rows), Ok(pitch)) = (
        usize::try_from(bitmap.width),
        usize::try_from(bitmap.rows),
        usize::try_from(bitmap.pitch),
    ) else {
        return false;
    };
    if pitch < width || bitmap.buffer.len() < pitch.saturating_mul(rows) {
        return false;
    }
    let Some(new_pitch) = width.checked_add(x_pixels) else {
        return false;
    };

    if y_pixels == 0 && new_pitch <= pitch {
        for row in 0..rows {
            let start = row * pitch + new_pitch;
            let end = (row + 1) * pitch;
            bitmap.buffer[start..end].fill(0);
        }
    } else {
        let Some(new_rows) = rows.checked_add(y_pixels) else {
            return false;
        };
        let Some(new_len) = new_rows.checked_mul(new_pitch) else {
            return false;
        };
        let mut new_buffer = vec![0; new_len];
        for row in 0..rows {
            let src = row * pitch;
            let dst = (row + y_pixels) * new_pitch;
            new_buffer[dst..dst + width].copy_from_slice(&bitmap.buffer[src..src + width]);
        }
        bitmap.buffer = new_buffer;
        bitmap.pitch = match i32::try_from(new_pitch) {
            Ok(value) => value,
            Err(_) => return false,
        };
    }

    let pitch = usize::try_from(bitmap.pitch).unwrap_or(new_pitch);
    let max_gray = u8::try_from(bitmap.num_grays.saturating_sub(1).min(255)).unwrap_or(255);
    for row in 0..rows {
        let row_start = (row + y_pixels) * pitch;
        for x in (0..pitch).rev() {
            for i in 1..=x_pixels {
                if x < i {
                    break;
                }
                let src = bitmap.buffer[row_start + x - i];
                let dst = &mut bitmap.buffer[row_start + x];
                *dst = dst.saturating_add(src).min(max_gray);
                if *dst == max_gray {
                    break;
                }
            }
        }
        for y in 1..=y_pixels {
            let dst = row_start - pitch * y;
            for i in 0..pitch {
                bitmap.buffer[dst + i] |= bitmap.buffer[row_start + i];
            }
        }
    }

    // Both values originated as nonnegative `i32`, so they fit `u32`.
    let (x_pixels, y_pixels) = (x_pixels as u32, y_pixels as u32);
    let (Some(width), Some(rows)) = (
        bitmap.width.checked_add(x_pixels),
        bitmap.rows.checked_add(y_pixels),
    ) else {
        return false;
    };
    bitmap.width = width;
    bitmap.rows = rows;
    true
}

fn convert_packed_gray_bitmap(
    bitmap: &mut RenderedBitmap,
    bits_per_pixel: usize,
    num_grays: u16,
) -> bool {
    let (Ok(width), Ok(rows), Ok(pitch)) = (
        usize::try_from(bitmap.width),
        usize::try_from(bitmap.rows),
        usize::try_from(bitmap.pitch),
    ) else {
        return false;
    };
    let Ok(width_i32) = i32::try_from(width) else {
        return false;
    };
    let Some(bits_per_row) = width.checked_mul(bits_per_pixel) else {
        return false;
    };
    let Some(row_bytes) = bits_per_row.checked_add(7).map(|bits| bits >> 3) else {
        return false;
    };
    let Some(source_len) = pitch.checked_mul(rows) else {
        return false;
    };
    let Some(new_len) = width.checked_mul(rows) else {
        return false;
    };
    if pitch < row_bytes || bitmap.buffer.len() < source_len {
        return false;
    }

    let mut new_buffer = vec![0; new_len];
    let mask = ((1u16 << bits_per_pixel) - 1) as u8;
    for row in 0..rows {
        let src_row = row * pitch;
        let dst_row = row * width;
        for x in 0..width {
            let bit_offset = x * bits_per_pixel;
            let byte = bitmap.buffer[src_row + bit_offset / 8];
            let shift = 8 - bits_per_pixel - bit_offset % 8;
            new_buffer[dst_row + x] = (byte >> shift) & mask;
        }
    }

    bitmap.buffer = new_buffer;
    bitmap.pitch = width_i32;
    bitmap.pixel_mode = PixelMode::Gray;
    bitmap.num_grays = num_grays;
    true
}

fn embolden_mono_positive_pitch_bitmap(
    bitmap: &mut RenderedBitmap,
    x_pixels: usize,
    y_pixels: usize,
) -> bool {
    let (Ok(width), Ok(rows), Ok(pitch)) = (
        usize::try_from(bitmap.width),
        usize::try_from(bitmap.rows),
        usize::try_from(bitmap.pitch),
    ) else {
        return false;
    };
    let Some(row_bytes) = width.checked_add(7).map(|value| value >> 3) else {
        return false;
    };
    let Some(source_len) = pitch.checked_mul(rows) else {
        return false;
    };
    if pitch < row_bytes || bitmap.buffer.len() < source_len {
        return false;
    }
    let Some(new_width) = width.checked_add(x_pixels) else {
        return false;
    };
    let Some(new_pitch) = new_width.checked_add(7).map(|value| value >> 3) else {
        return false;
    };

    if y_pixels == 0 && new_pitch <= pitch {
        let bit_last = new_width;
        for row in 0..rows {
            zero_mono_padding(&mut bitmap.buffer, row * pitch, pitch, bit_last);
        }
    } else {
        let Some(new_rows) = rows.checked_add(y_pixels) else {
            return false;
        };
        let Some(new_len) = new_rows.checked_mul(new_pitch) else {
            return false;
        };
        let Ok(new_pitch_i32) = i32::try_from(new_pitch) else {
            return false;
        };
        let mut new_buffer = vec![0; new_len];
        for row in 0..rows {
            let src = row * pitch;
            let dst = (row + y_pixels) * new_pitch;
            new_buffer[dst..dst + row_bytes].copy_from_slice(&bitmap.buffer[src..src + row_bytes]);
        }
        bitmap.buffer = new_buffer;
        bitmap.pitch = new_pitch_i32;
    }

    let pitch = usize::try_from(bitmap.pitch).unwrap_or(new_pitch);
    for row in 0..rows {
        let row_start = (row + y_pixels) * pitch;
        for x in (0..pitch).rev() {
            let source = bitmap.buffer[row_start + x];
            for i in 1..=x_pixels {
                bitmap.buffer[row_start + x] |= source >> i;
                if x > 0 {
                    bitmap.buffer[row_start + x] |= bitmap.buffer[row_start + x - 1] << (8 - i);
                }
            }
        }
        for y in 1..=y_pixels {
            let dst = row_start - pitch * y;
            for i in 0..pitch {
                bitmap.buffer[dst + i] |= bitmap.buffer[row_start + i];
            }
        }
    }

    // Both values originated as nonnegative `i32`, so they fit `u32`.
    let (x_pixels, y_pixels) = (x_pixels as u32, y_pixels as u32);
    let (Some(width), Some(rows)) = (
        bitmap.width.checked_add(x_pixels),
        bitmap.rows.checked_add(y_pixels),
    ) else {
        return false;
    };
    bitmap.width = width;
    bitmap.rows = rows;
    true
}

fn zero_mono_padding(buffer: &mut [u8], row_start: usize, pitch: usize, bit_last: usize) {
    let bit_width = pitch * 8;
    if bit_last >= bit_width {
        return;
    }

    let mut byte_index = bit_last >> 3;
    let shift = bit_last & 7;
    if shift > 0 {
        buffer[row_start + byte_index] &= (0xFF00u16 >> shift) as u8;
        byte_index += 1;
    }
    buffer[row_start + byte_index..row_start + pitch].fill(0);
}

fn transform_outline_points(
    points: &mut [crate::outline::OutlinePoint],
    xx: i32,
    xy: i32,
    yx: i32,
    yy: i32,
    dx: i32,
    dy: i32,
) {
    let ft_mul = crate::fixed::ft_mul_fix;
    for pt in points {
        let (px, py) = (
            ft_mul(pt.x, xx)
                .wrapping_add(ft_mul(pt.y, xy))
                .wrapping_add(dx),
            ft_mul(pt.x, yx)
                .wrapping_add(ft_mul(pt.y, yy))
                .wrapping_add(dy),
        );
        pt.x = px;
        pt.y = py;
    }
}

pub(crate) fn reverse_outline_buffers<T>(
    points: &mut [T],
    tags: &mut [u8],
    contours: &[u16],
    flags: &mut i32,
) {
    // C reference: `FT_Outline_Reverse` in `src/base/ftoutln.c:545-600`.
    // The FFI record boundary validates contour ranges before entering this
    // loop; invalid C buffers have no defined FreeType result.
    let mut first = 1usize;
    for &last in contours {
        let end = usize::from(last) + 1;
        points[first..end].reverse();
        tags[first..end].reverse();
        first = end + 1;
    }
    *flags ^= 4; // FT_OUTLINE_REVERSE_FILL
}

pub(crate) fn transform_outline_coordinates(
    points: &mut [(i64, i64)],
    xx: i64,
    xy: i64,
    yx: i64,
    yy: i64,
) {
    // C reference: `FT_Outline_Transform` and `FT_Vector_Transform` in
    // `src/base/ftoutln.c:695-734`.
    for point in points {
        let (x, y) = *point;
        point.0 =
            crate::fixed::ft_mul_fix_long(x, xx).wrapping_add(crate::fixed::ft_mul_fix_long(y, xy));
        point.1 =
            crate::fixed::ft_mul_fix_long(x, yx).wrapping_add(crate::fixed::ft_mul_fix_long(y, yy));
    }
}

fn outline_point_cbox(points: &[crate::outline::OutlinePoint]) -> Option<BBox> {
    let first = points.first()?;
    let mut cbox = BBox {
        x_min: first.x,
        y_min: first.y,
        x_max: first.x,
        y_max: first.y,
    };
    for point in &points[1..] {
        cbox.x_min = cbox.x_min.min(point.x);
        cbox.y_min = cbox.y_min.min(point.y);
        cbox.x_max = cbox.x_max.max(point.x);
        cbox.y_max = cbox.y_max.max(point.y);
    }
    Some(cbox)
}

fn transform_loaded_outline_for_render(
    loaded: &mut LoadedOutline,
    xx: i32,
    xy: i32,
    yx: i32,
    yy: i32,
    dx: i32,
    dy: i32,
) {
    if loaded.outline.is_empty() {
        return;
    }

    let base_x = loaded.left * 64;
    let base_y = loaded.bottom * 64;
    for point in &mut loaded.outline.points {
        point.x += base_x;
        point.y += base_y;
    }
    transform_outline_points(&mut loaded.outline.points, xx, xy, yx, yy, dx, dy);
    reposition_loaded_outline_for_render(loaded);
}

fn embolden_loaded_outline_for_render(loaded: &mut LoadedOutline, xstrength: i32, ystrength: i32) {
    if loaded.outline.is_empty() {
        return;
    }

    let base_x = loaded.left * 64;
    let base_y = loaded.bottom * 64;
    for point in &mut loaded.outline.points {
        point.x += base_x;
        point.y += base_y;
    }
    embolden_outline(&mut loaded.outline, xstrength, ystrength);
    reposition_loaded_outline_for_render(loaded);
}

fn reposition_loaded_outline_for_render(loaded: &mut LoadedOutline) {
    let Some(cbox) = outline_point_cbox(&loaded.outline.points) else {
        return;
    };
    let off_x = crate::scaler::ft_pix_floor(cbox.x_min);
    let off_y = crate::scaler::ft_pix_floor(cbox.y_min);
    let px_x_min = off_x >> 6;
    let px_y_min = off_y >> 6;
    let px_x_max = crate::scaler::ft_pix_ceil(cbox.x_max) >> 6;
    let px_y_max = crate::scaler::ft_pix_ceil(cbox.y_max) >> 6;

    // C transforms the slot outline before `ft_glyphslot_preset_bitmap`
    // recomputes bitmap placement from `FT_Outline_Get_CBox`
    // (`src/base/ftobjs.c:1129-1178`, `src/smooth/ftsmooth.c:595-619`).
    for point in &mut loaded.outline.points {
        point.x -= off_x;
        point.y -= off_y;
    }
    loaded.outline.cbox_x_min = 0;
    loaded.outline.cbox_y_min = 0;
    loaded.outline.cbox_x_max = px_x_max - px_x_min;
    loaded.outline.cbox_y_max = px_y_max - px_y_min;
    loaded.left = px_x_min;
    loaded.bottom = px_y_min;
    loaded.top = px_y_max;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutlineOrientation {
    TrueType,
    PostScript,
    None,
}

impl OutlineOrientation {
    fn to_ft_orientation(self) -> i32 {
        match self {
            Self::TrueType => 0,
            Self::PostScript => 1,
            Self::None => 2,
        }
    }
}

/// Return the FreeType `FT_Orientation` value for an outline.
pub fn outline_get_orientation(outline: Option<&crate::outline::Outline>) -> i32 {
    outline.map_or(0, |outline| {
        outline_orientation(outline).to_ft_orientation()
    })
}

pub(crate) fn embolden_outline(
    outline: &mut crate::outline::Outline,
    mut xstrength: i32,
    mut ystrength: i32,
) {
    // C reference: `FT_Outline_EmboldenXY` in `src/base/ftoutln.c:911-1047`.
    xstrength /= 2;
    ystrength /= 2;
    if xstrength == 0 && ystrength == 0 {
        return;
    }

    let orientation = outline_orientation(outline);
    if orientation == OutlineOrientation::None {
        return;
    }

    let Ok(contour_count) = usize::try_from(outline.n_contours) else {
        return;
    };
    if contour_count > outline.contours.len() {
        return;
    }

    let mut previous_last = -1i32;
    for contour_index in 0..contour_count {
        let first_i32 = previous_last + 1;
        let last_i32 = i32::from(outline.contours[contour_index]);
        if first_i32 < 0 || last_i32 < first_i32 {
            return;
        }
        let Ok(first) = usize::try_from(first_i32) else {
            return;
        };
        let Ok(last) = usize::try_from(last_i32) else {
            return;
        };
        if last >= outline.points.len() {
            return;
        }
        embolden_contour_points(
            &mut outline.points,
            first,
            last,
            orientation,
            xstrength,
            ystrength,
        );
        previous_last = last_i32;
    }
}

fn embolden_contour_points(
    points: &mut [crate::outline::OutlinePoint],
    first: usize,
    last: usize,
    orientation: OutlineOrientation,
    xstrength: i32,
    ystrength: i32,
) {
    let mut in_vec = (0, 0);
    let mut anchor = (0, 0);
    let mut l_in = 0;
    let mut l_anchor = 0;
    let mut i = last;
    let mut j = first;
    let mut k = None;

    while j != i && Some(i) != k {
        let (out, l_out) = if Some(j) != k {
            let out_x = points[j].x.wrapping_sub(points[i].x);
            let out_y = points[j].y.wrapping_sub(points[i].y);
            let (normalized, length) = crate::fixed::ft_vector_norm_len(out_x, out_y);
            if length == 0 {
                j = next_contour_index(j, first, last);
                continue;
            }
            (normalized, i32::try_from(length).unwrap_or(i32::MAX))
        } else {
            (anchor, l_anchor)
        };

        if l_in != 0 {
            if k.is_none() {
                k = Some(i);
                anchor = in_vec;
                l_anchor = l_in;
            }

            let mut d = crate::fixed::ft_mul_fix(in_vec.0, out.0)
                .wrapping_add(crate::fixed::ft_mul_fix(in_vec.1, out.1));
            let (shift_x, shift_y) = if d > -0xF000 {
                d = d.wrapping_add(0x10000);
                let mut shift_x = in_vec.1.wrapping_add(out.1);
                let mut shift_y = in_vec.0.wrapping_add(out.0);
                if orientation == OutlineOrientation::TrueType {
                    shift_x = shift_x.wrapping_neg();
                } else {
                    shift_y = shift_y.wrapping_neg();
                }

                let mut q = crate::fixed::ft_mul_fix(out.0, in_vec.1)
                    .wrapping_sub(crate::fixed::ft_mul_fix(out.1, in_vec.0));
                if orientation == OutlineOrientation::TrueType {
                    q = q.wrapping_neg();
                }
                let l = l_in.min(l_out);

                if crate::fixed::ft_mul_fix(xstrength, q) <= crate::fixed::ft_mul_fix(l, d) {
                    shift_x = crate::fixed::ft_mul_div(shift_x, xstrength, d);
                } else {
                    shift_x = crate::fixed::ft_mul_div(shift_x, l, q);
                }

                if crate::fixed::ft_mul_fix(ystrength, q) <= crate::fixed::ft_mul_fix(l, d) {
                    shift_y = crate::fixed::ft_mul_div(shift_y, ystrength, d);
                } else {
                    shift_y = crate::fixed::ft_mul_div(shift_y, l, q);
                }
                (shift_x, shift_y)
            } else {
                (0, 0)
            };

            while i != j {
                points[i].x = points[i].x.wrapping_add(xstrength).wrapping_add(shift_x);
                points[i].y = points[i].y.wrapping_add(ystrength).wrapping_add(shift_y);
                i = next_contour_index(i, first, last);
            }
        } else {
            i = j;
        }

        in_vec = out;
        l_in = l_out;
        j = next_contour_index(j, first, last);
    }
}

fn next_contour_index(index: usize, first: usize, last: usize) -> usize {
    if index < last { index + 1 } else { first }
}

fn outline_orientation(outline: &crate::outline::Outline) -> OutlineOrientation {
    // C reference: `FT_Outline_Get_Orientation` in `src/base/ftoutln.c:1055-1117`.
    // FreeType special-cases only a null outline or `n_points <= 0`.
    // Nonempty points with zero contours continue through cbox validation and
    // produce zero accumulated area, hence `FT_ORIENTATION_NONE`.
    if outline.points.is_empty() {
        return OutlineOrientation::TrueType;
    }

    let Some(cbox) = outline_point_cbox(&outline.points) else {
        return OutlineOrientation::TrueType;
    };
    if cbox.x_min == cbox.x_max || cbox.y_min == cbox.y_max {
        return OutlineOrientation::None;
    }
    if cbox.x_min < -0x1000000
        || cbox.y_min < -0x1000000
        || cbox.x_max > 0x1000000
        || cbox.y_max > 0x1000000
    {
        return OutlineOrientation::None;
    }

    let x_abs = ft_abs_i32_as_u32(cbox.x_max) | ft_abs_i32_as_u32(cbox.x_min);
    let xshift = (ft_msb_nonzero(x_abs) - 14).max(0);
    let yspan = cbox.y_max.wrapping_sub(cbox.y_min) as u32;
    let yshift = (ft_msb_nonzero(yspan) - 14).max(0);

    let Ok(contour_count) = usize::try_from(outline.n_contours) else {
        return OutlineOrientation::None;
    };
    if contour_count > outline.contours.len() {
        return OutlineOrientation::None;
    }

    let mut area = 0i64;
    let mut previous_last = -1i32;
    for contour_index in 0..contour_count {
        let first_i32 = previous_last + 1;
        let last_i32 = i32::from(outline.contours[contour_index]);
        if first_i32 < 0 || last_i32 < first_i32 {
            return OutlineOrientation::None;
        }
        let Ok(first) = usize::try_from(first_i32) else {
            return OutlineOrientation::None;
        };
        let Ok(last) = usize::try_from(last_i32) else {
            return OutlineOrientation::None;
        };
        if last >= outline.points.len() {
            return OutlineOrientation::None;
        }

        let mut prev_x = outline.points[last].x >> xshift;
        let mut prev_y = outline.points[last].y >> yshift;
        for point in &outline.points[first..=last] {
            let cur_x = point.x >> xshift;
            let cur_y = point.y >> yshift;
            let product = (i64::from(cur_y.wrapping_sub(prev_y)) as u64)
                .wrapping_mul(i64::from(cur_x.wrapping_add(prev_x)) as u64)
                as i64;
            area = crate::fixed::ft_add_long(area, product);
            prev_x = cur_x;
            prev_y = cur_y;
        }
        previous_last = last_i32;
    }

    if area > 0 {
        OutlineOrientation::PostScript
    } else if area < 0 {
        OutlineOrientation::TrueType
    } else {
        OutlineOrientation::None
    }
}

fn ft_abs_i32_as_u32(value: i32) -> u32 {
    if value < 0 {
        0u32.wrapping_sub(value as u32)
    } else {
        value as u32
    }
}

fn ft_msb_nonzero(value: u32) -> i32 {
    31 - value.leading_zeros() as i32
}
